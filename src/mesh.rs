use crate::{Result, Value, invalid};

/// One triangle of a compressed mesh.
#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    /// Right-aligned shape key: section, primitive, and quad half.
    pub key: u32,
    /// Shape-local coordinates, in the coordinate frame the file stores.
    pub vertices: [[f32; 3]; 3],
}
/// The triangles of a compressed mesh tree, and the primitives that are not triangles.
#[derive(Debug, Default)]
pub struct CompressedMesh {
    /// Every triangle; a quad is two.
    pub triangles: Vec<Triangle>,
    /// How many primitives are the `DEAD` sentinel (`[0xde, 0xad, 0xde, 0xad]`), which hold nothing.
    pub dead_primitives: usize,
    /// Primitives that are not triangles or quads, in key order.
    pub custom: Vec<CustomPrimitive>,
}

/// A custom primitive of a mesh tree: one whose second and fourth indices are equal (`[k, x, n, x]`; no triangle or
/// quad in the Sekiro or Elden Ring collision read has them equal). Instead of vertices, `k` names where its data starts in the
/// shared-vertex index (counting the section's packed vertices first, as a vertex index does).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomPrimitive {
    /// Right-aligned shape key: section and primitive, quad half zero.
    pub key: u32,
    /// The primitive's four stored indices.
    pub indices: [usize; 4],
    /// What the data is.
    pub kind: CustomKind,
}

/// What a custom primitive's data is, by its first word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomKind {
    /// `[146, w, index low, index high]` with indices `[k, k, 4, k]`: the extern shape at `index` of the shape's
    /// `externShapes` (the second word is not read). Elden Ring's asset collision.
    Extern(usize),
    /// Records whose first word's low byte is 0x52 (`[0x2252, 8, 0x3d4d]`: a word, what may be a vertex index and what
    /// may be a half float; in both games'): their meaning is not known, so they are not read.
    Unread,
}

/// One extern shape instance of a compressed mesh shape (`externShapes`): a shape the mesh holds whole, placed by its
/// own transform (columns: rotation x, y and z, then the translation) in the mesh's space. Its scale is one: a scaled
/// instance is refused.
#[derive(Debug, Clone, Copy)]
pub struct ShapeInstance<'t, 'a> {
    /// Four columns of three floats: rotation x, y and z, then translation.
    pub transform: [[f32; 3]; 4],
    /// The placed shape.
    pub shape: Value<'t, 'a>,
}

/// The first word of a custom primitive that references an extern shape.
const EXTERN_SHAPE_TAG: u32 = 146;

/// The extern shape instances of an hknpCompressedMeshShape (`externShapes`), each with its transform; each one is
/// referenced by exactly one of its mesh's extern custom primitives, and they reference no other. A class without the
/// field (Havok 2016's) holds none.
pub fn extern_shapes<'t, 'a>(
    shape: Value<'t, 'a>,
    mesh: &CompressedMesh,
) -> Result<Vec<ShapeInstance<'t, 'a>>> {
    if !shape.has_field("externShapes") {
        if mesh
            .custom
            .iter()
            .any(|c| matches!(c.kind, CustomKind::Extern(_)))
        {
            return Err(invalid(
                shape.at(),
                "extern shape references in a shape without extern shapes",
            ));
        }
        return Ok(Vec::new());
    }
    let stored = shape.field("externShapes")?.array()?;
    let mut instances = Vec::with_capacity(stored.len());
    for i in 0..stored.len() {
        let instance = stored.get(i)?;
        let t = instance.field("transform")?.vector::<16>()?;
        let scale = instance.field("scale")?.vector::<4>()?;
        if scale[..3] != [1., 1., 1.] {
            return Err(invalid(instance.at(), "a scaled extern shape instance"));
        }
        // four columns of `x y z` and a padding lane
        let mut transform = [[0.0f32; 3]; 4];
        for (column, stored) in transform.iter_mut().zip(t.as_chunks::<4>().0) {
            let [x, y, z, _] = *stored;
            *column = [x, y, z];
        }
        if transform.iter().flatten().any(|x| !x.is_finite()) {
            return Err(invalid(instance.at(), "nonfinite extern shape transform"));
        }
        instances.push(ShapeInstance {
            transform,
            shape: instance.field("shape")?.dereference()?,
        });
    }
    let mut referenced = vec![false; instances.len()];
    for custom in &mesh.custom {
        let CustomKind::Extern(index) = custom.kind else {
            continue;
        };
        match referenced.get_mut(index) {
            Some(seen @ false) => *seen = true,
            Some(true) => {
                return Err(invalid(
                    shape.at(),
                    format!("extern shape {index} referenced twice"),
                ));
            }
            None => {
                return Err(invalid(
                    shape.at(),
                    format!(
                        "custom primitive references extern shape {index} of {}",
                        instances.len()
                    ),
                ));
            }
        }
    }
    if referenced.contains(&false) {
        return Err(invalid(
            shape.at(),
            "an extern shape no custom primitive references",
        ));
    }
    Ok(instances)
}

