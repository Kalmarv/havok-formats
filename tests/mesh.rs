//! Synthetic hknpCompressedMeshShapeData trees (both SDK layouts of the sections) check how shared vertices are
//! addressed: a section's page, and the widths of the vertex arrays. The class layouts follow the field names and
//! sizes of the Sekiro and Elden Ring type tables; no game payload is embedded here.
#![allow(
    clippy::unwrap_used,
    reason = "a test fixture that does not parse is the test's failure"
)]
mod tagbuilder;

use havok_formats::{Tagfile, compressed_mesh, mesh_sections};
use tagbuilder::{Built, Definition, Layout, fixed, primitive, record, tagfile};

const SEKIRO: &[u8; 8] = b"20160200";
const ELDEN_RING: &[u8; 8] = b"20180100";

/// The element types (type indices: 2 = 16 bits, 3 = 32, 4 = 64) of `sharedVerticesIndex`, `packedVertices` and
/// `sharedVertices`.
#[derive(Clone, Copy)]
struct Widths {
    index: u32,
    packed: u32,
    shared: u32,
}
const STORED: Widths = Widths {
    index: 2,
    packed: 3,
    shared: 4,
};

/// One section as the fixture stores it.
#[derive(Clone, Copy)]
struct Section {
    first_packed: u32,
    packed: u8,
    first_shared: u32,
    shared_indices: u8,
    first_primitive: u32,
    primitives: u8,
    page: u8,
}

/// The `hkAabb` of every domain here: a shared vertex's three codes then decode to themselves (the step is exactly 1).
const DOMAIN: [f32; 8] = [0.0, 0.0, 0.0, 0.0, 2_097_151.0, 2_097_151.0, 4_194_303.0, 0.0];

/// A shared vertex's code: `x` in the low 21 bits, `y` in the next 21, `z` in the top 22.
const fn shared_code(x: u64, y: u64, z: u64) -> u64 {
    x | (y << 21) | (z << 42)
}

fn definitions(sdk: &[u8; 8], widths: Widths) -> Vec<Definition> {
    let section = if sdk == SEKIRO {
        // sharedVertices.data = first shared index << 8 | packed vertices, primitives.data = first primitive << 8 | count
        vec![
            ("codecParms", 0, 7),
            ("domain", 32, 9),
            ("firstPackedVertex", 64, 3),
            ("sharedVertices", 68, 12),
            ("primitives", 72, 13),
            ("dataRuns", 76, 14),
            ("numPackedVertices", 80, 1),
            ("numSharedIndices", 81, 1),
            ("leafIndex", 82, 2),
            ("page", 84, 1),
            ("flags", 85, 1),
            ("layerData", 86, 1),
            ("unusedData", 87, 1),
        ]
    } else {
        vec![
            ("codecParms", 0, 7),
            ("domain", 32, 9),
            ("firstPackedVertexIndex", 64, 3),
            ("firstSharedVertexIndex", 68, 3),
            ("firstPrimitiveIndex", 72, 3),
            ("firstDataRunIndex", 76, 3),
            ("numPackedVertices", 80, 1),
            ("numPrimitives", 81, 1),
            ("numDataRuns", 82, 1),
            ("page", 83, 1),
            ("leafIndex", 84, 2),
            ("layerData", 86, 1),
            ("flags", 87, 1),
        ]
    };
    vec![
        primitive("hkUint8", 0x2004, 1, None),                               // 1
        primitive("hkUint16", 0x4004, 2, None),                              // 2
        primitive("hkUint32", 0x8004, 4, None),                              // 3
        primitive("hkUint64", 0x10004, 8, None),                             // 4
        primitive("hkReal", 0x174605, 4, None),                              // 5
        fixed("T[N]<hkReal,4>", 4, 5, 4),                                    // 6
        fixed("T[N]<hkReal,6>", 6, 5, 4),                                    // 7
        fixed("T[N]<hkUint8,4>", 4, 1, 1),                                   // 8
        record("hkAabb", 32, vec![("min", 0, 6), ("max", 16, 6)]),           // 9
        record("hkcdStaticMeshTree::Primitive", 4, vec![("indices", 0, 8)]), // 10
        record("hkcdStaticMeshTree::Section", 88, section),                  // 11
        record(
            "hkcdStaticMeshTree::Section::SharedVertices",
            4,
            vec![("data", 0, 3)],
        ), // 12
        record("hkcdStaticMeshTree::Section::Primitives", 4, vec![("data", 0, 3)]), // 13
        record("hkcdStaticMeshTree::Section::DataRuns", 4, vec![("data", 0, 3)]), // 14
        primitive("hkArray", 8, 16, Some(11)),                               // 15 sections
        primitive("hkArray", 8, 16, Some(10)),                               // 16 primitives
        primitive("hkArray", 8, 16, Some(widths.index)),                     // 17 sharedVerticesIndex
        primitive("hkArray", 8, 16, Some(widths.packed)),                    // 18 packedVertices
        primitive("hkArray", 8, 16, Some(widths.shared)),                    // 19 sharedVertices
        record(
            "hkcdStaticMeshTree",
            112,
            vec![
                ("domain", 0, 9),
                ("sections", 32, 15),
                ("primitives", 48, 16),
                ("sharedVerticesIndex", 64, 17),
                ("packedVertices", 80, 18),
                ("sharedVertices", 96, 19),
            ],
        ), // 20
        record("hknpCompressedMeshShapeData", 112, vec![("meshTree", 0, 20)]), // 21
    ]
}

