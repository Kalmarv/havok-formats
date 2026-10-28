# havok-formats

A Rust library that reads Havok physics and animation data as games store it: tagfiles (`.hkx`) and packfiles, and the collision meshes, convex shapes, cloth, ragdolls, skeletons and spline-compressed animations inside them. Every read is bounds-checked, so untrusted files produce errors, not panics.

## What it reads

- **Tagfiles** (`TAG0`), SDK versions **2015.1**, **2016.2** and **2018.1**, and their external type compendia (`TCM0`). Class layouts come from each file's own type metadata, so any class can be read field by field.
- **Packfiles** (the older `0x57E0E057 0x10C0C010` container), version 7, contents version `Havok-7.1.0-r1`, 4-byte pointers, big endian; versions 9 and 11, contents versions `hk_2013.1.0-r1` and `hk_2014.1.0-r1`, 8-byte pointers, little endian. `Packfile::image` and `PackfileImage::write` write them back byte for byte, and `packfile::reflection` reads the class metadata a file carries and checks it against the signatures the file stores.
- **Objects**, read by class from those files:
  - `hknp` compressed meshes (triangles, dead primitives, custom primitives, extern shape instances, and the bounding-volume trees of the mesh, of each section and the four-lane simd tree, decoded and checked as they are read), extern triangle meshes (`hknpExternMeshShape` over `hkGeometry`) and convex shapes;
  - navigation meshes (`hkaiNavMesh`: faces, edges with their opposites and user edges, vertices, ear-clipped triangles), Sekiro's cluster graphs (`hkaiDirectedGraphExplicitCost`), Elden Ring's 6-byte query trees and the user-edge setups, as `navmesh`;
  - cloth (the whole `hclClothData`: sim cloths with their particles, rest poses, constraint sets of every kind the games use, collidables and shapes, and the buffers, transform sets, operators and states around them), ragdoll data and constraint descriptions;
  - skeletons, animation bindings and animation containers, and `hkaSplineCompressedAnimation` decoded to bone transforms at a time;
  - behaviour tagfiles (`behavior`): typed views of the 93 `hkb*` and `Custom*` classes in Sekiro's and Elden Ring's `behbnd` binders, behaviour graphs with their variables, events and bindings by name, characters and projects;
  - 7.1 `hkp` rigid bodies and collision shapes in packfiles (rigid body, MOPP tree, storage extended mesh, convex vertices, box, sphere, capsule, cylinder, convex translate, list).
  - 7.1 `hkp` rigid bodies and collision shapes in packfiles (rigid body, MOPP tree, storage extended mesh, convex vertices, box, sphere, capsule, cylinder, convex translate, list), and a walker and box query for their MOPP code (`mopp`);
  - the convex point sets of custom mesh primitives, and the typed atoms of ragdoll, hinge, ball and socket, fixed and wheel constraints (`constraint`).
- **Collision written from triangles**: `build_compressed_mesh` and `replace_compressed_mesh` rebuild the mesh tree of an `hknp` compressed mesh (sections, packed and shared vertices, the trees, the simd tree) and write it into a tagfile image; `replace_extern_mesh` rewrites an extern mesh. The stock file layout is read from the file's type table, so both SDK layouts work.

It was written against Sekiro (tagfiles, 2016.2; packfiles, versions 9 and 11), Elden Ring (tagfiles, 2018.1; packfiles, versions 9 and 11) and Tony Hawk's SHRED for PS3 (packfiles, version 7). It is an independent reader of the stored formats: it contains no Havok SDK code, and its knowledge of the layouts comes from the files themselves and from public documentation and open-source readers.

## Example

```rust
use havok_formats::{Compendium, Tagfile, compressed_mesh};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let compendium = Compendium::read(&std::fs::read("shared.compendium")?)?;
    let bytes = std::fs::read("collision.hkx")?;
    let tagfile = Tagfile::read(&bytes, Some(&compendium))?;

    let variants = tagfile.root()?.field("namedVariants")?.array()?;
    for i in 0..variants.len() {
        let physics = variants.get(i)?.field("variant")?.dereference()?;
        let systems = physics.field("systemDatas")?.array()?;
        for j in 0..systems.len() {
            let bodies = systems.get(j)?.dereference()?.field("bodyCinfos")?.array()?;
            for k in 0..bodies.len() {
                let shape = bodies.get(k)?.field("shape")?.dereference()?;
                let mesh = compressed_mesh(shape.field("data")?.dereference()?)?;
                println!("{}: {} triangles", shape.type_name(), mesh.triangles.len());
            }
        }
    }
    Ok(())
}
```

The same code is the compile-checked doc test of the crate documentation, and, taking its file names from the command line, `examples/triangles.rs` (`cargo run --example triangles -- collision.hkx shared.compendium`).

## What it guarantees

- **Checked reads.** Every read is bounds-checked. Data that is shorter than it says, refers outside itself, or holds something the reader does not know is an `Error` carrying the byte offset and what was wrong (`Havok at 0x0: section length is less than its header`), not a panic. Truncating a tagfile at every length is covered by a test.
- **Refusals instead of guesses.** Unknown sections, SDK versions, type flags, packfile versions and layout rules are refused by name. A class or field the file does not have is an error naming it, never a default. Inheritance cycles among types, item and patch references outside the data, and fixups that point outside the data are refused.
- **No recursion on file data.** Object graphs are views over the borrowed bytes; references are resolved on demand, so cyclic graphs cannot overflow the stack.
- **Stored values stay as stored.** Integers keep their bits and non-finite floats in unused SIMD lanes are preserved; geometry readers reject non-finite coordinates they use.

## What it does not do

- It writes tagfiles (`TagfileImage::write`: the type table as read, the items renumbered and laid out as the game's own files are) and packfiles (`PackfileImage::write`, the same), and it does not simulate: no physics, cloth solver or animation blending.
- It is not a complete validator: it checks the values its accessors read, not every unused byte of a file.
- Tagfiles of other SDK versions, packfiles of versions other than 7, 9 and 11, big-endian spline animations and ones with more than one float track, and Havok classes without a reader here are refused or left unread (`hkpMoppCode`, convex connectivity and constraint atoms are recognised by name and counted, not decoded).
- Material meanings and coordinate-system conversions are left to the caller: it returns what the file stores.

## Tests

```sh
cargo test --workspace
```

runs the unit tests, the hand-built fixtures (tagfile and packfile records written byte by byte, including every truncated prefix of a tagfile) and the doc tests. They need no game files.

One test reads real data: `tests/corpus.rs` reads every tagfile and the compendium of a Sekiro area's map collision and decodes every compressed mesh in them. It needs your own copy of those files, and skips with one line naming what is missing when they are not configured. Point the `GAME_DATA` environment variable at a TOML file naming where your installs, keys, tools and extracted file sets are (the layout and a sample are in [primase-testdata](https://github.com/Kalmarv/primase-testdata)); [tests/inputs/README.md](tests/inputs/README.md) lists exactly which inputs this crate's tests read, what each is and where it comes from. No game files are included in this repository or needed to build it.

Add it with `cargo add havok-formats` (or `havok-formats = "0.8"` under `[dependencies]`). Building needs Rust 1.99 (edition 2024). The crate depends on [primase-foundations](https://github.com/Kalmarv/primase-foundations) (byte cursors and vertex unpacking) and, for tests only, primase-testdata.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md), which also says how to report a file the reader cannot read.

## Licence

GPL-3.0-or-later. See [LICENSE](LICENSE).

Used by [primase](https://github.com/Kalmarv/primase), a game-asset conversion library.
