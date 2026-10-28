//! Havok cloth and ragdoll data of a tagfile, as typed values: the simulation cloths of a character's `hclClothData`
//! (their particles, fixed particles, rest poses and constraint sets) and the ragdoll (`hknpRagdollData`: the skeleton,
//! the map from bones to bodies and the body tags; and the constraints that join bodies, `hknpConstraintCinfo`).
//!
//! This is reading: every number is the file's, read by the names of the class metadata the file carries; a class or
//! field the file does not have is an error naming it, never a default. Objects are found by their class among the
//! file's items, so the order of the object graph does not matter. What the simulation does with these (the solver, the
//! operators that move a mesh by the particles, the states and transitions) is not here, and neither are a constraint's
//! atoms: a constraint set of a class this reader has no layout for is kept by its class name and size.
//!
//! The particle indices of a sim cloth (its fixed particles, a link's two particles, a local range's particle) are
//! 16-bit in the Sekiro and Elden Ring files, and each is checked against the sim cloth's particle count; a file that
//! stores them in another width, or one that names a particle it does not have, is refused.

use crate::animation::{Skeleton, skeleton};
use crate::{Result, Tagfile, Value, invalid};

/// `hclSimClothData::ParticleData`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    /// `mass`.
    pub mass: f32,
    /// `invMass`, as stored (nominally the reciprocal of `mass`).
    pub inv_mass: f32,
    /// `radius`.
    pub radius: f32,
    /// `friction`.
    pub friction: f32,
}

/// A rest pose of the particles (`hclSimClothPose`): one position (`x y z w`, as stored) a particle.
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    /// `name`.
    pub name: String,
    /// `positions`: `x y z` and the fourth lane as stored, one for each particle.
    pub positions: Vec<[f32; 4]>,
}

/// One link of a link constraint set. A standard or stretch link has the first four fields; a compressible link adds
/// `compression_length`; a bend link has `bendMinLength` and `bendStiffness` in the length and stiffness fields and adds
/// `stretchMaxLength` and `stretchStiffness`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Link {
    /// `particleA`: a particle index.
    pub particle_a: u16,
    /// `particleB`: a particle index.
    pub particle_b: u16,
    /// `restLength` (a bend link's `bendMinLength`).
    pub rest_length: f32,
    /// `stiffness` (a bend link's `bendStiffness`).
    pub stiffness: f32,
    /// A compressible link's `compressionLength`.
    pub compression_length: Option<f32>,
    /// A bend link's `stretchMaxLength` and `stretchStiffness`.
    pub stretch: Option<(f32, f32)>,
}

/// One entry of a local range constraint set (`hclLocalRangeConstraintSet::LocalConstraint`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalRange {
    /// `particleIndex`: a particle index.
    pub particle: u16,
    /// `referenceVertex`: the vertex index the particle is limited relative to (16-bit; a vertex of the reference mesh
    /// buffer, whose size this reader does not have, so unlike the particle indices it is not bounded).
    pub reference_vertex: u16,
    /// `maximumDistance`.
    pub maximum_distance: f32,
    /// `maxNormalDistance`.
    pub max_normal_distance: f32,
    /// `minNormalDistance`.
    pub min_normal_distance: f32,
}

/// The shape a local range constraint limits a particle to (`hclLocalRangeConstraintSet`'s `shapeType`, a 32-bit enum).
/// The values are the files': a set stores 0 (656 of 656 in Sekiro's cloth, 4,527 of 4,644 in Elden Ring's) or 1 (the
/// other 117); any other value is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalRangeShape {
    /// 0: a sphere around the reference vertex.
    Sphere,
    /// 1: a cylinder along the reference vertex's normal.
    Cylinder,
}

