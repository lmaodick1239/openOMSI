//! Tests for inspector multi-hit penetration stack (Subsystem G1, G2).
//!
//! These tests verify that the penetration stack correctly collects, orders, and allows
//! cycling through multiple overlapping hits along a raycast.

use omsi_app::inspector::{
    build_hit_display_name, build_penetration_stack, InspectorHit, InspectorSelection,
    PenetrationHit, SceneryKey, SelectionTarget, VehicleKey,
};

#[test]
fn test_penetration_hit_creation() {
    let target = SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation: 1 },
        mesh: None,
    };
    let hit = PenetrationHit::new(1.5, target.clone(), "test_mesh.o3d".to_string());

    assert_eq!(hit.distance, 1.5);
    assert_eq!(hit.target, target);
    assert_eq!(hit.display_name, "test_mesh.o3d");
}

#[test]
fn test_penetration_stack_ordering() {
    // Create hits at different distances
    let target1 = SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation: 1 },
        mesh: None,
    };
    let target2 = SelectionTarget::Vehicle {
        key: VehicleKey::AiCar { id: 100 },
        mesh: None,
    };
    let target3 = SelectionTarget::Scenery {
        key: SceneryKey::Editable { map_id: 1 },
        mesh: None,
    };

    let hits = vec![
        InspectorHit::new(5.0, target3).unwrap(),
        InspectorHit::new(1.0, target1).unwrap(),
        InspectorHit::new(3.0, target2).unwrap(),
    ];

    let stack = build_penetration_stack(hits);

    // Verify hits are sorted by distance
    assert_eq!(stack.len(), 3);
    assert_eq!(stack[0].distance, 1.0);
    assert_eq!(stack[1].distance, 3.0);
    assert_eq!(stack[2].distance, 5.0);
}

#[test]
fn test_penetration_stack_limit() {
    // Create 25 hits (should be limited to 20)
    let mut hits = Vec::new();
    for i in 0..25 {
        let target = SelectionTarget::Vehicle {
            key: VehicleKey::AiCar { id: i },
            mesh: None,
        };
        hits.push(InspectorHit::new((i + 1) as f32, target).unwrap());
    }

    let stack = build_penetration_stack(hits);

    // Verify only first 20 hits are included
    assert_eq!(stack.len(), 20);
    assert_eq!(stack[0].distance, 1.0);
    assert_eq!(stack[19].distance, 20.0);
}

#[test]
fn test_cycle_next_hit() {
    let mut selection = InspectorSelection::default();

    // Create test penetration stack with 3 hits
    let hits = vec![
        PenetrationHit::new(
            1.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[1.0m] windshield.o3d".to_string(),
        ),
        PenetrationHit::new(
            2.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[2.0m] dashboard.o3d".to_string(),
        ),
        PenetrationHit::new(
            3.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[3.0m] seat.o3d".to_string(),
        ),
    ];

    selection.set_penetration_stack(hits);

    // Verify initial state (should be at index 0)
    assert_eq!(selection.current_hit_index, 0);
    assert_eq!(selection.current_hit().unwrap().distance, 1.0);

    // Cycle to next hit
    selection.cycle_next_hit();
    assert_eq!(selection.current_hit_index, 1);
    assert_eq!(selection.current_hit().unwrap().distance, 2.0);

    // Cycle to next hit
    selection.cycle_next_hit();
    assert_eq!(selection.current_hit_index, 2);
    assert_eq!(selection.current_hit().unwrap().distance, 3.0);

    // Cycle wraps around to first hit
    selection.cycle_next_hit();
    assert_eq!(selection.current_hit_index, 0);
    assert_eq!(selection.current_hit().unwrap().distance, 1.0);
}