/// How a mesh-tree section names its vertex and primitive ranges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SectionLayout {
    /// 2016.2: `firstPackedVertex`, `numPackedVertices` and `numSharedIndices`, and `sharedVertices.data` /
    /// `primitives.data` each packing `(first << 8) | count`; the count in `sharedVertices.data` is the packed-vertex
    /// count again.
    Packed,
    /// 2018.1: separate `firstPackedVertexIndex`, `numPackedVertices`, `firstSharedVertexIndex`, `firstPrimitiveIndex`
    /// and `numPrimitives` fields; no shared-index count (a section's run ends where the next one's starts).
    Fields,
}
/// The section layout of each supported SDK version.
const SECTION_LAYOUTS: [([u8; 8], SectionLayout); 2] = [
    (*b"20160200", SectionLayout::Packed),
    (*b"20180100", SectionLayout::Fields),
];
/// One section of a compressed mesh tree as stored, whichever SDK version stored it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshSection {
    /// The dequantisation offset (xyz) and scale (xyz) of the section's packed vertices.
    pub codec: [f32; 6],
    /// Index of the section's first packed vertex in the tree's packed-vertex array.
    pub first_packed_vertex: usize,
    /// How many packed vertices the section has.
    pub packed_vertices: usize,
    /// The section's first entry of the shared-vertex index.
    pub first_shared_vertex: usize,
    /// How many entries of the shared-vertex index the section has: vertex indices from `packed_vertices` on reach
    /// them.
    pub shared_indices: usize,
    /// Index of the section's first primitive in the tree's primitive array.
    pub first_primitive: usize,
    /// How many primitives the section has.
    pub primitives: usize,
}
/// An unsigned stored value as an index; a refusal at `at` when it does not fit the platform's `usize`.
fn index(value: Value<'_, '_>, at: usize) -> Result<usize> {
    let stored = value.unsigned()?;
    usize::try_from(stored)
        .map_err(|_| invalid(at, format!("expected an index that fits usize, found {stored}")))
}

