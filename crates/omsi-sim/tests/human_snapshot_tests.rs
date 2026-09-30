//! Tests for human snapshot and animation introspection.

use omsi_sim::human::{Activity, BehaviorState, HumanSnapshot, Pose, Rig};
use glam::Vec3;

#[test]
fn test_human_snapshot_minimal() {
    let snapshot = HumanSnapshot::minimal(42, 1, false);
    
    assert_eq!(snapshot.id, 42);
    assert_eq!(snapshot.generation, 1);
    assert!(!snapshot.is_driver);
    assert_eq!(snapshot.position, Vec3::ZERO);
    assert_eq!(snapshot.velocity, Vec3::ZERO);
    assert_eq!(snapshot.behavior_state, BehaviorState::Idle);
    assert!(snapshot.skeleton_bones.is_empty());
}

#[test]
fn test_behavior_state_from_activity() {
    assert_eq!(BehaviorState::from(Activity::Stand), BehaviorState::Idle);
    assert_eq!(BehaviorState::from(Activity::Walk), BehaviorState::Walking);
    assert_eq!(BehaviorState::from(Activity::Sit), BehaviorState::Sitting);
    assert_eq!(BehaviorState::from(Activity::Pay), BehaviorState::Paying);
}

#[test]
fn test_pose_accessors() {
    let pose = Pose::new(12345);
    
    // Test accessor methods
    assert_eq!(pose.walk_weight(), 0.0);
    assert_eq!(pose.gait_phase(), 0.0);
    assert_eq!(pose.sit_weight(), 0.0);
    assert_eq!(pose.ground_speed(), 0.0);
    assert_eq!(pose.forward_accel(), 0.0);
    assert_eq!(pose.turn_rate(), 0.0);
    
    let breath = pose.breath_phase();
    assert!(breath >= 0.0 && breath < 1.0);
}

#[test]
fn test_animation_state_capture() {
    let pose = Pose::new(54321);
    let activity = Activity::Stand;
    
    let anim_state = omsi_sim::human::animation::AnimationState::from_pose(&pose, activity);
    
    assert_eq!(anim_state.activity, Activity::Stand);
    assert_eq!(anim_state.gait_phase, 0.0);
    assert_eq!(anim_state.walk_weight, 0.0);
    assert_eq!(anim_state.sit_weight, 0.0);
    assert!(!anim_state.active_clips.is_empty()); // Should have idle clip
}

#[test]
fn test_bone_transform_euler_conversion() {
    use omsi_sim::human::BoneTransform;
    use glam::{Quat, Vec3};
    
    let transform = glam::Affine3A::from_rotation_translation(
        Quat::IDENTITY,
        Vec3::new(1.0, 2.0, 3.0),
    );
    
    let bone = BoneTransform::from_affine("TestBone".to_string(), &transform);
    
    assert_eq!(bone.name, "TestBone");
    assert_eq!(bone.position, Vec3::new(1.0, 2.0, 3.0));
    assert!((bone.euler_angles.length()) < 0.1); // Near zero rotation
}

