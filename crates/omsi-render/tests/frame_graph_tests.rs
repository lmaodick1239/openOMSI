//! Tests for frame graph introspection and pass management.

use omsi_render::inspector::{
    frame_graph::{EntityKey, FrameGraphInspector, IsolationMode, PassInfo, RenderSnapshot},
    TimestampQueryPool,
};

#[test]
fn test_pass_info_creation() {
    let pass = PassInfo::new("Test Pass");

    assert_eq!(pass.name, "Test Pass");
    assert_eq!(pass.gpu_time_ms, 0.0);
    assert_eq!(pass.draw_calls, 0);
    assert_eq!(pass.triangles, 0);
    assert!(pass.enabled);
    assert!(pass.children.is_empty());
}

#[test]
fn test_pass_info_hierarchy() {
    let mut root = PassInfo::new("Main Pass");
    root.gpu_time_ms = 5.0;
    root.draw_calls = 100;
    root.triangles = 50000;

    let mut child1 = PassInfo::new("Shadow Cascade 0");
    child1.gpu_time_ms = 1.5;
    child1.draw_calls = 30;
    child1.triangles = 15000;

    let mut child2 = PassInfo::new("Shadow Cascade 1");
    child2.gpu_time_ms = 1.2;
    child2.draw_calls = 25;
    child2.triangles = 12000;

    root.add_child(child1);
    root.add_child(child2);

    assert_eq!(root.children.len(), 2);
    assert_eq!(root.total_gpu_time_ms(), 7.7);
    assert_eq!(root.total_draw_calls(), 155);
    assert_eq!(root.total_triangles(), 77000);
}

#[test]
fn test_render_snapshot_creation() {
    let snapshot = RenderSnapshot::new(42);

    assert_eq!(snapshot.frame_id, 42);
    assert!(snapshot.passes.is_empty());
    assert_eq!(snapshot.total_gpu_time_ms, 0.0);
    assert!(snapshot.active_pipelines.is_empty());
}

#[test]
fn test_render_snapshot_finalize() {
    let mut snapshot = RenderSnapshot::new(1);

    let mut pass1 = PassInfo::new("Opaque");
    pass1.gpu_time_ms = 3.5;
    snapshot.add_pass(pass1);

    let mut pass2 = PassInfo::new("Transparent");
    pass2.gpu_time_ms = 2.0;
    snapshot.add_pass(pass2);

    let mut pass3 = PassInfo::new("Post");
    pass3.gpu_time_ms = 0.8;
    snapshot.add_pass(pass3);

    snapshot.finalize();

    assert_eq!(snapshot.total_gpu_time_ms, 6.3);
    assert_eq!(snapshot.passes.len(), 3);
}

#[test]
fn test_frame_graph_inspector_creation() {
    let inspector = FrameGraphInspector::new();

    assert_eq!(inspector.isolation_mode(), IsolationMode::None);
    assert!(inspector.pass_toggles().is_empty());
}

#[test]
fn test_frame_graph_inspector_frame_counter() {
    let mut inspector = FrameGraphInspector::new();

    let frame1 = inspector.begin_frame();
    assert_eq!(frame1, 1);

    let frame2 = inspector.begin_frame();
    assert_eq!(frame2, 2);

    let frame3 = inspector.begin_frame();
    assert_eq!(frame3, 3);
}

#[test]
fn test_pass_toggle() {
    let mut inspector = FrameGraphInspector::new();

    assert!(inspector.is_pass_enabled("Shadow"));

    inspector.toggle_pass("Shadow", false);
    assert!(!inspector.is_pass_enabled("Shadow"));

    inspector.toggle_pass("Shadow", true);
    assert!(inspector.is_pass_enabled("Shadow"));

    // Other passes remain enabled by default
    assert!(inspector.is_pass_enabled("SSAO"));
    assert!(inspector.is_pass_enabled("Post"));
}

#[test]
fn test_isolation_mode_none() {
    let mut inspector = FrameGraphInspector::new();

    assert_eq!(inspector.isolation_mode(), IsolationMode::None);

    inspector.reset();
    assert_eq!(inspector.isolation_mode(), IsolationMode::None);
}

#[test]
fn test_isolation_mode_isolate_mesh() {
    let mut inspector = FrameGraphInspector::new();

    let entity = EntityKey::Vehicle { id: 42 };
    inspector.set_isolation_mode(IsolationMode::IsolateMesh(entity));

    assert_eq!(
        inspector.isolation_mode(),
        IsolationMode::IsolateMesh(entity)
    );

    inspector.reset();
    assert_eq!(inspector.isolation_mode(), IsolationMode::None);
}

#[test]
fn test_isolation_mode_hide_mesh() {
    let mut inspector = FrameGraphInspector::new();

    let entity = EntityKey::Scenery { id: 123 };
    inspector.set_isolation_mode(IsolationMode::HideMesh(entity));

    assert_eq!(inspector.isolation_mode(), IsolationMode::HideMesh(entity));
}

#[test]
fn test_isolation_mode_ghost() {
    let mut inspector = FrameGraphInspector::new();

    let entity = EntityKey::Vehicle { id: 99 };
    inspector.set_isolation_mode(IsolationMode::GhostMode { selected: entity });

    if let IsolationMode::GhostMode { selected } = inspector.isolation_mode() {
        assert_eq!(selected, entity);
    } else {
        panic!("Expected GhostMode");
    }
}

