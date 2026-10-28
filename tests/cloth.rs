//! Synthetic reflected layouts, including two-element reference arrays and record-valued ids.
//! The layouts follow the field names of the cloth, ragdoll and constraint classes in Sekiro's tagfiles and HKLib's 2018 class metadata;
//! no game payload is embedded here.
#![allow(
    clippy::unwrap_used,
    reason = "a test fixture that does not parse is the test's failure"
)]
mod tagbuilder;

use havok_formats::cloth::{ConstraintSet, Link, LocalRange, LocalRangeShape};
use havok_formats::{Tagfile, cloth};
use tagbuilder::{Built, Item, primitive, record, tagfile};

/// What the fixture's constraint sets store.
#[derive(Clone, Copy)]
struct Options {
    /// Particle indices as 32-bit `hkUint32` instead of the 16-bit `hkUint16` the files use.
    wide: bool,
    /// The second particle of the standard link; the sim cloth has one particle.
    link_b: u32,
    /// The particle of the local range constraint.
    local_particle: u32,
    /// The local range constraint set's `shapeType`.
    shape_type: u32,
    /// The cloth data's `targetPlatform`.
    target_platform: u32,
}
const FILES: Options = Options {
    wide: false,
    link_b: 0,
    local_particle: 0,
    shape_type: 1,
    target_platform: 2,
};

