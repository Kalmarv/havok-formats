//! The constraint data of ragdolls and other joints in tagfiles: the `hkp*ConstraintData` objects an
//! `hknpConstraintCinfo` points to, with their atoms typed. The classes are the ones the Sekiro (2016.2) and Elden Ring
//! (2018.1) character, part, asset and object files hold: ragdoll, limited hinge, hinge, ball and socket, fixed and wheel
//! data, and the breakable wrapper (`hknpBreakableConstraintData`) around one of them.
//!
//! An atom is one rule of a constraint (a pair of local frames, a limit, a motor, a friction), stored as a class with a
//! `type` word that names the rule. [`read_constraint_data`] checks every `type` against the rule its slot holds (`2` for
//! `transforms`, `23` for `setupStabilization`, `19` for `ragdollMotors`, `17` for `angFriction`, `15` for `twistLimit`,
//! `16` for `coneLimit` and `planesLimit`, `5` for `ballSocket`, `18` for `angMotor`, `14` for `angLimit`, `12` for
//! `2dAng`, `3` for `pivots`, `24` for `ang`, `4` for `steeringBase`, `7` for `lin1` and `lin2`, `8` for `lin0Soft` and `9`
//! for `lin0Limit`), the values every one of the 2,499 Sekiro and 7,934 Elden Ring constraint data objects of the
//! installs hold (each tagfile counted as often as a binder repeats it). Their integer members are constant per class
//! in both games (a census over all of them): every `isEnabled` is 1 but the motors', every motor pointer is set or
//! null, the axes are fixed per slot (`twistAxis` 0, `refAxis` 1, the cone's `refAxisInB` 0 and the planes limit's 1, a
//! hinge's `limitAxis` 0 and `cosineAxis` 1) and `velocityStabilizationFactor` is 48. Beyond that the values are the
//! file's: the angles are radians, an axis is the index of a column of the frame.
#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]
use crate::{Result, Value, invalid};

/// A rigid transform: the three columns of the rotation and the translation. Each stored column has a fourth lane
/// that is not read: it is zero in a minority of the transforms and arbitrary in the rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// The rotation's columns (the images of the x, y and z axes). Orthonormal and right-handed in every stored
    /// transform (the worst error of a column's length or of two columns' dot product is 1.2e-6).
    pub columns: [[f32; 3]; 3],
    /// The translation.
    pub translation: [f32; 3],
}

/// `hkpSetLocalTransformsConstraintAtom`: the constraint frame in each body's local space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalTransforms {
    /// The frame in body A's space.
    pub transform_a: Transform,
    /// The frame in body B's space.
    pub transform_b: Transform,
}

/// `hkpSetupStabilizationAtom`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SetupStabilization {
    /// Whether it is on: 1 in every constraint that is not wrapped in a breakable one, 0 in all 169 Elden Ring and 3
    /// Sekiro breakable ones.
    pub enabled: bool,
    /// `maxLinImpulse`.
    pub max_lin_impulse: f32,
    /// `maxAngImpulse`.
    pub max_ang_impulse: f32,
    /// `maxAngle`.
    pub max_angle: f32,
}

/// `hkpBallSocketConstraintAtom`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BallSocket {
    /// `solvingMethod`.
    pub solving_method: u8,
    /// `bodiesToNotify`.
    pub bodies_to_notify: u8,
    /// `velocityStabilizationFactor` (an `hkUFloat8`), as the byte stored.
    pub velocity_stabilization_factor: u8,
    /// `enableLinearImpulseLimit`.
    pub enable_linear_impulse_limit: bool,
    /// `breachImpulse`.
    pub breach_impulse: f32,
    /// `inertiaStabilizationFactor`.
    pub inertia_stabilization_factor: f32,
}

/// `hkpAngFrictionConstraintAtom`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AngFriction {
    /// `isEnabled`.
    pub enabled: bool,
    /// `firstFrictionAxis`.
    pub first_friction_axis: u8,
    /// `numFrictionAxes`.
    pub num_friction_axes: u8,
    /// `maxFrictionTorque`.
    pub max_friction_torque: f32,
}