/// The bytes an element of type `type_index` (2, 3 or 4) takes.
fn width(type_index: u32) -> usize {
    match type_index {
        2 => 2,
        3 => 4,
        _ => 8,
    }
}

/// The values of a mesh tree's arrays: `shared` are the (slot, code) pairs set among `shared_len` shared vertices.
struct Tree<'a> {
    sections: &'a [Section],
    primitives: &'a [[u8; 4]],
    entries: &'a [u32],
    packed: &'a [u32],
    shared_len: usize,
    shared: &'a [(usize, u64)],
}

/// A tagfile whose root is an hknpCompressedMeshShapeData holding `tree`.
fn mesh_fixture(sdk: &[u8; 8], widths: Widths, tree: &Tree<'_>) -> Built<Vec<u8>> {
    let sekiro = sdk == SEKIRO;
    let mut layout = Layout::default();
    let (_, root) = layout.alloc(21, 112, 1, false)?;
    for (i, v) in DOMAIN.iter().enumerate() {
        layout.put(root + 4 * i, &v.to_le_bytes());
    }
    // an array member of the tree at `field`: its item and the item's DATA offset (none for an empty array)
    let array =
        |layout: &mut Layout, field: usize, type_index: u32, size: usize, count: usize| -> Built<usize> {
            if count == 0 {
                return Ok(0);
            }
            let (item, offset) = layout.alloc(type_index, size, u32::try_from(count)?, true)?;
            layout.put(root + field, &item.to_le_bytes());
            Ok(offset)
        };
    let at = array(&mut layout, 32, 11, 88 * tree.sections.len(), tree.sections.len())?;
    for (i, s) in tree.sections.iter().enumerate() {
        let base = at + 88 * i;
        // codecParms: offset xyz, scale xyz (the packed vertices decode to their codes)
        for (k, v) in [0.0f32, 0.0, 0.0, 1.0, 1.0, 1.0].iter().enumerate() {
            layout.put(base + 4 * k, &v.to_le_bytes());
        }
        for (k, v) in DOMAIN.iter().enumerate() {
            layout.put(base + 32 + 4 * k, &v.to_le_bytes());
        }
        layout.put(base + 64, &s.first_packed.to_le_bytes());
        if sekiro {
            layout.put(
                base + 68,
                &((s.first_shared << 8) | u32::from(s.packed)).to_le_bytes(),
            );
            layout.put(
                base + 72,
                &((s.first_primitive << 8) | u32::from(s.primitives)).to_le_bytes(),
            );
            layout.put(base + 80, &[s.packed, s.shared_indices]);
            layout.put(base + 84, &[s.page]);
        } else {
            layout.put(base + 68, &s.first_shared.to_le_bytes());
            layout.put(base + 72, &s.first_primitive.to_le_bytes());
            layout.put(base + 80, &[s.packed, s.primitives, 0, s.page]);
        }
    }
    let at = array(
        &mut layout,
        48,
        10,
        4 * tree.primitives.len(),
        tree.primitives.len(),
    )?;
    for (i, p) in tree.primitives.iter().enumerate() {
        layout.put(at + 4 * i, p);
    }
    let n = width(widths.index);
    let at = array(
        &mut layout,
        64,
        widths.index,
        n * tree.entries.len(),
        tree.entries.len(),
    )?;
    for (i, e) in tree.entries.iter().enumerate() {
        layout.put(at + n * i, &e.to_le_bytes()[..n]);
    }
    let n = width(widths.packed);
    let at = array(
        &mut layout,
        80,
        widths.packed,
        n * tree.packed.len(),
        tree.packed.len(),
    )?;
    for (i, e) in tree.packed.iter().enumerate() {
        layout.put(at + n * i, &e.to_le_bytes()[..n]);
    }
    let n = width(widths.shared);
    let at = array(
        &mut layout,
        96,
        widths.shared,
        n * tree.shared_len,
        tree.shared_len,
    )?;
    for &(slot, code) in tree.shared {
        layout.put(at + n * slot, &code.to_le_bytes()[..n]);
    }
    tagfile(sdk, &definitions(sdk, widths), layout.data, &layout.items)
}

