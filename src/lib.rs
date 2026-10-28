//! Reads Havok physics and animation data as games store it: tagfiles (`.hkx`) and packfiles, and the collision meshes,
//! convex shapes, cloth, ragdolls, skeletons and spline-compressed animations inside them. Every read is
//! bounds-checked, so untrusted files produce errors, not panics. It is an independent reader of the stored formats and
//! contains no Havok SDK code.
//!
//! # What it reads
//!
//! - **Tagfiles** (`TAG0`, SDK versions 2015.1, 2016.2 and 2018.1, see [`SDK_VERSIONS`]) and their external type compendia
//!   (`TCM0`): [`Tagfile`] and [`Compendium`]. Class layouts come from the file's own type metadata, so any class
//!   can be read field by field through [`Value`] and [`Array`].
//! - **Packfiles** (version 7, contents version `Havok-7.1.0-r1`, 4-byte pointers, big endian): [`packfile`].
//! - Objects read from them: `hknp` compressed meshes ([`compressed_mesh`], with their bounding-volume trees
//!   [`mesh_trees`] and [`simd_tree`]), extern triangle meshes ([`extern_mesh`]),
//!   convex shapes ([`convex`]), cloth and ragdoll data ([`cloth`]), skeletons, animation bindings and spline
//!   compressed animation ([`animation`], [`spline`]), and the 7.1 `hkp` rigid bodies and collision shapes of packfiles
//!   ([`packfile::physics`]).
//!
//! It was written against Sekiro (tagfiles, 2016.2), Elden Ring (tagfiles, 2018.1) and Tony Hawk's SHRED for PS3
//! (packfiles).
//!
//! # Guarantees
//!
//! Every read is checked. Data that is shorter than it says, refers outside itself, or holds something the reader
//! does not know is an [`Error`] that names the byte offset and what was wrong, never a panic. Unknown sections,
//! versions and flags are refused rather than skipped. Type indices are one-based; zero denotes a null type or object
//! reference.
//!
//! It does not validate every byte of a file, only what the accessors you call read, and it does not write Havok data
//! or simulate anything.
//!
//! # Example
//!
//! Read the triangles of the collision meshes in a tagfile (a Sekiro map collision file, whose types are in a shared
//! compendium):
//!
//! ```no_run
//! use havok_formats::{Compendium, Tagfile, compressed_mesh};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let compendium = Compendium::read(&std::fs::read("shared.compendium")?)?;
//!     let bytes = std::fs::read("collision.hkx")?;
//!     let tagfile = Tagfile::read(&bytes, Some(&compendium))?;
//!
//!     let variants = tagfile.root()?.field("namedVariants")?.array()?;
//!     for i in 0..variants.len() {
//!         let physics = variants.get(i)?.field("variant")?.dereference()?;
//!         let systems = physics.field("systemDatas")?.array()?;
//!         for j in 0..systems.len() {
//!             let bodies = systems.get(j)?.dereference()?.field("bodyCinfos")?.array()?;
//!             for k in 0..bodies.len() {
//!                 let shape = bodies.get(k)?.field("shape")?.dereference()?;
//!                 let mesh = compressed_mesh(shape.field("data")?.dereference()?)?;
//!                 println!("{}: {} triangles", shape.type_name(), mesh.triangles.len());
//!             }
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//!
//! A file the reader refuses says where and why:
//!
//! ```
//! let error = havok_formats::Tagfile::read(b"\0\0\0\x07TAG0", None).unwrap_err();
//! assert_eq!(error.offset, 0);
//! assert_eq!(error.to_string(), "Havok at 0x0: section length is less than its header");
//! ```
#![deny(missing_docs, clippy::disallowed_methods, clippy::disallowed_types)]
#![cfg_attr(
    not(test),
    deny(
        clippy::arithmetic_side_effects,
        clippy::cast_possible_truncation,
        clippy::indexing_slicing
    )
)]

#[cfg(test)]
#[path = "../tests/inputs/mod.rs"]
mod inputs;

pub mod animation;
mod bvtree;
pub mod cloth;
pub mod convex;
mod extern_mesh;
mod mesh;
pub mod packfile;
mod section;
mod simdtree;
pub mod spline;
mod spline_sample;

pub use bvtree::{
    BOUNDS_TOLERANCE_ULPS, BoundsCheck, BvNode, BvNodeKind, BvTree, MeshTrees, NodeWidth, SectionTree,
    mesh_trees,
};
pub use extern_mesh::{ExternMesh, extern_mesh};
pub use mesh::{
    CompressedMesh, CustomKind, CustomPrimitive, MeshSection, ShapeInstance, Triangle, compressed_mesh,
    extern_shapes, mesh_sections,
};
mod tagfile;
mod types;
mod value;
pub use tagfile::{Item, ItemKind, SDK_VERSIONS, Tagfile};
pub use types::{Compendium, Field, Type, TypeFlags};
pub use value::{Array, Value};

pub use section::{Section, sections};
pub use simdtree::{LaneBox, SimdCheck, SimdLane, SimdNode, SimdTree, simd_tree};

/// A refusal: what is wrong, at the byte offset where it was found.
#[derive(Debug, thiserror::Error)]
#[error("Havok at {offset:#x}: {detail}")]
pub struct Error {
    /// The byte offset in the data read.
    pub offset: usize,
    /// What the data holds that the reader refuses, or lacks.
    pub detail: String,
}

impl Error {
    /// A refusal at `offset`.
    pub fn new(offset: usize, detail: impl Into<String>) -> Self {
        Error {
            offset,
            detail: detail.into(),
        }
    }
}

/// A read's result.
pub type Result<T> = std::result::Result<T, Error>;

/// A failed byte-range read of a cursor, as a refusal at the range's offset in the data read.
impl From<primase_foundations::bytes::BoundsError> for Error {
    fn from(e: primase_foundations::bytes::BoundsError) -> Self {
        use primase_foundations::bytes::BoundsError;
        let detail = match e {
            BoundsError::LengthOverflow { count, stride, .. } => {
                format!("{count} entries of {stride} bytes overflow the address space")
            }
            BoundsError::OutOfBounds { length, total, .. } => {
                format!("{length} bytes run past the end of the {total} bytes of data")
            }
        };
        invalid(usize::try_from(e.absolute()).unwrap_or(usize::MAX), detail)
    }
}

fn invalid(offset: usize, detail: impl Into<String>) -> Error {
    Error::new(offset, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use primase_foundations::bytes::BoundsError;

    #[test]
    fn a_bounds_error_is_a_refusal_at_its_offset_that_says_what_ran_out() {
        let past = Error::from(BoundsError::OutOfBounds {
            offset: 4,
            origin: 0x10,
            length: 8,
            total: 6,
        });
        assert_eq!(past.offset, 0x14);
        assert_eq!(past.detail, "8 bytes run past the end of the 6 bytes of data");
        let overflow = Error::from(BoundsError::LengthOverflow {
            offset: 4,
            origin: 0x10,
            count: usize::MAX,
            stride: 4,
        });
        assert_eq!(overflow.offset, 0x14);
        assert!(overflow.detail.contains("overflow"), "{overflow}");
        assert!(!past.detail.contains("animation") && !past.detail.contains("block"));
    }
}