/// `hkpTwistLimitConstraintAtom`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwistLimit {
    /// `isEnabled`.
    pub enabled: bool,
    /// `twistAxis`.
    pub twist_axis: u8,
    /// `refAxis`.
    pub ref_axis: u8,
    /// `minAngle`; at most `max_angle` in every stored limit.
    pub min_angle: f32,
    /// `maxAngle`.
    pub max_angle: f32,
    /// `angularLimitsTauFactor`.
    pub tau_factor: f32,
    /// `angularLimitsDampFactor`.
    pub damp_factor: f32,
}

/// `hkpConeLimitConstraintAtom`. A ragdoll holds two: the cone itself and the planes limit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConeLimit {
    /// `isEnabled`.
    pub enabled: bool,
    /// `twistAxisInA`.
    pub twist_axis_in_a: u8,
    /// `refAxisInB`.
    pub ref_axis_in_b: u8,
    /// `angleMeasurementMode`.
    pub angle_measurement_mode: u8,
    /// `minAngle`; at most `max_angle` in every stored limit.
    pub min_angle: f32,
    /// `maxAngle`.
    pub max_angle: f32,
    /// `angularLimitsTauFactor`.
    pub tau_factor: f32,
    /// `angularLimitsDampFactor`.
    pub damp_factor: f32,
}

/// `hkpAngLimitConstraintAtom`: a hinge's angle limit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AngLimit {
    /// `isEnabled`.
    pub enabled: bool,
    /// `limitAxis`.
    pub limit_axis: u8,
    /// `cosineAxis`.
    pub cosine_axis: u8,
    /// `minAngle`; at most `max_angle` in every stored limit.
    pub min_angle: f32,
    /// `maxAngle`.
    pub max_angle: f32,
    /// `angularLimitsTauFactor`.
    pub tau_factor: f32,
    /// `angularLimitsDampFactor`.
    pub damp_factor: f32,
}

/// `hkpPositionConstraintMotor`: the motor a motor atom points to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionMotor {
    /// `type`.
    pub kind: u8,
    /// `minForce`.
    pub min_force: f32,
    /// `maxForce`.
    pub max_force: f32,
    /// `tau`.
    pub tau: f32,
    /// `damping`.
    pub damping: f32,
    /// `proportionalRecoveryVelocity`.
    pub proportional_recovery_velocity: f32,
    /// `constantRecoveryVelocity`.
    pub constant_recovery_velocity: f32,
}

/// `hkpAngMotorConstraintAtom`: a hinge's motor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AngMotor {
    /// `isEnabled`: 0 in every stored motor.
    pub enabled: bool,
    /// `motorAxis`.
    pub motor_axis: u8,
    /// `targetAngle`.
    pub target_angle: f32,
    /// The motor the atom points to (none for a null pointer).
    pub motor: Option<PositionMotor>,
}

/// `hkpRagdollMotorConstraintAtom`: the three motors of a ragdoll joint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RagdollMotors {
    /// `isEnabled`: 0 in every stored atom.
    pub enabled: bool,
    /// `target_bRca`: the target orientation as a 3 x 3 matrix, column by column with the fourth lane of each.
    pub target_b_rca: [f32; 12],
    /// The three motors (`motors`); none for a null pointer.
    pub motors: [Option<PositionMotor>; 3],
}

/// What the atoms of a ragdoll joint (`hkpRagdollConstraintData`) are, in the order the class stores them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ragdoll {
    /// `transforms`.
    pub transforms: LocalTransforms,
    /// `setupStabilization`.
    pub setup_stabilization: SetupStabilization,
    /// `ragdollMotors`.
    pub motors: RagdollMotors,
    /// `angFriction`.
    pub friction: AngFriction,
    /// `twistLimit`.
    pub twist: TwistLimit,
    /// `coneLimit`.
    pub cone: ConeLimit,
    /// `planesLimit`.
    pub planes: ConeLimit,
    /// `ballSocket`.
    pub ball_socket: BallSocket,
}