#[test]
fn test_cycle_prev_hit() {
    let mut selection = InspectorSelection::default();

    // Create test penetration stack with 3 hits
    let hits = vec![
        PenetrationHit::new(
            1.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[1.0m] windshield.o3d".to_string(),
        ),
        PenetrationHit::new(
            2.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[2.0m] dashboard.o3d".to_string(),
        ),
        PenetrationHit::new(
            3.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[3.0m] seat.o3d".to_string(),
        ),
    ];

    selection.set_penetration_stack(hits);

    // Verify initial state (should be at index 0)
    assert_eq!(selection.current_hit_index, 0);

    // Cycle to previous hit (wraps around to last)
    selection.cycle_prev_hit();
    assert_eq!(selection.current_hit_index, 2);
    assert_eq!(selection.current_hit().unwrap().distance, 3.0);

    // Cycle to previous hit
    selection.cycle_prev_hit();
    assert_eq!(selection.current_hit_index, 1);
    assert_eq!(selection.current_hit().unwrap().distance, 2.0);

    // Cycle to previous hit
    selection.cycle_prev_hit();
    assert_eq!(selection.current_hit_index, 0);
    assert_eq!(selection.current_hit().unwrap().distance, 1.0);
}

#[test]
fn test_jump_to_hit() {
    let mut selection = InspectorSelection::default();

    // Create test penetration stack with 5 hits
    let hits = (0..5)
        .map(|i| {
            PenetrationHit::new(
                (i + 1) as f32,
                SelectionTarget::Vehicle {
                    key: VehicleKey::AiCar { id: i },
                    mesh: None,
                },
                format!("[{}.0m] mesh_{}.o3d", i + 1, i),
            )
        })
        .collect();

    selection.set_penetration_stack(hits);

    // Jump to hit 3 (index 2)
    selection.jump_to_hit(2);
    assert_eq!(selection.current_hit_index, 2);
    assert_eq!(selection.current_hit().unwrap().distance, 3.0);

    // Jump to hit 5 (index 4)
    selection.jump_to_hit(4);
    assert_eq!(selection.current_hit_index, 4);
    assert_eq!(selection.current_hit().unwrap().distance, 5.0);

    // Jump to hit 1 (index 0)
    selection.jump_to_hit(0);
    assert_eq!(selection.current_hit_index, 0);
    assert_eq!(selection.current_hit().unwrap().distance, 1.0);

    // Jump to invalid index (should have no effect)
    selection.jump_to_hit(10);
    assert_eq!(selection.current_hit_index, 0); // Should remain at index 0
}

#[test]
fn test_jump_to_hit_out_of_bounds() {
    let mut selection = InspectorSelection::default();

    let hits = vec![PenetrationHit::new(
        1.0,
        SelectionTarget::Vehicle {
            key: VehicleKey::Player { generation: 1 },
            mesh: None,
        },
        "[1.0m] mesh.o3d".to_string(),
    )];

    selection.set_penetration_stack(hits);
    assert_eq!(selection.current_hit_index, 0);

    // Try to jump to out-of-bounds index
    selection.jump_to_hit(5);
    // Should remain at index 0
    assert_eq!(selection.current_hit_index, 0);
}

#[test]
fn test_clear_resets_penetration_stack() {
    let mut selection = InspectorSelection::default();

    let hits = vec![
        PenetrationHit::new(
            1.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[1.0m] mesh1.o3d".to_string(),
        ),
        PenetrationHit::new(
            2.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[2.0m] mesh2.o3d".to_string(),
        ),
    ];

    selection.set_penetration_stack(hits);
    assert_eq!(selection.penetration_stack.len(), 2);
    assert_eq!(selection.current_hit_index, 0);

    // Clear should reset everything
    selection.clear();
    assert_eq!(selection.penetration_stack.len(), 0);
    assert_eq!(selection.current_hit_index, 0);
    assert!(!selection.is_active());
}

#[test]
fn test_build_hit_display_name_vehicle() {
    use omsi_app::inspector::MeshIdentity;

    let mesh_id = MeshIdentity::new(
        "models/bus.cfg".to_string(),
        5,
        "cockpit_speedo.o3d".to_string(),
        None,
    );

    let target = SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation: 1 },
        mesh: Some(mesh_id),
    };

    let display_name = build_hit_display_name(&target, 1.23);
    assert_eq!(display_name, "[1.2m] cockpit_speedo.o3d");
}