/// Shared vertices 0 to 2 (page 0) and 65,536 and 65,537 (page 1).
const SHARED: [(usize, u64); 5] = [
    (0, shared_code(1, 2, 3)),
    (1, shared_code(4, 5, 6)),
    (2, shared_code(7, 8, 9)),
    (65_536, shared_code(11, 12, 13)),
    (65_537, shared_code(14, 15, 16)),
];

/// Two sections: section 0 (page 0) has one triangle on shared entries 0, 1 and 2; section 1 (page `page`) has one
/// triangle on its packed vertex (code x 1, y 2, z 3) and its own shared entries 0 and 1, which store the same 16-bit
/// values as section 0's first two.
fn two_pages(sdk: &[u8; 8], widths: Widths, page: u8) -> Built<Vec<u8>> {
    let sections = [
        Section {
            first_packed: 0,
            packed: 0,
            first_shared: 0,
            shared_indices: 3,
            first_primitive: 0,
            primitives: 1,
            page: 0,
        },
        Section {
            first_packed: 0,
            packed: 1,
            first_shared: 3,
            shared_indices: 2,
            first_primitive: 1,
            primitives: 1,
            page,
        },
    ];
    mesh_fixture(
        sdk,
        widths,
        &Tree {
            sections: &sections,
            primitives: &[[0, 1, 2, 2], [0, 1, 2, 2]],
            entries: &[0, 1, 2, 0, 1],
            packed: &[1 | (2 << 11) | (3 << 22)],
            shared_len: 65_538,
            shared: &SHARED,
        },
    )
}

#[test]
fn a_sections_page_selects_its_shared_vertices() -> Built<()> {
    for sdk in [SEKIRO, ELDEN_RING] {
        let bytes = two_pages(sdk, STORED, 1)?;
        let tag = Tagfile::read(&bytes, None).unwrap();
        let data = tag.root().unwrap();
        let sections = mesh_sections(data).unwrap();
        assert_eq!(sections.iter().map(|s| s.page).collect::<Vec<_>>(), [0, 1]);
        assert_eq!(
            sections[1].domain,
            [[0.0; 3], [2_097_151.0, 2_097_151.0, 4_194_303.0]]
        );
        let mesh = compressed_mesh(data).unwrap();
        let triangles: Vec<_> = mesh.triangles.iter().map(|t| (t.key, t.vertices)).collect();
        assert_eq!(
            triangles,
            [
                (0, [[1., 2., 3.], [4., 5., 6.], [7., 8., 9.]]),
                // the packed vertex, then shared entries 0 and 1 of page 1 (slots 65,536 and 65,537); a reader that
                // ignored the page would give slots 0 and 1 here
                (256, [[1., 2., 3.], [11., 12., 13.], [14., 15., 16.]]),
            ],
            "{sdk:?}"
        );
    }
    Ok(())
}

#[test]
fn a_page_past_the_shared_vertices_is_refused() -> Built<()> {
    for sdk in [SEKIRO, ELDEN_RING] {
        let bytes = two_pages(sdk, STORED, 2)?;
        let tag = Tagfile::read(&bytes, None).unwrap();
        let e = compressed_mesh(tag.root().unwrap()).unwrap_err().to_string();
        assert!(e.contains("out of bounds"), "{e}");
    }
    Ok(())
}

#[test]
fn vertex_arrays_of_another_width_are_refused() -> Built<()> {
    for sdk in [SEKIRO, ELDEN_RING] {
        for (widths, name) in [
            (Widths { index: 3, ..STORED }, "sharedVerticesIndex"),
            (Widths { packed: 2, ..STORED }, "packedVertices"),
            (Widths { shared: 3, ..STORED }, "sharedVertices"),
        ] {
            let bytes = two_pages(sdk, widths, 1)?;
            let tag = Tagfile::read(&bytes, None).unwrap();
            let e = compressed_mesh(tag.root().unwrap()).unwrap_err().to_string();
            assert!(e.contains(name) && e.contains("bytes, not the"), "{name}: {e}");
        }
    }
    Ok(())
}