/// A limited hinge (`hkpLimitedHingeConstraintData`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimitedHinge {
    /// `transforms`.
    pub transforms: LocalTransforms,
    /// `setupStabilization`.
    pub setup_stabilization: SetupStabilization,
    /// `angMotor`.
    pub motor: AngMotor,
    /// `angFriction`.
    pub friction: AngFriction,
    /// `angLimit`.
    pub limit: AngLimit,
    /// `2dAng`'s `freeRotationAxis`.
    pub free_rotation_axis: u8,
    /// `ballSocket`.
    pub ball_socket: BallSocket,
}

/// A hinge (`hkpHingeConstraintData`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hinge {
    /// `transforms`.
    pub transforms: LocalTransforms,
    /// `setupStabilization`.
    pub setup_stabilization: SetupStabilization,
    /// `2dAng`'s `freeRotationAxis`.
    pub free_rotation_axis: u8,
    /// `ballSocket`.
    pub ball_socket: BallSocket,
}

/// A ball and socket joint (`hkpBallAndSocketConstraintData`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BallAndSocket {
    /// `pivots`: the pivot in body A's space.
    pub translation_a: [f32; 3],
    /// `pivots`: the pivot in body B's space.
    pub translation_b: [f32; 3],
    /// `setupStabilization`.
    pub setup_stabilization: SetupStabilization,
    /// `ballSocket`.
    pub ball_socket: BallSocket,
}

/// A fixed joint (`hkpFixedConstraintData`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fixed {
    /// `transforms`.
    pub transforms: LocalTransforms,
    /// `setupStabilization`.
    pub setup_stabilization: SetupStabilization,
    /// `ballSocket`.
    pub ball_socket: BallSocket,
}

/// A wheel (`hkpWheelConstraintData`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wheel {
    /// `suspensionBase`.
    pub suspension_base: LocalTransforms,
    /// `lin0Limit`: the suspension travel along its axis.
    pub suspension_axis: u8,
    /// `lin0Limit`'s `min`.
    pub suspension_min: f32,
    /// `lin0Limit`'s `max`.
    pub suspension_max: f32,
    /// `lin0Soft`'s `tau`.
    pub suspension_tau: f32,
    /// `lin0Soft`'s `damping`.
    pub suspension_damping: f32,
    /// `steeringBase`'s `rotationA`, three columns with the fourth lane of each.
    pub steering_rotation_a: [f32; 12],
    /// `steeringBase`'s `rotationB`.
    pub steering_rotation_b: [f32; 12],
    /// `2dAng`'s `freeRotationAxis`.
    pub free_rotation_axis: u8,
    /// `initialAxleInB`.
    pub initial_axle_in_b: [f32; 3],
    /// `initialSteeringAxisInB`.
    pub initial_steering_axis_in_b: [f32; 3],
}

/// The constraint data an `hknpConstraintCinfo` points to.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstraintData {
    /// `hkpRagdollConstraintData`.
    Ragdoll(Ragdoll),
    /// `hkpLimitedHingeConstraintData`.
    LimitedHinge(LimitedHinge),
    /// `hkpHingeConstraintData`.
    Hinge(Hinge),
    /// `hkpBallAndSocketConstraintData`.
    BallAndSocket(BallAndSocket),
    /// `hkpFixedConstraintData`.
    Fixed(Fixed),
    /// `hkpWheelConstraintData`.
    Wheel(Wheel),
    /// `hknpBreakableConstraintData`: the constraint it wraps, which gives way at `threshold`.
    Breakable {
        /// The wrapped constraint (a ragdoll, limited hinge or ball and socket joint in the installs).
        constraint: Box<ConstraintData>,
        /// `threshold`.
        threshold: f32,
    },
}

/// An unsigned member that fits a byte.
fn byte(v: Value<'_, '_>, name: &str) -> Result<u8> {
    let field = v.field(name)?;
    u8::try_from(field.unsigned()?).map_err(|_| invalid(field.at(), format!("{name} does not fit a byte")))
}

/// A boolean member: 0 or 1.
fn flag(v: Value<'_, '_>, name: &str) -> Result<bool> {
    let field = v.field(name)?;
    match field.unsigned()? {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(invalid(field.at(), format!("{name} is {other}, not a flag"))),
    }
}

