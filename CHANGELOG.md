# Changelog

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