/// A constraint set of a sim cloth, by its class.
#[derive(Clone, Debug, PartialEq)]
pub enum ConstraintSet {
    /// `hclStandardLinkConstraintSet`, `hclStretchLinkConstraintSet`, `hclCompressibleLinkConstraintSet` or
    /// `hclBendLinkConstraintSet`: the class and its links.
    Links {
        /// The set's class name, which says what kind of links they are.
        class: String,
        /// `links`.
        links: Vec<Link>,
    },
    /// `hclLocalRangeConstraintSet`.
    LocalRange {
        /// `referenceMeshBufferIdx`: which mesh buffer holds the reference vertices.
        reference_mesh_buffer: u32,
        /// `stiffness`.
        stiffness: f32,
        /// `shapeType`.
        shape_type: LocalRangeShape,
        /// `applyNormalComponent`.
        apply_normal_component: bool,
        /// `localConstraints`.
        constraints: Vec<LocalRange>,
    },
    /// Any other class (bend stiffness, volume, transition ...): its name, kept unread.
    Unread {
        /// The set's class name.
        class: String,
    },
}

/// `hclSimClothData`.
#[derive(Clone, Debug, PartialEq)]
pub struct SimCloth {
    /// `name`.
    pub name: String,
    /// `particleDatas`.
    pub particles: Vec<Particle>,
    /// The particles that do not move (`fixedParticles`): indices into `particles`.
    pub fixed_particles: Vec<u16>,
    /// `simClothPoses`.
    pub poses: Vec<Pose>,
    /// `maxParticleRadius`.
    pub max_particle_radius: f32,
    /// `totalMass`.
    pub total_mass: f32,
    /// `staticConstraintSets`.
    pub constraint_sets: Vec<ConstraintSet>,
}

/// `hclClothData`: the cloth's name and its sim cloths.
#[derive(Clone, Debug, PartialEq)]
pub struct Cloth {
    /// `name`.
    pub name: String,
    /// `targetPlatform` (`hkEnum<Platform, hkUint32>`), as stored: 2 in all 610 `hclClothData` of Sekiro's and all 2,692 of
    /// Elden Ring's distinct tagfiles. The files do not say what the value means.
    pub target_platform: u32,
    /// `simClothDatas`.
    pub sim_cloths: Vec<SimCloth>,
}

/// `hknpConstraintCinfo`: a constraint joining two bodies. Its atoms are not read: `data_class` names the constraint's
/// kind (`hkpRagdollConstraintData`, `hkpLimitedHingeConstraintData` ...).
#[derive(Clone, Debug, PartialEq)]
pub struct Constraint {
    /// `name`.
    pub name: String,
    /// The class of the object `constraintData` points to, which says what kind of constraint it is.
    pub data_class: String,
    /// `bodyA`: a body id (`hknpBodyId`'s `serialAndIndex` when the file stores a record).
    pub body_a: u32,
    /// `bodyB`, as `body_a`.
    pub body_b: u32,
    /// `flags`, as stored.
    pub flags: u32,
    /// `desiredConstraintId`.
    pub desired_constraint_id: u32,
    /// `constraintGroupId`.
    pub constraint_group_id: u32,
}

/// `hknpRagdollData`.
#[derive(Clone, Debug, PartialEq)]
pub struct Ragdoll {
    /// `skeleton`.
    pub skeleton: Skeleton,
    /// One a bone: the body that moves it, as stored (negative: none).
    pub bone_to_body: Vec<i32>,
    /// `bodyTags`, as stored.
    pub body_tags: Vec<u32>,
}

/// What a file holds of cloth and ragdoll.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Physics {
    /// Every `hclClothData` of the file, in item order.
    pub cloths: Vec<Cloth>,
    /// Every `hknpRagdollData`, in item order.
    pub ragdolls: Vec<Ragdoll>,
    /// Every `hknpConstraintCinfo`, in item order.
    pub constraints: Vec<Constraint>,
}

fn list<'t, 'a, T>(v: Value<'t, 'a>, each: impl Fn(Value<'t, 'a>) -> Result<T>) -> Result<Vec<T>> {
    let a = v.array()?;
    (0..a.len()).map(|i| each(a.get(i)?)).collect()
}

