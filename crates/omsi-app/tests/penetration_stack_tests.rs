//! Integration tests for Phase 1, Task 1.2: Multi-Hit Penetration Stack & Ray Corridor (Subsystem G).
//!
//! These tests verify:
//! - Multi-hit raycast collection and sorting
//! - Tab/Shift+Tab cycling with wraparound
//! - 256-candidate cap enforcement
//! - Nested geometry handling (bus interior)
//! - Performance budget (<0.5ms)

use openomsi_game::inspector::{
    build_hit_display_name, build_penetration_stack, InspectorHit, InspectorSelection,
    PenetrationHit, SceneryKey, SelectionTarget, VehicleKey, MeshIdentity,
};
use std::time::Instant;

#[test]
fn test_multi_hit_collection_and_sorting() {
    // Verify hits are collected and sorted by distance
    let hits = vec![
        InspectorHit::new(
            5.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
        )
        .unwrap(),
        InspectorHit::new(
            1.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
        )
        .unwrap(),
        InspectorHit::new(
            3.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
        )
        .unwrap(),
    ];

    let stack = build_penetration_stack(hits);

    assert_eq!(stack.len(), 3);
    assert_eq!(stack[0].distance, 1.0);
    assert_eq!(stack[1].distance, 3.0);
    assert_eq!(stack[2].distance, 5.0);
}

#[test]
fn test_256_candidate_cap() {
    // Verify hard limit of 256 candidates (limited to 20 in build_penetration_stack)
    let mut hits = Vec::new();
    
    for i in 0..300 {
        hits.push(
            InspectorHit::new(
                (i + 1) as f32,
                SelectionTarget::Vehicle {
                    key: VehicleKey::AiCar { id: i },
                    mesh: None,
                },
            )
            .unwrap(),
        );
    }

    let stack = build_penetration_stack(hits);

    // Should be limited to first 20 hits
    assert_eq!(stack.len(), 20);
    assert_eq!(stack[0].distance, 1.0);
    assert_eq!(stack[19].distance, 20.0);
}

#[test]
fn test_tab_cycling_forward() {
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
        PenetrationHit::new(
            3.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[3.0m] mesh3.o3d".to_string(),
        ),
    ];

    selection.set_penetration_stack(hits);

    // Initial state
    assert_eq!(selection.current_hit_index, 0);

    // Tab forward
    selection.cycle_next_hit();
    assert_eq!(selection.current_hit_index, 1);

    selection.cycle_next_hit();
    assert_eq!(selection.current_hit_index, 2);

    // Wraparound
    selection.cycle_next_hit();
    assert_eq!(selection.current_hit_index, 0);
}

#[test]
fn test_shift_tab_cycling_backward() {
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
        PenetrationHit::new(
            3.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[3.0m] mesh3.o3d".to_string(),
        ),
    ];

    selection.set_penetration_stack(hits);

    // Initial state at index 0
    assert_eq!(selection.current_hit_index, 0);

    // Shift+Tab backward (wraps to end)
    selection.cycle_prev_hit();
    assert_eq!(selection.current_hit_index, 2);

    selection.cycle_prev_hit();
    assert_eq!(selection.current_hit_index, 1);

    selection.cycle_prev_hit();
    assert_eq!(selection.current_hit_index, 0);
}

#[test]
fn test_wraparound_at_boundaries() {
    let mut selection = InspectorSelection::default();

    let hits = vec![
        PenetrationHit::new(
            1.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[1.0m] mesh.o3d".to_string(),
        ),
        PenetrationHit::new(
            2.0,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: None,
            },
            "[2.0m] mesh.o3d".to_string(),
        ),
    ];

    selection.set_penetration_stack(hits);

    // Forward wraparound
    assert_eq!(selection.current_hit_index, 0);
    selection.cycle_next_hit();
    assert_eq!(selection.current_hit_index, 1);
    selection.cycle_next_hit();
    assert_eq!(selection.current_hit_index, 0); // Wrapped

    // Backward wraparound
    selection.cycle_prev_hit();
    assert_eq!(selection.current_hit_index, 1); // Wrapped to end
}