/// A float member; the fourth lane or a limit may be a huge value, but a NaN is refused.
fn real(v: Value<'_, '_>, name: &str) -> Result<f32> {
    let field = v.field(name)?;
    let x = field.float()?;
    if x.is_nan() {
        return Err(invalid(field.at(), format!("{name} is not a number")));
    }
    Ok(x)
}

/// The atom `name` of `atoms`, refused unless its `type` is `want`.
fn atom<'t, 'a>(atoms: Value<'t, 'a>, name: &str, want: u64) -> Result<Value<'t, 'a>> {
    let atom = atoms.field(name)?;
    let found = atom.field("type")?.unsigned()?;
    if found != want {
        return Err(invalid(
            atom.at(),
            format!("atom {name} has type {found}, not {want}"),
        ));
    }
    Ok(atom)
}

/// A member of `n` floats, none a NaN.
fn floats<const N: usize>(v: Value<'_, '_>, name: &str) -> Result<[f32; N]> {
    let field = v.field(name)?;
    let x = field.vector::<N>()?;
    if x.iter().any(|f| f.is_nan()) {
        return Err(invalid(
            field.at(),
            format!("{name} holds a number that is not one"),
        ));
    }
    Ok(x)
}

/// The rotation columns and translation of an `hkTransform` (16 floats).
fn transform(v: Value<'_, '_>, name: &str) -> Result<Transform> {
    let [a0, a1, a2, _, b0, b1, b2, _, c0, c1, c2, _, t0, t1, t2, _] = floats::<16>(v, name)?;
    Ok(Transform {
        columns: [[a0, a1, a2], [b0, b1, b2], [c0, c1, c2]],
        translation: [t0, t1, t2],
    })
}

fn local_transforms(atoms: Value<'_, '_>, name: &str) -> Result<LocalTransforms> {
    let a = atom(atoms, name, 2)?;
    Ok(LocalTransforms {
        transform_a: transform(a, "transformA")?,
        transform_b: transform(a, "transformB")?,
    })
}

fn setup_stabilization(atoms: Value<'_, '_>) -> Result<SetupStabilization> {
    let a = atom(atoms, "setupStabilization", 23)?;
    Ok(SetupStabilization {
        enabled: flag(a, "enabled")?,
        max_lin_impulse: real(a, "maxLinImpulse")?,
        max_ang_impulse: real(a, "maxAngImpulse")?,
        max_angle: real(a, "maxAngle")?,
    })
}

fn ball_socket(atoms: Value<'_, '_>) -> Result<BallSocket> {
    let a = atom(atoms, "ballSocket", 5)?;
    let factor = a.field("velocityStabilizationFactor")?;
    Ok(BallSocket {
        solving_method: byte(a, "solvingMethod")?,
        bodies_to_notify: byte(a, "bodiesToNotify")?,
        velocity_stabilization_factor: byte(factor, "value")?,
        enable_linear_impulse_limit: flag(a, "enableLinearImpulseLimit")?,
        breach_impulse: real(a, "breachImpulse")?,
        inertia_stabilization_factor: real(a, "inertiaStabilizationFactor")?,
    })
}

fn ang_friction(atoms: Value<'_, '_>) -> Result<AngFriction> {
    let a = atom(atoms, "angFriction", 17)?;
    Ok(AngFriction {
        enabled: flag(a, "isEnabled")?,
        first_friction_axis: byte(a, "firstFrictionAxis")?,
        num_friction_axes: byte(a, "numFrictionAxes")?,
        max_friction_torque: real(a, "maxFrictionTorque")?,
    })
}

fn twist_limit(atoms: Value<'_, '_>) -> Result<TwistLimit> {
    let a = atom(atoms, "twistLimit", 15)?;
    Ok(TwistLimit {
        enabled: flag(a, "isEnabled")?,
        twist_axis: byte(a, "twistAxis")?,
        ref_axis: byte(a, "refAxis")?,
        min_angle: real(a, "minAngle")?,
        max_angle: real(a, "maxAngle")?,
        tau_factor: real(a, "angularLimitsTauFactor")?,
        damp_factor: real(a, "angularLimitsDampFactor")?,
    })
}