fn small(v: Value<'_, '_>) -> Result<u32> {
    // Havok 2018 stores these ids as reflected records; 2016 may expose the
    // integer directly. Read only the established fields, never a record's first bytes.
    let v = match (v.format()? & 15, v.type_name()) {
        (7, "hknpBodyId") => v.field("serialAndIndex")?,
        (7, "hknpConstraintId" | "hknpConstraintGroupId" | "hkHandle") => v.field("value")?,
        _ => v,
    };
    u32::try_from(v.unsigned()?).map_err(|_| invalid(v.at(), "value outside u32"))
}

/// A 16-bit index stored as `hkUint16`; another width is refused.
fn index16(v: Value<'_, '_>) -> Result<u16> {
    let value = v.unsigned()?;
    let bits = v.format()? >> 10;
    if bits != 16 {
        return Err(invalid(
            v.at(),
            format!("expected a 16-bit index, found a {bits}-bit {}", v.type_name()),
        ));
    }
    u16::try_from(value).map_err(|_| invalid(v.at(), "value outside u16"))
}

/// A 16-bit particle index that names one of the sim cloth's `count` particles.
fn particle(v: Value<'_, '_>, count: usize) -> Result<u16> {
    let index = index16(v)?;
    if usize::from(index) >= count {
        return Err(invalid(
            v.at(),
            format!("particle index {index} of a sim cloth with {count} particles"),
        ));
    }
    Ok(index)
}

fn small_signed(v: Value<'_, '_>) -> Result<i32> {
    i32::try_from(v.signed()?).map_err(|_| invalid(v.at(), "value outside i32"))
}

/// Every object of class `class` among the file's items, in item order.
fn objects<'t, 'a>(tag: &'t Tagfile<'a>, class: &str) -> Result<Vec<Value<'t, 'a>>> {
    let mut out = Vec::new();
    for (index, item) in tag.items.iter().enumerate().skip(1) {
        if tag.types.get(item.type_index).is_some_and(|t| t.name == class) {
            let array = tag.item(index)?;
            for i in 0..array.len() {
                out.push(array.get(i)?);
            }
        }
    }
    Ok(out)
}

/// Which fields a link class stores.
#[derive(Clone, Copy)]
enum LinkKind {
    Plain,
    Compressible,
    Bend,
}

fn link(v: Value<'_, '_>, kind: LinkKind, particles: usize) -> Result<Link> {
    let (rest, stiffness) = match kind {
        LinkKind::Bend => ("bendMinLength", "bendStiffness"),
        LinkKind::Plain | LinkKind::Compressible => ("restLength", "stiffness"),
    };
    Ok(Link {
        particle_a: particle(v.field("particleA")?, particles)?,
        particle_b: particle(v.field("particleB")?, particles)?,
        rest_length: v.field(rest)?.float()?,
        stiffness: v.field(stiffness)?.float()?,
        compression_length: match kind {
            LinkKind::Compressible => Some(v.field("compressionLength")?.float()?),
            LinkKind::Plain | LinkKind::Bend => None,
        },
        stretch: match kind {
            LinkKind::Bend => Some((
                v.field("stretchMaxLength")?.float()?,
                v.field("stretchStiffness")?.float()?,
            )),
            LinkKind::Plain | LinkKind::Compressible => None,
        },
    })
}

fn local_range_shape(v: Value<'_, '_>) -> Result<LocalRangeShape> {
    match v.unsigned()? {
        0 => Ok(LocalRangeShape::Sphere),
        1 => Ok(LocalRangeShape::Cylinder),
        other => Err(invalid(
            v.at(),
            format!("local range shape type {other}, not 0 or 1"),
        )),
    }
}