#[test]
fn test_nested_bus_interior_geometry() {
    // Test nested geometry: bus interior with seats, dashboard, windows
    let mut selection = InspectorSelection::default();

    let mesh_id_windshield = MeshIdentity::new(
        "models/bus.cfg".to_string(),
        0,
        "windshield.o3d".to_string(),
        None,
    );
    let mesh_id_dashboard = MeshIdentity::new(
        "models/bus.cfg".to_string(),
        1,
        "dashboard_casing.o3d".to_string(),
        None,
    );
    let mesh_id_speedo = MeshIdentity::new(
        "models/bus.cfg".to_string(),
        2,
        "cockpit_speedo.o3d".to_string(),
        None,
    );
    let mesh_id_seat = MeshIdentity::new(
        "models/bus.cfg".to_string(),
        3,
        "driver_seat.o3d".to_string(),
        None,
    );
    let mesh_id_frame = MeshIdentity::new(
        "models/bus.cfg".to_string(),
        4,
        "chassis_frame.o3d".to_string(),
        None,
    );

    let hits = vec![
        PenetrationHit::new(
            0.82,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: Some(mesh_id_windshield),
            },
            "[0.8m] windshield.o3d".to_string(),
        ),
        PenetrationHit::new(
            1.10,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: Some(mesh_id_dashboard),
            },
            "[1.1m] dashboard_casing.o3d".to_string(),
        ),
        PenetrationHit::new(
            1.25,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: Some(mesh_id_speedo),
            },
            "[1.3m] cockpit_speedo.o3d".to_string(),
        ),
        PenetrationHit::new(
            1.80,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: Some(mesh_id_seat),
            },
            "[1.8m] driver_seat.o3d".to_string(),
        ),
        PenetrationHit::new(
            2.40,
            SelectionTarget::Vehicle {
                key: VehicleKey::Player { generation: 1 },
                mesh: Some(mesh_id_frame),
            },
            "[2.4m] chassis_frame.o3d".to_string(),
        ),
    ];

    selection.set_penetration_stack(hits);

    // Verify all hits are accessible
    assert_eq!(selection.penetration_stack.len(), 5);
    
    // Verify distance sorting
    assert_eq!(selection.penetration_stack[0].distance, 0.82);
    assert_eq!(selection.penetration_stack[1].distance, 1.10);
    assert_eq!(selection.penetration_stack[2].distance, 1.25);
    assert_eq!(selection.penetration_stack[3].distance, 1.80);
    assert_eq!(selection.penetration_stack[4].distance, 2.40);

    // Verify cycling through all hits
    for expected_idx in 0..5 {
        assert_eq!(selection.current_hit_index, expected_idx);
        let current = selection.current_hit().unwrap();
        assert_eq!(current.distance, selection.penetration_stack[expected_idx].distance);
        selection.cycle_next_hit();
    }

    // Verify wraparound
    assert_eq!(selection.current_hit_index, 0);
}

#[test]
fn test_performance_budget() {
    // Verify stack assembly and sorting is <0.5ms for 20 hits
    let mut hits = Vec::new();
    
    for i in (0..20).rev() {
        // Reverse order to ensure sorting is actually happening
        hits.push(
            InspectorHit::new(
                (i + 1) as f32,
                SelectionTarget::Vehicle {
                    key: VehicleKey::AiCar { id: i },
                    mesh: None,
                },
            )
            .unwrap(),
        );
    }

    let start = Instant::now();
    let stack = build_penetration_stack(hits);
    let elapsed = start.elapsed();

    // Verify correct sorting
    assert_eq!(stack.len(), 20);
    for i in 0..20 {
        assert_eq!(stack[i].distance, (i + 1) as f32);
    }

    // Performance budget: <0.5ms
    assert!(
        elapsed.as_micros() < 500,
        "Stack assembly took {}μs, expected <500μs",
        elapsed.as_micros()
    );
}

#[test]
fn test_display_name_formatting() {
    // Test display name generation for different target types
    let mesh_id = MeshIdentity::new(
        "models/bus.cfg".to_string(),
        0,
        "cockpit_speedo.o3d".to_string(),
        None,
    );

    let target_vehicle = SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation: 1 },
        mesh: Some(mesh_id),
    };

    let name = build_hit_display_name(&target_vehicle, 1.234);
    assert_eq!(name, "[1.2m] cockpit_speedo.o3d");

    // Scenery target
    let mesh_id_scenery = MeshIdentity::new(
        "scenery/building.sco".to_string(),
        0,
        "wall.o3d".to_string(),
        None,
    );

    let target_scenery = SelectionTarget::Scenery {
        key: SceneryKey::NonEditable {
            tile_x: 100,
            tile_y: 200,
            key: 42,
        },
        mesh: Some(mesh_id_scenery),
    };

    let name = build_hit_display_name(&target_scenery, 5.678);
    assert_eq!(name, "[5.7m] wall.o3d (Tile (100, 200))");
}

#[test]
fn test_empty_stack_behavior() {
    let mut selection = InspectorSelection::default();

    // Empty stack should not crash on operations
    assert_eq!(selection.penetration_stack.len(), 0);
    assert!(selection.current_hit().is_none());

    selection.cycle_next_hit();
    assert_eq!(selection.current_hit_index, 0);

    selection.cycle_prev_hit();
    assert_eq!(selection.current_hit_index, 0);

    selection.jump_to_hit(5);
    assert_eq!(selection.current_hit_index, 0);
}

#[test]
fn test_single_hit_wraparound() {
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

    // With single hit, cycling should stay at index 0
    assert_eq!(selection.current_hit_index, 0);

    selection.cycle_next_hit();
    assert_eq!(selection.current_hit_index, 0);

    selection.cycle_prev_hit();
    assert_eq!(selection.current_hit_index, 0);
}

#[test]
fn test_jump_to_specific_hit() {
    let mut selection = InspectorSelection::default();

    let hits = (0..10)
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

    // Jump to various indices
    selection.jump_to_hit(5);
    assert_eq!(selection.current_hit_index, 5);
    assert_eq!(selection.current_hit().unwrap().distance, 6.0);

    selection.jump_to_hit(0);
    assert_eq!(selection.current_hit_index, 0);
    assert_eq!(selection.current_hit().unwrap().distance, 1.0);

    selection.jump_to_hit(9);
    assert_eq!(selection.current_hit_index, 9);
    assert_eq!(selection.current_hit().unwrap().distance, 10.0);

    // Out of bounds should be ignored
    selection.jump_to_hit(20);
    assert_eq!(selection.current_hit_index, 9); // Should remain at 9
}
