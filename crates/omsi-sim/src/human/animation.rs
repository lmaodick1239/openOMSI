//! Animation state introspection for the human skeletal animation system.
//!
//! Provides snapshot APIs for capturing animation state, active clips, playback phase,
//! and skeletal pose data without mutating simulation state.

use glam::{Quat, Vec3};
use super::legacy::{Activity, Pose, Posed, Rig};

/// Active animation clip metadata.
#[derive(Debug, Clone)]
pub struct AnimationClip {
    /// Animation clip name (e.g., "walk", "sit_down", "idle").
    pub name: String,
    /// Normalized phase time [0.0..1.0] within the animation cycle.
    pub phase: f32,
    /// Playback speed scalar (1.0 = normal speed).
    pub speed: f32,
    /// Blend weight for this animation [0.0..1.0].
    pub weight: f32,
}

/// Animation state machine telemetry.
#[derive(Debug, Clone)]
pub struct AnimationState {
    /// Currently active animation clips.
    pub active_clips: Vec<AnimationClip>,
    /// Current activity driving the animation.
    pub activity: Activity,
    /// Walking gait cycle phase [0.0..1.0] (left heel strike at 0).
    pub gait_phase: f32,
    /// Walking weight for upper body blending [0.0..1.0].
    pub walk_weight: f32,
    /// Sitting weight [0.0..1.0].
    pub sit_weight: f32,
    /// Current ground speed (m/s).
    pub speed: f32,
    /// Forward acceleration (m/s²).
    pub accel: f32,
    /// Turning rate (deg/s).
    pub turn_rate: f32,
}

impl AnimationState {
    /// Capture animation state from a pose.
    pub fn from_pose(pose: &Pose, activity: Activity) -> Self {
        let mut clips = Vec::new();

        // Map internal pose state to animation clips
        let walk_weight = pose.walk_weight();
        if walk_weight > 0.01 {
            clips.push(AnimationClip {
                name: "walk".to_string(),
                phase: pose.gait_phase(),
                speed: 1.0,
                weight: walk_weight,
            });
        }

        let sit_weight = pose.sit_weight();
        if sit_weight > 0.01 && sit_weight < 0.99 {
            clips.push(AnimationClip {
                name: "sit_down".to_string(),
                phase: sit_weight,
                speed: 1.0,
                weight: 1.0,
            });
        }

        if walk_weight < 0.01 && sit_weight < 0.01 {
            clips.push(AnimationClip {
                name: "idle".to_string(),
                phase: pose.breath_phase(),
                speed: 1.0,
                weight: 1.0,
            });
        }

        AnimationState {
            active_clips: clips,
            activity,
            gait_phase: pose.gait_phase(),
            walk_weight,
            sit_weight,
            speed: pose.ground_speed(),
            accel: pose.forward_accel(),
            turn_rate: pose.turn_rate(),
        }
    }
}

/// A single bone transform with position and rotation.
#[derive(Debug, Clone)]
pub struct BoneTransform {
    /// Bone name (e.g., "Spine", "LeftUpperArm", "RightKnee").
    pub name: String,
    /// Bone position in model space.
    pub position: Vec3,
    /// Bone rotation as quaternion.
    pub rotation: Quat,
    /// Euler angles (degrees): [pitch, yaw, roll].
    pub euler_angles: Vec3,
}

impl BoneTransform {
    /// Create a bone transform from an affine transform.
    pub fn from_affine(name: String, transform: &glam::Affine3A) -> Self {
        let position = Vec3::from(transform.translation);
        let mat3 = glam::Mat3::from_cols(
            transform.matrix3.x_axis.into(),
            transform.matrix3.y_axis.into(),
            transform.matrix3.z_axis.into(),
        );
        let rotation = Quat::from_mat3(&mat3);
        let euler = rotation.to_euler(glam::EulerRot::XYZ);
        let euler_degrees = Vec3::new(
            euler.0.to_degrees(),
            euler.1.to_degrees(),
            euler.2.to_degrees(),
        );

        BoneTransform {
            name,
            position,
            rotation,
            euler_angles: euler_degrees,
        }
    }
}

/// Extract skeleton bone transforms from a posed human.
pub fn extract_skeleton(posed: &Posed, _rig: &Rig) -> Vec<BoneTransform> {
    use super::legacy::{HIP, MAIN, HEAD, THIGH, SHIN, FOOT, UPPER, FORE, HAND};
    
    let bone_names = [
        ("Hip", HIP),
        ("Spine", MAIN),
        ("Head", HEAD),
        ("LeftThigh", THIGH[0]),
        ("RightThigh", THIGH[1]),
        ("LeftShin", SHIN[0]),
        ("RightShin", SHIN[1]),
        ("LeftFoot", FOOT[0]),
        ("RightFoot", FOOT[1]),
        ("LeftUpperArm", UPPER[0]),
        ("RightUpperArm", UPPER[1]),
        ("LeftForearm", FORE[0]),
        ("RightForearm", FORE[1]),
        ("LeftHand", HAND[0]),
        ("RightHand", HAND[1]),
    ];

    bone_names
        .iter()
        .map(|(name, slot)| {
            BoneTransform::from_affine(name.to_string(), &posed.bones[*slot])
        })
        .collect()
}

/// Check for animation artifacts (foot sliding, clipping).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationArtifact {
    /// Foot sliding detected (ground speed mismatch).
    FootSliding { side: usize, velocity: f32 },
    /// Limb clipping through geometry.
    Clipping { bone_index: usize },
    /// Unreachable IK target.
    IkMiss { side: usize, distance: f32 },
}

/// Detect animation artifacts in a posed human.
pub fn detect_artifacts(posed: &Posed, velocity: Vec3, _rig: &Rig) -> Vec<AnimationArtifact> {
    let mut artifacts = Vec::new();

    // Check for foot sliding (planted feet should match ground velocity)
    let speed_threshold = 0.05; // m/s
    for side in 0..2 {
        let foot_vel_estimate = velocity.length();
        if foot_vel_estimate > speed_threshold && posed.sole[side] < 0.1 {
            artifacts.push(AnimationArtifact::FootSliding {
                side,
                velocity: foot_vel_estimate,
            });
        }
    }

    // Check for IK misses
    for side in 0..2 {
        if posed.leg_miss[side] > 0.02 {
            artifacts.push(AnimationArtifact::IkMiss {
                side,
                distance: posed.leg_miss[side],
            });
        }
    }

    artifacts
}