fn fixture(sdk: &[u8; 8], options: Options) -> Built<Vec<u8>> {
    // the particle index type and its size
    let (index, isz) = if options.wide { (1, 4usize) } else { (19, 2) };
    let isz32 = u32::try_from(isz)?;
    let definitions = vec![
        primitive("hkUint32", 0x8004, 4, None), // 1
        primitive("hkReal", 0x174605, 4, None), // 2
        primitive("hkStringPtr", 3, 8, None),   // 3
        primitive("T*", 6, 8, Some(1)),         // 4
        primitive("hkArray", 8, 16, Some(1)),   // 5
        primitive("hkArray", 8, 16, Some(4)),   // 6
        record(
            "hclSimClothData::ParticleData",
            16,
            vec![
                ("mass", 0, 2),
                ("invMass", 4, 2),
                ("radius", 8, 2),
                ("friction", 12, 2),
            ],
        ), // 7
        primitive("hkArray", 8, 16, Some(7)),   // 8
        record("hclSimClothPose", 24, vec![("name", 0, 3), ("positions", 8, 5)]), // 9
        record(
            "hclSimClothData",
            80,
            vec![
                ("name", 0, 3),
                ("particleDatas", 8, 8),
                ("fixedParticles", 24, 20),
                ("simClothPoses", 40, 6),
                ("staticConstraintSets", 56, 6),
                ("maxParticleRadius", 72, 2),
                ("totalMass", 76, 2),
            ],
        ), // 10
        record(
            "hclClothData",
            32,
            vec![("name", 0, 3), ("simClothDatas", 8, 6), ("targetPlatform", 24, 1)],
        ), // 11
        record(
            "hkaSkeleton",
            88,
            vec![
                ("name", 0, 3),
                ("parentIndices", 8, 5),
                ("bones", 24, 5),
                ("referencePose", 40, 5),
                ("referenceFloats", 56, 5),
                ("floatSlots", 72, 5),
            ],
        ), // 12; all arrays empty
        record(
            "hknpRagdollData",
            40,
            vec![("skeleton", 0, 4), ("boneToBodyMap", 8, 5), ("bodyTags", 24, 5)],
        ), // 13
        record("hknpBodyId", 4, vec![("serialAndIndex", 0, 1)]), // 14
        record("hknpConstraintId", 4, vec![("value", 0, 1)]), // 15
        record("hknpConstraintGroupId", 4, vec![("value", 0, 1)]), // 16
        record("hkpRagdollConstraintData", 4, Vec::new()), // 17; atoms deliberately not interpreted
        record(
            "hknpConstraintCinfo",
            40,
            vec![
                ("constraintData", 0, 4),
                ("bodyA", 8, 14),
                ("bodyB", 12, 1),
                ("flags", 16, 1),
                ("name", 24, 3),
                ("desiredConstraintId", 32, 15),
                ("constraintGroupId", 36, 16),
            ],
        ), // 18
        primitive("hkUint16", 0x4004, 2, None), // 19
        primitive("hkArray", 8, 16, Some(index)), // 20, the fixed particles
        primitive("hkUint8", 0x2004, 1, None),  // 21
        record(
            "hclStandardLinkConstraintSet::Link",
            16,
            vec![
                ("particleA", 0, index),
                ("particleB", isz32, index),
                ("restLength", 2 * isz32, 2),
                ("stiffness", 2 * isz32 + 4, 2),
            ],
        ), // 22
        primitive("hkArray", 8, 16, Some(22)),  // 23
        record("hclStandardLinkConstraintSet", 16, vec![("links", 0, 23)]), // 24
        record(
            "hclLocalRangeConstraintSet::LocalConstraint",
            24,
            vec![
                ("particleIndex", 0, index),
                ("referenceVertex", isz32, index),
                ("maximumDistance", 2 * isz32, 2),
                ("maxNormalDistance", 2 * isz32 + 4, 2),
                ("minNormalDistance", 2 * isz32 + 8, 2),
            ],
        ), // 25
        primitive("hkArray", 8, 16, Some(25)),  // 26
        record(
            "hclLocalRangeConstraintSet",
            32,
            vec![
                ("localConstraints", 0, 26),
                ("referenceMeshBufferIdx", 16, 1),
                ("stiffness", 20, 2),
                ("shapeType", 24, 1),
                ("applyNormalComponent", 28, 21),
            ],
        ), // 27
    ];
    // the cloth data object comes last: it is 32 bytes
    let cloth_data = 472;
    let mut data = vec![0; cloth_data + 32];
    for (offset, index) in [
        (cloth_data + 8, 2u64),
        (24, 3),
        (32, 3),
        (48, 6),
        (64, 7),
        (80, 4),
        (96, 13),
        (120, 5),
        (128, 5),
        (272, 8),
        (296, 10),
        (320, 12),
        // the constraint sets: the pointer array, the link set's links, the local range set's constraints
        (368, 14),
        (376, 15),
        (384, 16),
        (400, 17),
    ] {
        data[offset..offset + 8].copy_from_slice(&index.to_le_bytes());
    }
    for (offset, value) in [
        (112, 1.5f32),
        (116, 2.0),
        (160, 2.0),
        (164, 0.5),
        (168, 0.1),
        (172, 0.25),
        // the link's rest length and stiffness
        (432 + 2 * isz, 0.75),
        (436 + 2 * isz, 0.25),
        // the local range set's stiffness, then its constraint's three distances
        (420, 0.5),
        (448 + 2 * isz, 0.4),
        (452 + 2 * isz, 0.3),
        (456 + 2 * isz, 0.2),
    ] {
        data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (offset, value) in [
        (cloth_data + 24, options.target_platform),
        (312, 17u32),
        (316, 23),
        (328, 0x0102_0304),
        (332, 12),
        (336, 5),
        (352, 99),
        (356, 7),
        (424, options.shape_type),
    ] {
        data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    data[428] = 1; // applyNormalComponent
    for (offset, value) in [
        (432, 0),
        (432 + isz, options.link_b),
        (448, options.local_particle),
        (448 + isz, 5),
    ] {
        data[offset..offset + isz].copy_from_slice(&value.to_le_bytes()[..isz]);
    }
    let items = [
        (11u32, 472u32, 1u32, false),
        (4, 24, 2, true),
        (10, 40, 1, false),
        (4, 120, 2, true),
        (9, 136, 1, false),
        (7, 160, 1, true),
        (index, 176, 1, true),
        (12, 184, 1, false),
        (13, 272, 1, false),
        (1, 312, 2, true),
        (18, 320, 1, true),
        (17, 360, 1, false),
        // the constraint sets
        (4, 368, 2, true),
        (24, 384, 1, false),
        (27, 400, 1, false),
        (22, 432, 1, true),
        (25, 448, 1, true),
    ]
    .map(|(type_index, offset, count, array)| Item {
        type_index,
        offset,
        count,
        array,
    });
    tagfile(sdk, &definitions, data, &items)
}

fn read(options: Options) -> Built<Result<cloth::Physics, String>> {
    let bytes = fixture(b"20180100", options)?;
    let tag = Tagfile::read(&bytes, None).unwrap();
    Ok(cloth::read(&tag).map_err(|e| e.to_string()))
}

#[test]
fn cloth_data_reads_both_eight_byte_sim_references() -> Built<()> {
    for sdk in havok_formats::SDK_VERSIONS {
        let bytes = fixture(&sdk, FILES)?;
        let tag = Tagfile::read(&bytes, None).unwrap();
        let physics = cloth::read(&tag).unwrap();
        assert_eq!(physics.cloths.len(), 1);
        assert_eq!(physics.cloths[0].sim_cloths.len(), 2);
        assert_eq!(physics.cloths[0].target_platform, 2);
    }
    Ok(())
}

#[test]
fn target_platform_is_kept_as_stored() -> Built<()> {
    for value in [0, 1, 7, u32::MAX] {
        let physics = read(Options {
            target_platform: value,
            ..FILES
        })?
        .unwrap();
        assert_eq!(physics.cloths[0].target_platform, value);
    }
    Ok(())
}

#[test]
fn sim_cloth_reads_particles_fixed_particles_and_both_pose_references() -> Built<()> {
    let bytes = fixture(b"20180100", FILES)?;
    let tag = Tagfile::read(&bytes, None).unwrap();
    let physics = cloth::read(&tag).unwrap();
    let sim = &physics.cloths[0].sim_cloths[0];
    assert_eq!(
        sim.particles,
        [cloth::Particle {
            mass: 2.0,
            inv_mass: 0.5,
            radius: 0.1,
            friction: 0.25
        }]
    );
    assert_eq!(sim.fixed_particles, [0]);
    assert_eq!(sim.poses.len(), 2);
    assert_eq!((sim.max_particle_radius, sim.total_mass), (1.5, 2.0));
    Ok(())
}

#[test]
fn ragdoll_data_retains_its_skeleton_and_body_tags() -> Built<()> {
    let bytes = fixture(b"20180100", FILES)?;
    let tag = Tagfile::read(&bytes, None).unwrap();
    let physics = cloth::read(&tag).unwrap();
    let ragdoll = &physics.ragdolls[0];
    assert!(ragdoll.skeleton.bones.is_empty());
    assert!(ragdoll.bone_to_body.is_empty());
    assert_eq!(ragdoll.body_tags, [17, 23]);
    Ok(())
}

#[test]
fn constraint_cinfo_reads_record_handles_and_primitive_ids() -> Built<()> {
    let bytes = fixture(b"20180100", FILES)?;
    let tag = Tagfile::read(&bytes, None).unwrap();
    let physics = cloth::read(&tag).unwrap();
    let constraint = &physics.constraints[0];
    assert_eq!((constraint.body_a, constraint.body_b), (0x0102_0304, 12));
    assert_eq!(
        (constraint.desired_constraint_id, constraint.constraint_group_id),
        (99, 7)
    );
    assert_eq!(constraint.flags, 5);
    assert_eq!(constraint.data_class, "hkpRagdollConstraintData");
    Ok(())
}

#[test]
fn link_and_local_range_sets_read_sixteen_bit_particle_indices() -> Built<()> {
    for sdk in havok_formats::SDK_VERSIONS {
        let bytes = fixture(&sdk, FILES)?;
        let tag = Tagfile::read(&bytes, None).unwrap();
        let physics = cloth::read(&tag).unwrap();
        let sim = &physics.cloths[0].sim_cloths[0];
        assert_eq!(
            sim.constraint_sets,
            [
                ConstraintSet::Links {
                    class: "hclStandardLinkConstraintSet".into(),
                    links: vec![Link {
                        particle_a: 0,
                        particle_b: 0,
                        rest_length: 0.75,
                        stiffness: 0.25,
                        compression_length: None,
                        stretch: None,
                    }],
                },
                ConstraintSet::LocalRange {
                    reference_mesh_buffer: 0,
                    stiffness: 0.5,
                    shape_type: LocalRangeShape::Cylinder,
                    apply_normal_component: true,
                    constraints: vec![LocalRange {
                        particle: 0,
                        reference_vertex: 5,
                        maximum_distance: 0.4,
                        max_normal_distance: 0.3,
                        min_normal_distance: 0.2,
                    }],
                },
            ]
        );
    }
    Ok(())
}

#[test]
fn shape_type_zero_is_a_sphere_and_others_are_refused() -> Built<()> {
    let sphere = read(Options {
        shape_type: 0,
        ..FILES
    })?
    .unwrap();
    assert!(matches!(
        sphere.cloths[0].sim_cloths[0].constraint_sets[1],
        ConstraintSet::LocalRange {
            shape_type: LocalRangeShape::Sphere,
            ..
        }
    ));
    let e = read(Options {
        shape_type: 2,
        ..FILES
    })?
    .unwrap_err();
    assert!(e.contains("shape type 2"), "{e}");
    Ok(())
}

#[test]
fn particle_indices_of_another_width_or_past_the_particles_are_refused() -> Built<()> {
    let e = read(Options { wide: true, ..FILES })?.unwrap_err();
    assert!(e.contains("16-bit index") && e.contains("32-bit"), "{e}");
    let e = read(Options { link_b: 1, ..FILES })?.unwrap_err();
    assert!(e.contains("particle index 1") && e.contains("1 particles"), "{e}");
    let e = read(Options {
        local_particle: 1,
        ..FILES
    })?
    .unwrap_err();
    assert!(e.contains("particle index 1"), "{e}");
    Ok(())
}