#[test]
fn test_snapshot_with_navigation() {
    let pose = Pose::new(99999);
    let rig = create_test_rig();
    let posed = pose.clone().bones(&rig);
    
    let nav_target = Some(Vec3::new(10.0, 20.0, 0.0));
    
    let snapshot = HumanSnapshot::from_pose(
        1,
        1,
        false,
        &pose,
        &posed,
        &rig,
        Activity::Walk,
        Vec3::new(5.0, 5.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        nav_target,
    );
    
    assert_eq!(snapshot.navigation_target, nav_target);
    assert!(snapshot.steering_vector.is_some());
    assert_eq!(snapshot.behavior_state, BehaviorState::Walking);
}

#[test]
fn test_artifact_detection() {
    use omsi_sim::human::animation::detect_artifacts;
    
    let mut pose = Pose::new(11111);
    let rig = create_test_rig();
    let posed = pose.bones(&rig);
    
    // Test with zero velocity (no sliding)
    let artifacts = detect_artifacts(&posed, Vec3::ZERO, &rig);
    assert!(artifacts.is_empty() || artifacts.iter().all(|a| !matches!(a, omsi_sim::human::AnimationArtifact::FootSliding { .. })));
}

#[test]
fn test_skeleton_extraction() {
    use omsi_sim::human::animation::extract_skeleton;
    
    let mut pose = Pose::new(22222);
    let rig = create_test_rig();
    let posed = pose.bones(&rig);
    
    let skeleton = extract_skeleton(&posed, &rig);
    
    // Should have all major bones
    assert!(!skeleton.is_empty());
    
    // Check for key bones
    let bone_names: Vec<_> = skeleton.iter().map(|b| b.name.as_str()).collect();
    assert!(bone_names.contains(&"Hip"));
    assert!(bone_names.contains(&"Spine"));
    assert!(bone_names.contains(&"Head"));
    assert!(bone_names.contains(&"LeftThigh"));
    assert!(bone_names.contains(&"RightThigh"));
}

#[test]
fn test_passenger_economy_default() {
    use omsi_sim::human::PassengerEconomy;
    
    let economy = PassengerEconomy::default();
    
    assert_eq!(economy.ticket_type, "None");
    assert_eq!(economy.comfort_index, 100.0);
    assert_eq!(economy.destination_stop, "Unknown");
    assert!(!economy.alighting_requested);
    assert!(economy.comfort_factors.is_empty());
}

#[test]
fn test_snapshot_performance() {
    use std::time::Instant;
    
    let pose = Pose::new(33333);
    let rig = create_test_rig();
    
    let start = Instant::now();
    
    // Create 200 snapshots to simulate 200 pedestrians
    for i in 0..200 {
        let posed = pose.clone().bones(&rig);
        let _snapshot = HumanSnapshot::from_pose(
            i,
            1,
            false,
            &pose,
            &posed,
            &rig,
            Activity::Walk,
            Vec3::new(i as f32, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            None,
        );
    }
    
    let elapsed = start.elapsed();
    
    // Should be fast: <0.15ms per snapshot * 200 = <30ms total
    assert!(elapsed.as_millis() < 100, "Snapshot creation too slow: {:?}", elapsed);
}

// Helper function to create a test rig
fn create_test_rig() -> Rig {
    use omsi_sim::human::Joints;
    use glam::Vec3;
    
    let _joints = Joints {
        hip: Vec3::new(0.08, 0.0, 0.9),
        knee: Vec3::new(0.08, 0.0, 0.5),
        waist: Vec3::new(0.0, 0.0, 1.0),
        shoulder: Vec3::new(0.15, 0.0, 1.35),
        elbow: Vec3::new(0.3, 0.0, 1.35),
        neck: Vec3::new(0.0, 0.0, 1.55),
        hand: Vec3::new(0.5, 0.0, 1.35),
        finger: Vec3::new(0.6, 0.0, 1.35),
    };
    
    // Create a minimal rig for testing
    Rig {
        hip: [Vec3::new(-0.08, 0.0, 0.9), Vec3::new(0.08, 0.0, 0.9)],
        knee: [Vec3::new(-0.08, 0.0, 0.5), Vec3::new(0.08, 0.0, 0.5)],
        ankle: [Vec3::new(-0.08, 0.1, 0.08), Vec3::new(0.08, 0.1, 0.08)],
        shoulder: [Vec3::new(-0.15, 0.0, 1.35), Vec3::new(0.15, 0.0, 1.35)],
        elbow: [Vec3::new(-0.3, 0.0, 1.35), Vec3::new(0.3, 0.0, 1.35)],
        wrist: [Vec3::new(-0.5, 0.0, 1.35), Vec3::new(0.5, 0.0, 1.35)],
        waist: Vec3::new(0.0, 0.0, 1.0),
        neck: Vec3::new(0.0, 0.0, 1.55),
        head_pivot: Vec3::new(0.0, 0.0, 1.55),
        pelvis: Vec3::new(0.0, 0.0, 0.9),
        thigh: 0.4,
        shin: 0.42,
        upper_arm: 0.28,
        forearm: 0.25,
        sole: 0.0,
        ankle_h: 0.08,
        heel: -0.05,
        ball: 0.15,
        toe: 0.22,
        ball_h: 0.02,
        head_top: 1.75,
        seat_lift: 0.1,
        scale: 1.0,
        walk_speed: 1.4,
        walk_step: 0.75,
        arm_swing: 1.0,
        hip_sway: 1.0,
    }
}