fn cone_limit(atoms: Value<'_, '_>, name: &str) -> Result<ConeLimit> {
    let a = atom(atoms, name, 16)?;
    Ok(ConeLimit {
        enabled: flag(a, "isEnabled")?,
        twist_axis_in_a: byte(a, "twistAxisInA")?,
        ref_axis_in_b: byte(a, "refAxisInB")?,
        angle_measurement_mode: byte(a, "angleMeasurementMode")?,
        min_angle: real(a, "minAngle")?,
        max_angle: real(a, "maxAngle")?,
        tau_factor: real(a, "angularLimitsTauFactor")?,
        damp_factor: real(a, "angularLimitsDampFactor")?,
    })
}

fn ang_limit(atoms: Value<'_, '_>) -> Result<AngLimit> {
    let a = atom(atoms, "angLimit", 14)?;
    Ok(AngLimit {
        enabled: flag(a, "isEnabled")?,
        limit_axis: byte(a, "limitAxis")?,
        cosine_axis: byte(a, "cosineAxis")?,
        min_angle: real(a, "minAngle")?,
        max_angle: real(a, "maxAngle")?,
        tau_factor: real(a, "angularLimitsTauFactor")?,
        damp_factor: real(a, "angularLimitsDampFactor")?,
    })
}

/// The motor a pointer slot refers to: `None` for a null pointer, else an `hkpPositionConstraintMotor`.
fn motor(slot: Value<'_, '_>) -> Result<Option<PositionMotor>> {
    let Some(m) = slot.pointer()? else {
        return Ok(None);
    };
    if m.type_name() != "hkpPositionConstraintMotor" {
        return Err(invalid(
            m.at(),
            format!("a constraint motor of class {}", m.type_name()),
        ));
    }
    Ok(Some(PositionMotor {
        kind: byte(m, "type")?,
        min_force: real(m, "minForce")?,
        max_force: real(m, "maxForce")?,
        tau: real(m, "tau")?,
        damping: real(m, "damping")?,
        proportional_recovery_velocity: real(m, "proportionalRecoveryVelocity")?,
        constant_recovery_velocity: real(m, "constantRecoveryVelocity")?,
    }))
}

fn ang_motor(atoms: Value<'_, '_>) -> Result<AngMotor> {
    let a = atom(atoms, "angMotor", 18)?;
    Ok(AngMotor {
        enabled: flag(a, "isEnabled")?,
        motor_axis: byte(a, "motorAxis")?,
        target_angle: real(a, "targetAngle")?,
        motor: motor(a.field("motor")?)?,
    })
}

fn ragdoll_motors(atoms: Value<'_, '_>) -> Result<RagdollMotors> {
    let a = atom(atoms, "ragdollMotors", 19)?;
    let slots = a.field("motors")?.array()?;
    if slots.len() != 3 {
        return Err(invalid(a.at(), format!("{} ragdoll motors, not 3", slots.len())));
    }
    Ok(RagdollMotors {
        enabled: flag(a, "isEnabled")?,
        target_b_rca: floats::<12>(a, "target_bRca")?,
        motors: [
            motor(slots.get(0)?)?,
            motor(slots.get(1)?)?,
            motor(slots.get(2)?)?,
        ],
    })
}

/// `2dAng`'s `freeRotationAxis`.
fn free_rotation_axis(atoms: Value<'_, '_>) -> Result<u8> {
    byte(atom(atoms, "2dAng", 12)?, "freeRotationAxis")
}

/// Read the constraint data object `data` (the target of an `hknpConstraintCinfo`'s `constraintData`).
///
/// Refused: a class that is not one of the seven above, an atom whose `type` is not the rule of its slot, a flag that is
/// not 0 or 1, a number that is not one, a motor that is not an `hkpPositionConstraintMotor`, a breakable constraint that
/// wraps another breakable one. Every one of the constraint data objects of the Sekiro and Elden Ring installs reads.
pub fn read_constraint_data(data: Value<'_, '_>) -> Result<ConstraintData> {
    read_at(data, 0)
}