/// The sections of an hknpCompressedMeshShapeData mesh tree, as its SDK version names their ranges.
///
/// The sections' ranges tile the tree's arrays: each section's packed vertices, shared-vertex index entries and
/// primitives start where the previous section's end (the first at 0), and the last ends at the array's end. In 2016.2
/// the count packed into `sharedVertices.data` equals `numPackedVertices`. Both hold in every compressed mesh of the
/// Sekiro and Elden Ring installs' collision (273,228 and 722,403 sections); a tree that breaks either is refused.
pub fn mesh_sections(data: Value<'_, '_>) -> Result<Vec<MeshSection>> {
    let layout = SECTION_LAYOUTS
        .iter()
        .find(|(version, _)| *version == data.tag.version)
        .map(|(_, layout)| *layout)
        .ok_or_else(|| invalid(data.at(), "no mesh section layout for this SDK version"))?;
    let tree = data.field("meshTree")?;
    let sections = tree.field("sections")?.array()?;
    let shared_index_len = tree.field("sharedVerticesIndex")?.array()?.len();
    let mut out: Vec<MeshSection> = Vec::with_capacity(sections.len());
    // where the next section's ranges must start
    let (mut packed_at, mut shared_at, mut primitive_at) = (0usize, 0usize, 0usize);
    for i in 0..sections.len() {
        let section = sections.get(i)?;
        let field = |name: &str| index(section.field(name)?, section.at());
        let (
            first_packed_vertex,
            packed_vertices,
            first_shared_vertex,
            shared_indices,
            first_primitive,
            primitives,
        ) = match layout {
            SectionLayout::Packed => {
                let shared = index(section.field("sharedVertices")?.field("data")?, section.at())?;
                let range = index(section.field("primitives")?.field("data")?, section.at())?;
                let packed_vertices = field("numPackedVertices")?;
                if shared & 255 != packed_vertices {
                    return Err(invalid(
                        section.at(),
                        format!(
                            "section {i}: the count in sharedVertices.data is {}, numPackedVertices {packed_vertices}",
                            shared & 255
                        ),
                    ));
                }
                (
                    field("firstPackedVertex")?,
                    packed_vertices,
                    shared >> 8,
                    field("numSharedIndices")?,
                    range >> 8,
                    range & 255,
                )
            }
            SectionLayout::Fields => {
                let first_shared = field("firstSharedVertexIndex")?;
                let end = if i.saturating_add(1) < sections.len() {
                    let next = sections.get(i.saturating_add(1))?;
                    index(next.field("firstSharedVertexIndex")?, next.at())?
                } else {
                    shared_index_len
                };
                let shared_indices = end.checked_sub(first_shared).ok_or_else(|| {
                        invalid(
                            section.at(),
                            format!("section {i}: its shared-vertex index entries start at {first_shared}, past the next section's {end}"),
                        )
                    })?;
                (
                    field("firstPackedVertexIndex")?,
                    field("numPackedVertices")?,
                    first_shared,
                    shared_indices,
                    field("firstPrimitiveIndex")?,
                    field("numPrimitives")?,
                )
            }
        };
        for (what, first, want) in [
            ("packed vertex", first_packed_vertex, packed_at),
            ("shared-vertex index entry", first_shared_vertex, shared_at),
            ("primitive", first_primitive, primitive_at),
        ] {
            if first != want {
                return Err(invalid(
                    section.at(),
                    format!(
                        "section {i}: its first {what} is {first}, not {want} where the earlier sections end"
                    ),
                ));
            }
        }
        let overflow = || invalid(section.at(), "section range overflow");
        packed_at = packed_at.checked_add(packed_vertices).ok_or_else(overflow)?;
        shared_at = shared_at.checked_add(shared_indices).ok_or_else(overflow)?;
        primitive_at = primitive_at.checked_add(primitives).ok_or_else(overflow)?;
        out.push(MeshSection {
            codec: section.field("codecParms")?.vector::<6>()?,
            first_packed_vertex,
            packed_vertices,
            first_shared_vertex,
            shared_indices,
            first_primitive,
            primitives,
        });
    }
    for (what, end, len) in [
        (
            "packedVertices",
            packed_at,
            tree.field("packedVertices")?.array()?.len(),
        ),
        ("sharedVerticesIndex", shared_at, shared_index_len),
        (
            "primitives",
            primitive_at,
            tree.field("primitives")?.array()?.len(),
        ),
    ] {
        if end != len {
            return Err(invalid(
                sections.get(0).map_or(data.at(), |s| s.at()),
                format!("the sections cover {end} entries of {what}, which has {len}"),
            ));
        }
    }
    Ok(out)
}