#[test]
fn test_build_hit_display_name_scenery() {
    use omsi_app::inspector::MeshIdentity;

    let mesh_id = MeshIdentity::new(
        "scenery/building.sco".to_string(),
        2,
        "wall_front.o3d".to_string(),
        None,
    );

    let target = SelectionTarget::Scenery {
        key: SceneryKey::NonEditable {
            tile_x: 4850,
            tile_y: 3170,
            key: 42,
        },
        mesh: Some(mesh_id),
    };

    let display_name = build_hit_display_name(&target, 14.56);
    assert_eq!(display_name, "[14.6m] wall_front.o3d (Tile (4850, 3170))");
}

#[test]
fn test_articulated_bus_regression() {
    // Regression test: articulated bus joint with 3+ overlapping volumes
    let mut selection = InspectorSelection::default();

    let hits = vec![
        PenetrationHit::new(
            5.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[5.0m] front_section.o3d".to_string(),
        ),
        PenetrationHit::new(
            5.1,
            SelectionTarget::Vehicle {
                key: VehicleKey::PlayerTrailer {
                    generation: 1,
                    trailer_index: 0,
                },
                mesh: None,
            },
            "[5.1m] articulation_joint.o3d".to_string(),
        ),
        PenetrationHit::new(
            5.2,
            SelectionTarget::Vehicle {
                key: VehicleKey::PlayerTrailer {
                    generation: 1,
                    trailer_index: 0,
                },
                mesh: None,
            },
            "[5.2m] rear_section.o3d".to_string(),
        ),
    ];

    selection.set_penetration_stack(hits);

    // Verify we can cycle through all 3 hits
    assert_eq!(selection.current_hit_index, 0);
    selection.cycle_next_hit();
    assert_eq!(selection.current_hit_index, 1);
    selection.cycle_next_hit();
    assert_eq!(selection.current_hit_index, 2);
    selection.cycle_next_hit();
    assert_eq!(selection.current_hit_index, 0); // Wraps around
}

#[test]
fn test_nested_bus_interior_scenario() {
    // Test case: nested geometry with bus interior (seats, passengers, dashboard)
    let mut selection = InspectorSelection::default();

    let hits = vec![
        PenetrationHit::new(
            1.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[1.0m] windshield.o3d".to_string(),
        ),
        PenetrationHit::new(
            1.5,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[1.5m] dashboard_casing.o3d".to_string(),
        ),
        PenetrationHit::new(
            1.8,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[1.8m] speedometer.o3d".to_string(),
        ),
        PenetrationHit::new(
            2.5,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[2.5m] driver_seat.o3d".to_string(),
        ),
        PenetrationHit::new(
            5.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[5.0m] passenger_seat_01.o3d".to_string(),
        ),
    ];

    selection.set_penetration_stack(hits);

    // Verify all hits are accessible and in correct order
    assert_eq!(selection.penetration_stack.len(), 5);
    assert_eq!(selection.penetration_stack[0].distance, 1.0);
    assert_eq!(selection.penetration_stack[4].distance, 5.0);

    // Cycle through all hits
    for expected_idx in 0..5 {
        assert_eq!(selection.current_hit_index, expected_idx);
        selection.cycle_next_hit();
    }
    // Should wrap back to 0
    assert_eq!(selection.current_hit_index, 0);
}

#[test]
fn test_empty_penetration_stack_behavior() {
    let mut selection = InspectorSelection::default();

    // Empty stack
    assert_eq!(selection.penetration_stack.len(), 0);
    assert_eq!(selection.current_hit_index, 0);
    assert!(selection.current_hit().is_none());

    // Cycling should have no effect
    selection.cycle_next_hit();
    assert_eq!(selection.current_hit_index, 0);

    selection.cycle_prev_hit();
    assert_eq!(selection.current_hit_index, 0);

    selection.jump_to_hit(5);
    assert_eq!(selection.current_hit_index, 0);
}