fn read_at(data: Value<'_, '_>, depth: usize) -> Result<ConstraintData> {
    let class = data.type_name();
    if class == "hknpBreakableConstraintData" {
        if depth > 0 {
            return Err(invalid(
                data.at(),
                "a breakable constraint inside a breakable constraint",
            ));
        }
        let inner = data.field("constraintData")?.dereference()?;
        return Ok(ConstraintData::Breakable {
            constraint: Box::new(read_at(inner, depth.saturating_add(1))?),
            threshold: real(data, "threshold")?,
        });
    }
    let atoms = data.field("atoms")?;
    Ok(match class {
        "hkpRagdollConstraintData" => ConstraintData::Ragdoll(Ragdoll {
            transforms: local_transforms(atoms, "transforms")?,
            setup_stabilization: setup_stabilization(atoms)?,
            motors: ragdoll_motors(atoms)?,
            friction: ang_friction(atoms)?,
            twist: twist_limit(atoms)?,
            cone: cone_limit(atoms, "coneLimit")?,
            planes: cone_limit(atoms, "planesLimit")?,
            ball_socket: ball_socket(atoms)?,
        }),
        "hkpLimitedHingeConstraintData" => ConstraintData::LimitedHinge(LimitedHinge {
            transforms: local_transforms(atoms, "transforms")?,
            setup_stabilization: setup_stabilization(atoms)?,
            motor: ang_motor(atoms)?,
            friction: ang_friction(atoms)?,
            limit: ang_limit(atoms)?,
            free_rotation_axis: free_rotation_axis(atoms)?,
            ball_socket: ball_socket(atoms)?,
        }),
        "hkpHingeConstraintData" => ConstraintData::Hinge(Hinge {
            transforms: local_transforms(atoms, "transforms")?,
            setup_stabilization: setup_stabilization(atoms)?,
            free_rotation_axis: free_rotation_axis(atoms)?,
            ball_socket: ball_socket(atoms)?,
        }),
        "hkpBallAndSocketConstraintData" => {
            let pivots = atom(atoms, "pivots", 3)?;
            let [ax, ay, az, _] = floats::<4>(pivots, "translationA")?;
            let [bx, by, bz, _] = floats::<4>(pivots, "translationB")?;
            ConstraintData::BallAndSocket(BallAndSocket {
                translation_a: [ax, ay, az],
                translation_b: [bx, by, bz],
                setup_stabilization: setup_stabilization(atoms)?,
                ball_socket: ball_socket(atoms)?,
            })
        }
        "hkpFixedConstraintData" => {
            // the `ang` atom holds nothing but its type
            atom(atoms, "ang", 24)?;
            ConstraintData::Fixed(Fixed {
                transforms: local_transforms(atoms, "transforms")?,
                setup_stabilization: setup_stabilization(atoms)?,
                ball_socket: ball_socket(atoms)?,
            })
        }
        "hkpWheelConstraintData" => {
            let limit = atom(atoms, "lin0Limit", 9)?;
            let soft = atom(atoms, "lin0Soft", 8)?;
            atom(atoms, "lin1", 7)?;
            atom(atoms, "lin2", 7)?;
            let steering = atom(atoms, "steeringBase", 4)?;
            let [ax, ay, az, _] = floats::<4>(data, "initialAxleInB")?;
            let [sx, sy, sz, _] = floats::<4>(data, "initialSteeringAxisInB")?;
            ConstraintData::Wheel(Wheel {
                suspension_base: local_transforms(atoms, "suspensionBase")?,
                suspension_axis: byte(limit, "axisIndex")?,
                suspension_min: real(limit, "min")?,
                suspension_max: real(limit, "max")?,
                suspension_tau: real(soft, "tau")?,
                suspension_damping: real(soft, "damping")?,
                steering_rotation_a: floats::<12>(steering, "rotationA")?,
                steering_rotation_b: floats::<12>(steering, "rotationB")?,
                free_rotation_axis: free_rotation_axis(atoms)?,
                initial_axle_in_b: [ax, ay, az],
                initial_steering_axis_in_b: [sx, sy, sz],
            })
        }
        other => {
            return Err(invalid(
                data.at(),
                format!("no reader for the constraint data class {other}"),
            ));
        }
    })
}
