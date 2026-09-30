//! Human snapshot system for inspector telemetry.
//!
//! Provides atomic snapshot capture of human state including skeletal pose, animation state,
//! navigation intent, and passenger economy data.

use glam::Vec3;
use super::legacy::{Activity, Pose, Posed, Rig};
use super::{BoneTransform, AnimationState, AnimationArtifact};
use crate::human::animation::{detect_artifacts, extract_skeleton};

/// Behavior state for human agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehaviorState {
    /// Standing idle.
    Idle,
    /// Walking to destination.
    Walking,
    /// Sitting in seat.
    Sitting,
    /// Boarding vehicle.
    Boarding,
    /// Alighting from vehicle.
    Alighting,
    /// Paying fare at cash desk.
    Paying,
    /// Waiting at stop.
    WaitingAtStop,
    /// Queuing (e.g., at door or cash desk).
    Queuing,
}

impl From<Activity> for BehaviorState {
    fn from(activity: Activity) -> Self {
        match activity {
            Activity::Stand => BehaviorState::Idle,
            Activity::Walk => BehaviorState::Walking,
            Activity::Sit => BehaviorState::Sitting,
            Activity::Pay => BehaviorState::Paying,
        }
    }
}

/// Passenger economy and disposition telemetry.
#[derive(Debug, Clone)]
pub struct PassengerEconomy {
    /// Ticket transaction status.
    pub ticket_type: String,
    /// Comfort & satisfaction index [0..100%].
    pub comfort_index: f32,
    /// Desired destination stop name.
    pub destination_stop: String,
    /// Whether alighting request is active.
    pub alighting_requested: bool,
    /// Degradation factors affecting comfort.
    pub comfort_factors: Vec<(String, f32)>,
}

impl Default for PassengerEconomy {
    fn default() -> Self {
        PassengerEconomy {
            ticket_type: "None".to_string(),
            comfort_index: 100.0,
            destination_stop: "Unknown".to_string(),
            alighting_requested: false,
            comfort_factors: Vec::new(),
        }
    }
}

/// Complete snapshot of a human's state for inspector display.
#[derive(Debug, Clone)]
pub struct HumanSnapshot {
    /// Stable human identity key.
    pub id: u32,
    pub generation: u64,
    pub is_driver: bool,
    /// World-space position.
    pub position: Vec3,
    /// World-space velocity (m/s).
    pub velocity: Vec3,
    /// Current behavior state.
    pub behavior_state: BehaviorState,
    /// Animation phase [0.0..1.0].
    pub animation_phase: f32,
    /// Skeletal bone transforms.
    pub skeleton_bones: Vec<BoneTransform>,
    /// Active animation clip name.
    pub active_animation: String,
    /// Animation state telemetry.
    pub animation_state: AnimationState,
    /// Navigation target position (world-space).
    pub navigation_target: Option<Vec3>,
    /// Navigation steering vector.
    pub steering_vector: Option<Vec3>,
    /// Passenger economy data (for boarded passengers).
    pub passenger_economy: Option<PassengerEconomy>,
    /// Detected animation artifacts.
    pub artifacts: Vec<AnimationArtifact>,
}

impl HumanSnapshot {
    /// Create a snapshot from pose and state.
    pub fn from_pose(
        id: u32,
        generation: u64,
        is_driver: bool,
        pose: &Pose,
        posed: &Posed,
        rig: &Rig,
        activity: Activity,
        world_position: Vec3,
        world_velocity: Vec3,
        navigation_target: Option<Vec3>,
    ) -> Self {
        let behavior_state = BehaviorState::from(activity);
        let animation_state = AnimationState::from_pose(pose, activity);
        let skeleton_bones = extract_skeleton(posed, rig);
        let artifacts = detect_artifacts(posed, world_velocity, rig);

        let active_animation = if !animation_state.active_clips.is_empty() {
            animation_state.active_clips[0].name.clone()
        } else {
            "none".to_string()
        };

        let animation_phase = if !animation_state.active_clips.is_empty() {
            animation_state.active_clips[0].phase
        } else {
            0.0
        };

        // Compute steering vector from velocity
        let steering_vector = if world_velocity.length() > 0.01 {
            Some(world_velocity.normalize())
        } else {
            None
        };

        HumanSnapshot {
            id,
            generation,
            is_driver,
            position: world_position,
            velocity: world_velocity,
            behavior_state,
            animation_phase,
            skeleton_bones,
            active_animation,
            animation_state,
            navigation_target,
            steering_vector,
            passenger_economy: None, // Populated by passenger system
            artifacts,
        }
    }

    /// Create a minimal snapshot (for unavailable humans).
    pub fn minimal(id: u32, generation: u64, is_driver: bool) -> Self {
        HumanSnapshot {
            id,
            generation,
            is_driver,
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            behavior_state: BehaviorState::Idle,
            animation_phase: 0.0,
            skeleton_bones: Vec::new(),
            active_animation: "unavailable".to_string(),
            animation_state: AnimationState {
                active_clips: Vec::new(),
                activity: Activity::Stand,
                gait_phase: 0.0,
                walk_weight: 0.0,
                sit_weight: 0.0,
                speed: 0.0,
                accel: 0.0,
                turn_rate: 0.0,
            },
            navigation_target: None,
            steering_vector: None,
            passenger_economy: None,
            artifacts: Vec::new(),
        }
    }
}