#[test]
fn test_inspector_reset() {
    let mut inspector = FrameGraphInspector::new();

    // Set up some state
    inspector.toggle_pass("Shadow", false);
    inspector.toggle_pass("SSAO", false);

    let entity = EntityKey::Vehicle { id: 42 };
    inspector.set_isolation_mode(IsolationMode::IsolateMesh(entity));

    // Verify state was set
    assert!(!inspector.is_pass_enabled("Shadow"));
    assert!(!inspector.is_pass_enabled("SSAO"));
    assert_ne!(inspector.isolation_mode(), IsolationMode::None);

    // Reset
    inspector.reset();

    // Verify everything is back to defaults
    assert!(inspector.is_pass_enabled("Shadow"));
    assert!(inspector.is_pass_enabled("SSAO"));
    assert_eq!(inspector.isolation_mode(), IsolationMode::None);
}

#[test]
fn test_snapshot_storage() {
    let mut inspector = FrameGraphInspector::new();

    assert!(inspector.latest_snapshot().is_none());

    let snapshot = RenderSnapshot::new(1);
    inspector.store_snapshot(snapshot);

    let stored = inspector.latest_snapshot();
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().frame_id, 1);
}

#[test]
fn test_snapshot_pass_toggles_integration() {
    let mut snapshot = RenderSnapshot::new(1);

    snapshot.pass_toggles.insert("Shadow".to_string(), false);
    snapshot.pass_toggles.insert("SSAO".to_string(), true);

    assert!(!snapshot.is_pass_enabled("Shadow"));
    assert!(snapshot.is_pass_enabled("SSAO"));
    assert!(snapshot.is_pass_enabled("Post")); // Default true
}

#[test]
fn test_complex_pass_hierarchy() {
    let mut root = PassInfo::new("Frame");
    root.gpu_time_ms = 0.5; // Overhead

    let mut shadow_group = PassInfo::new("Shadow Passes");
    shadow_group.gpu_time_ms = 0.1; // Setup time

    for i in 0..3 {
        let mut cascade = PassInfo::new(format!("Cascade {}", i));
        cascade.gpu_time_ms = 1.0 + (i as f32 * 0.2);
        cascade.draw_calls = 30 + (i * 5);
        cascade.triangles = 10000 + (i * 2000);
        shadow_group.add_child(cascade);
    }

    root.add_child(shadow_group);

    let mut main = PassInfo::new("Main PBR");
    main.gpu_time_ms = 5.0;
    main.draw_calls = 150;
    main.triangles = 80000;
    root.add_child(main);

    // Verify totals
    assert_eq!(root.children.len(), 2);

    let total_time = root.total_gpu_time_ms();
    assert!(total_time > 9.0 && total_time < 10.0);

    let total_calls = root.total_draw_calls();
    assert_eq!(total_calls, 30 + 35 + 40 + 150);
}

#[test]
fn test_entity_key_equality() {
    let v1 = EntityKey::Vehicle { id: 42 };
    let v2 = EntityKey::Vehicle { id: 42 };
    let v3 = EntityKey::Vehicle { id: 43 };

    assert_eq!(v1, v2);
    assert_ne!(v1, v3);

    let s1 = EntityKey::Scenery { id: 100 };
    let s2 = EntityKey::Scenery { id: 100 };

    assert_eq!(s1, s2);
    assert_ne!(v1, s1);
}

#[test]
fn test_isolation_mode_equality() {
    let entity1 = EntityKey::Vehicle { id: 1 };
    let entity2 = EntityKey::Vehicle { id: 2 };

    let mode1 = IsolationMode::IsolateMesh(entity1);
    let mode2 = IsolationMode::IsolateMesh(entity1);
    let mode3 = IsolationMode::IsolateMesh(entity2);

    assert_eq!(mode1, mode2);
    assert_ne!(mode1, mode3);

    let ghost1 = IsolationMode::GhostMode { selected: entity1 };
    let ghost2 = IsolationMode::GhostMode { selected: entity1 };

    assert_eq!(ghost1, ghost2);
    assert_ne!(mode1, ghost1);
}

#[test]
fn test_snapshot_with_hierarchy() {
    let mut snapshot = RenderSnapshot::new(100);

    let mut shadows = PassInfo::new("Shadows");
    shadows.gpu_time_ms = 0.2;

    let mut cascade0 = PassInfo::new("Cascade 0");
    cascade0.gpu_time_ms = 1.2;
    cascade0.draw_calls = 50;
    cascade0.triangles = 25000;

    let mut cascade1 = PassInfo::new("Cascade 1");
    cascade1.gpu_time_ms = 0.9;
    cascade1.draw_calls = 40;
    cascade1.triangles = 20000;

    shadows.add_child(cascade0);
    shadows.add_child(cascade1);

    snapshot.add_pass(shadows);

    let mut main = PassInfo::new("Main");
    main.gpu_time_ms = 6.5;
    main.draw_calls = 200;
    main.triangles = 100000;

    snapshot.add_pass(main);

    snapshot.finalize();

    // Total should be: 0.2 (shadows) + 1.2 (c0) + 0.9 (c1) + 6.5 (main) = 8.8ms
    assert!((snapshot.total_gpu_time_ms - 8.8).abs() < 0.01);
}