/// Decode an hknpCompressedMeshShapeData mesh tree. Materials and rigid-body
/// transforms belong to the caller. Packed section vertices are 11/11/10 bits;
/// shared vertices are 21/21/22 bits over the declared domain. Custom primitives
/// are listed ([`CustomPrimitive`]), not decoded.
pub fn compressed_mesh(data: Value<'_, '_>) -> Result<CompressedMesh> {
    let tree = data.field("meshTree")?;
    let domain = tree.field("domain")?;
    let min = domain.field("min")?.vector::<4>()?;
    let max = domain.field("max")?.vector::<4>()?;
    let min = [min[0], min[1], min[2]];
    let max = [max[0], max[1], max[2]];
    let primitives = tree.field("primitives")?.array()?;
    let packed = tree.field("packedVertices")?.array()?;
    let shared = tree.field("sharedVertices")?.array()?;
    let shared_indices = tree.field("sharedVerticesIndex")?.array()?;
    let sections = tree.field("sections")?.array()?;
    let ranges = mesh_sections(data)?;
    let mut result = CompressedMesh::default();
    for (section_index, range) in ranges.iter().enumerate() {
        let section = sections.get(section_index)?;
        let &MeshSection {
            codec,
            first_packed_vertex: first,
            packed_vertices: local_count,
            first_shared_vertex: shared_base,
            shared_indices: shared_count,
            first_primitive: start,
            primitives: count,
        } = range;
        let offset = [codec[0], codec[1], codec[2]];
        let scale = [codec[3], codec[4], codec[5]];
        // The primitive occupies seven key bits; its quad half occupies one.
        if count > 128 || section_index > 0x00ff_ffff {
            return Err(invalid(section.at(), "shape key range overflow"));
        }
        let vertex = |index: usize| -> Result<[f32; 3]> {
            if index >= local_count.saturating_add(shared_count) {
                return Err(invalid(
                    section.at(),
                    format!(
                        "vertex index {index} is past the section's {local_count} packed vertices and {shared_count} shared ones"
                    ),
                ));
            }
            if index < local_count {
                let index = first
                    .checked_add(index)
                    .ok_or_else(|| invalid(section.at(), "packed vertex index overflow"))?;
                let value = u32::try_from(packed.get(index)?.unsigned()?)
                    .map_err(|_| invalid(section.at(), "packed vertex exceeds u32"))?;
                primase_foundations::vertex::packed_position32(value, scale, offset)
                    .map_err(|e| invalid(section.at(), e.to_string()))
            } else {
                let position = index
                    .checked_sub(local_count)
                    .and_then(|i| i.checked_add(shared_base))
                    .ok_or_else(|| invalid(section.at(), "shared vertex index overflow"))?;
                let position = self::index(shared_indices.get(position)?, section.at())?;
                primase_foundations::vertex::packed_position64(shared.get(position)?.unsigned()?, min, max)
                    .map_err(|e| invalid(section.at(), e.to_string()))
            }
        };
        for i in 0..count {
            let primitive = start
                .checked_add(i)
                .ok_or_else(|| invalid(section.at(), "primitive index overflow"))?;
            let indices = primitives.get(primitive)?.field("indices")?.array()?;
            if indices.len() != 4 {
                return Err(invalid(section.at(), "primitive must have four indices"));
            }
            let mut ix = [0usize; 4];
            for (j, x) in ix.iter_mut().enumerate() {
                *x = index(indices.get(j)?, section.at())?;
            }
            if ix == [0xde, 0xad, 0xde, 0xad] {
                result.dead_primitives = result.dead_primitives.saturating_add(1);
                continue;
            }
            // the section number above the quad half and the primitive's seven bits (bounded by the check above)
            let key = u32::try_from(section_index)
                .ok()
                .and_then(|section| section.checked_mul(256))
                .zip(
                    u32::try_from(i)
                        .ok()
                        .and_then(|primitive| primitive.checked_mul(2)),
                )
                .map(|(section, primitive)| section | primitive)
                .ok_or_else(|| invalid(section.at(), "shape key range overflow"))?;
            if ix[1] == ix[3] {
                // a custom primitive `[k, x, n, x]`: its data from vertex index k on in the shared-vertex index
                let k = ix[0];
                if k >= local_count.saturating_add(shared_count) {
                    return Err(invalid(
                        section.at(),
                        format!(
                            "custom primitive {ix:?} starts past the section's shared-vertex index entries"
                        ),
                    ));
                }
                let start = k
                    .checked_sub(local_count)
                    .and_then(|o| o.checked_add(shared_base))
                    .ok_or_else(|| {
                        invalid(
                            section.at(),
                            format!("custom primitive {ix:?} names a packed vertex"),
                        )
                    })?;
                let word = |j: usize| -> Result<u32> {
                    let at = start
                        .checked_add(j)
                        .ok_or_else(|| invalid(section.at(), "custom primitive word index overflow"))?;
                    u32::try_from(shared_indices.get(at)?.unsigned()?)
                        .map_err(|_| invalid(section.at(), "custom primitive word exceeds u32"))
                };
                let kind = match word(0)? {
                    EXTERN_SHAPE_TAG if ix == [k, k, 4, k] => {
                        let (low, high) = (word(2)?, word(3)?);
                        if low > 0xffff || high > 0xffff {
                            return Err(invalid(section.at(), "extern shape index exceeds 32 bits"));
                        }
                        CustomKind::Extern((low | (high << 16)) as usize)
                    }
                    first if first != EXTERN_SHAPE_TAG && first & 0xff == 0x52 => CustomKind::Unread,
                    first => {
                        return Err(invalid(
                            section.at(),
                            format!("custom primitive {ix:?} of an unknown kind (first word {first})"),
                        ));
                    }
                };
                result.custom.push(CustomPrimitive {
                    key,
                    indices: ix,
                    kind,
                });
                continue;
            }
            let a = vertex(ix[0])?;
            let b = vertex(ix[1])?;
            let c = vertex(ix[2])?;
            result.triangles.push(Triangle {
                key,
                vertices: [a, b, c],
            });
            if ix[2] != ix[3] {
                result.triangles.push(Triangle {
                    key: key | 1,
                    vertices: [a, c, vertex(ix[3])?],
                });
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::disallowed_methods,
    reason = "a test reads the corpus's files; the library itself takes bytes"
)]
mod tests {
    use crate::inputs::{Game, OracleSet, Query, expect_found, resolve};
    use crate::{Compendium, Tagfile, mesh_sections};

    /// The tagfile, with its own types or the first compendium that has them.
    fn read<'a>(bytes: &'a [u8], compendia: &[Compendium]) -> Tagfile<'a> {
        std::iter::once(None)
            .chain(compendia.iter().map(Some))
            .find_map(|c| Tagfile::read(bytes, c).ok())
            .unwrap()
    }

    /// A real mesh tree whose section counts no longer tile its arrays is refused: one of the corpus's trees of two or
    /// more sections, with the packed-vertex count of section 0 and then the first packed vertex of section 1 moved
    /// by one. Skips without the Sekiro corpus.
    #[test]
    fn sections_that_do_not_tile_the_arrays_are_refused() {
        let corpus = Query::Oracle(OracleSet::Corpus, Game::Sekiro, "havok");
        let Some(dir) = resolve(corpus) else {
            return;
        };
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        files.sort();
        let with = |ext: &str| {
            files
                .iter()
                .filter(|p| p.extension().is_some_and(|e| e == ext))
                .map(|p| std::fs::read(p).unwrap())
                .collect::<Vec<_>>()
        };
        let compendia = with("compendium");
        let compendia: Vec<_> = compendia.iter().map(|b| Compendium::read(b).unwrap()).collect();
        let tagfiles = with("hkx");
        expect_found(corpus, ".hkx files", tagfiles.len());
        // the first tree of two or more sections: its file, item, and the file offsets of the two counts
        let (bytes, item, packed_count, second_first) = tagfiles
            .iter()
            .find_map(|bytes| {
                let tag = read(bytes, &compendia);
                (1..tag.items.len()).find_map(|i| {
                    let data = tag.item(i).ok()?.get(0).ok()?;
                    let sections = data
                        .field("meshTree")
                        .ok()?
                        .field("sections")
                        .ok()?
                        .array()
                        .ok()?;
                    if data.type_name() != "hknpCompressedMeshShapeData" || sections.len() < 2 {
                        return None;
                    }
                    assert!(mesh_sections(data).is_ok());
                    let at = |s: usize, name: &str| sections.get(s).unwrap().field(name).unwrap().at();
                    Some((
                        bytes.clone(),
                        i,
                        at(0, "numPackedVertices"),
                        at(1, "firstPackedVertex"),
                    ))
                })
            })
            .expect("a mesh tree of two or more sections");
        for (at, want) in [
            (packed_count, "the count in sharedVertices.data"),
            (second_first, "its first packed vertex is"),
        ] {
            let mut patched = bytes.clone();
            patched[at] = patched[at].wrapping_add(1);
            let tag = read(&patched, &compendia);
            let e = mesh_sections(tag.item(item).unwrap().get(0).unwrap())
                .unwrap_err()
                .to_string();
            assert!(e.contains(want), "{e}");
        }
    }
}
