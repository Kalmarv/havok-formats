# Changelog

## 0.3.0 (2026-10-27)

A fix to the shared vertices of the `l` collision sets, and the checks a census of every Sekiro and Elden Ring
collision binder and cloth file settled: each new refusal is of an input none of those files has.

- Fix: `compressed_mesh` ignored the section page. A section's shared-vertex index entries are 16 bits, and a section
  of page `p` addresses `sharedVertices[p * 65536 + entry]`; the reader used the entry alone, so a triangle with a
  shared vertex in such a section got another triangle's vertex. Pages 1 to 4 occur in the `l` sets only (89 Sekiro
  sections, 6,233 Elden Ring sections); 6,746 Sekiro and 347,611 Elden Ring triangles (the first triangle of each
  primitive) were decoded with wrong vertices. Every triangle of every collision mesh of both installs now equals an
  independent decode and lies inside its section's domain.
- `MeshSection::page` and `MeshSection::domain` (the section's `hkAabb`, minimum then maximum) are new, and
  `Value::element_size` gives an array value's element size from its type, empty or not.
- `compressed_mesh` refuses a tree whose `packedVertices`, `sharedVertices` or `sharedVerticesIndex` elements are not
  4, 8 and 2 bytes.
- `ConvexShape::read` refuses a polytope (a box, capsule, cylinder, triangle or polytope shape) with a different
  number of planes and faces.
- Breaking: cloth particle indices are 16-bit, as the files store them. `Link::particle_a`/`particle_b`,
  `LocalRange::particle`/`reference_vertex` are `u16` and `SimCloth::fixed_particles` is `Vec<u16>`; a file that stores
  them in another width is refused, and so is a fixed particle, a link or a local range particle that names a particle
  the sim cloth does not have.
- Breaking: `ConstraintSet::LocalRange::shape_type` is the enum `LocalRangeShape` (`Sphere` 0, `Cylinder` 1); any
  other stored value is refused.
- Tests: a synthetic tagfile builder (`tests/tagbuilder`); mesh trees of both SDK layouts with a section of page 1,
  the widths and an out-of-range page; polytope plane counts; link and local range sets with their index widths,
  bounds and shape types. A real-data test (`tests/paged_mesh.rs`, inputs `corpus.sekiro/havok-l` and
  `corpus.elden/havok-l`, skipped without them) decodes the one Sekiro and the one Elden Ring tagfile that have
  sections of page above 0 (89 and 6,233 sections, 6,980 and 507,001 triangles) and requires every triangle inside its
  section's domain: with the page ignored, 6,750 of the Sekiro triangles are not.

## 0.2.0 (2026-10-27)

Packfile members are named as the class signatures the files store define them, and what a census of the 490 Tony
Hawk's SHRED packfiles and of every Sekiro and Elden Ring collision mesh shows is now checked on every file.

- Breaking: `RigidBody::collision_filter_info` is the collidable's filter word at `+0x2C`; the word at `+0xA8` is
  `storage_index` (u16) and `contactPointCallbackDelay` (checked `0xFFFF`).
- Breaking: `MeshMaterial` is `filter_info`, `restitution`, `friction` (halves, read as `f32`; any finite value is
  accepted, where `0x3F80` was required) and `user_data` (kept as stored).
- Breaking: `Packfile::flags` is `Packfile::filler` (version 7 has no field there); `Packfile::user_tag` is read.
- `Packfile::read` refuses a class whose stored signature differs from the one the corpus stores for its name
  (`CLASS_SIGNATURES`, 24 classes); `Packfile::class_names` lists the entries with their signatures.
- Extended meshes: the scaling must be `(1, 1, 1, 1)`, the embedded subpart a copy of subpart 0 (but for its triangle
  offset), and every unread subpart byte zero.
- `mesh_sections` refuses sections that do not tile the tree's arrays, and in 2016.2 a `sharedVertices.data` count that
  is not `numPackedVertices`; `MeshSection::shared_indices` is new, and `compressed_mesh` refuses a vertex index past
  the section's packed and shared vertices.
- `hkpMotion::MotionType` constants for all eight 7.1 values (behaviour unchanged: 2, 3 and 5 are read).
- Breaking: `AnimationType::ReferenceFrame` is `AnimationType::ReferencePose` (value 6, a reference-pose animation).
- THREECOMP40/48 rotations use the f32 nearest the exact step `1 / (n sqrt 2)` (a change of at most 12 ulp per step).

## 0.1.1 (2026-10-27)

- README: install from crates.io (0.1.0's README still said to depend on the repository). No code changes.

## 0.1.0 (2026-10-27)

- First release: Havok tagfiles, packfiles, compressed collision meshes, cloth and spline animation, as game files store them.