fn constraint_set(v: Value<'_, '_>, particles: usize) -> Result<ConstraintSet> {
    let class = v.type_name().to_owned();
    Ok(match class.as_str() {
        "hclStandardLinkConstraintSet" | "hclStretchLinkConstraintSet" => ConstraintSet::Links {
            links: list(v.field("links")?, |l| link(l, LinkKind::Plain, particles))?,
            class,
        },
        "hclCompressibleLinkConstraintSet" => ConstraintSet::Links {
            links: list(v.field("links")?, |l| link(l, LinkKind::Compressible, particles))?,
            class,
        },
        "hclBendLinkConstraintSet" => ConstraintSet::Links {
            links: list(v.field("links")?, |l| link(l, LinkKind::Bend, particles))?,
            class,
        },
        "hclLocalRangeConstraintSet" => ConstraintSet::LocalRange {
            reference_mesh_buffer: small(v.field("referenceMeshBufferIdx")?)?,
            stiffness: v.field("stiffness")?.float()?,
            shape_type: local_range_shape(v.field("shapeType")?)?,
            apply_normal_component: v.field("applyNormalComponent")?.unsigned()? != 0,
            constraints: list(v.field("localConstraints")?, |c| {
                Ok(LocalRange {
                    particle: particle(c.field("particleIndex")?, particles)?,
                    reference_vertex: index16(c.field("referenceVertex")?)?,
                    maximum_distance: c.field("maximumDistance")?.float()?,
                    max_normal_distance: c.field("maxNormalDistance")?.float()?,
                    min_normal_distance: c.field("minNormalDistance")?.float()?,
                })
            })?,
        },
        _ => ConstraintSet::Unread { class },
    })
}

fn sim_cloth(v: Value<'_, '_>) -> Result<SimCloth> {
    let particles = list(v.field("particleDatas")?, |p| {
        Ok(Particle {
            mass: p.field("mass")?.float()?,
            inv_mass: p.field("invMass")?.float()?,
            radius: p.field("radius")?.float()?,
            friction: p.field("friction")?.float()?,
        })
    })?;
    let count = particles.len();
    Ok(SimCloth {
        name: v.field("name")?.string()?,
        particles,
        fixed_particles: list(v.field("fixedParticles")?, |p| particle(p, count))?,
        poses: list(v.field("simClothPoses")?, |p| {
            let p = p.dereference()?;
            Ok(Pose {
                name: p.field("name")?.string()?,
                positions: list(p.field("positions")?, |x| x.vector::<4>())?,
            })
        })?,
        max_particle_radius: v.field("maxParticleRadius")?.float()?,
        total_mass: v.field("totalMass")?.float()?,
        constraint_sets: list(v.field("staticConstraintSets")?, |s| {
            constraint_set(s.dereference()?, count)
        })?,
    })
}

/// The cloth, ragdoll and constraint objects of `tag`.
pub fn read(tag: &Tagfile<'_>) -> Result<Physics> {
    let mut out = Physics::default();
    for data in objects(tag, "hclClothData")? {
        out.cloths.push(Cloth {
            name: data.field("name")?.string()?,
            target_platform: u32::try_from(data.field("targetPlatform")?.unsigned()?)
                .map_err(|_| invalid(data.at(), "targetPlatform outside u32"))?,
            sim_cloths: list(data.field("simClothDatas")?, |s| sim_cloth(s.dereference()?))?,
        });
    }
    for data in objects(tag, "hknpRagdollData")? {
        let skeleton = skeleton(data.field("skeleton")?.dereference()?)?;
        let bone_to_body = list(data.field("boneToBodyMap")?, small_signed)?;
        if bone_to_body.len() != skeleton.bones.len() {
            return Err(invalid(
                data.at(),
                format!(
                    "{} bones, {} body map entries",
                    skeleton.bones.len(),
                    bone_to_body.len()
                ),
            ));
        }
        out.ragdolls.push(Ragdoll {
            skeleton,
            bone_to_body,
            body_tags: list(data.field("bodyTags")?, small)?,
        });
    }
    for c in objects(tag, "hknpConstraintCinfo")? {
        out.constraints.push(Constraint {
            name: c.field("name")?.string()?,
            data_class: c.field("constraintData")?.dereference()?.type_name().to_owned(),
            body_a: small(c.field("bodyA")?)?,
            body_b: small(c.field("bodyB")?)?,
            flags: small(c.field("flags")?)?,
            desired_constraint_id: small(c.field("desiredConstraintId")?)?,
            constraint_group_id: small(c.field("constraintGroupId")?)?,
        });
    }
    Ok(out)
}
