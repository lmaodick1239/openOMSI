//! Tests for InspectorMainView snapshot validation with full identity checks.

use openomsi_game::inspector::*;

fn make_vehicle_target(generation: u64) -> SelectionTarget {
    SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation },
        mesh: None,
    }
}

fn make_vehicle_target_with_mesh(generation: u64, mesh: MeshIdentity) -> SelectionTarget {
    SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation },
        mesh: Some(mesh),
    }
}

fn make_human_target(id: u32, generation: u64) -> SelectionTarget {
    SelectionTarget::Human {
        key: HumanKey {
            id,
            generation,
            is_driver: false,
        },
        mesh_id: None,
    }
}

fn make_scenery_target(map_id: i64) -> SelectionTarget {
    SelectionTarget::Scenery {
        key: SceneryKey::Editable { map_id },
        mesh: None,
    }
}

fn make_mesh_identity(name: &str, index: usize) -> MeshIdentity {
    MeshIdentity {
        model_path: "test.o3d".to_string(),
        definition_index: index,
        disambiguator: None,
        mesh_name: name.to_string(),
    }
}

#[test]
fn test_with_snapshot_matching_generation() {
    let target = make_vehicle_target(1);
    let selection = InspectorSelection::new(target.clone());
    let view = InspectorMainView::from(&selection);

    let snapshot = InspectorSnapshot {
        target,
        position: Some([10.0, 20.0, 30.0]),
        rotation: Some([0.0, 0.0, 0.0, 1.0]),
        bounds: None,
        model_path: Some("vehicle.o3d".to_string()),
        mesh_name: Some("body".to_string()),
        metadata: vec![],
    };

    let result = view.with_snapshot(&snapshot);
    assert!(result.is_ok());
    let enriched = result.unwrap();
    assert_eq!(enriched.position, Some([10.0, 20.0, 30.0]));
    assert_eq!(enriched.model_path, Some("vehicle.o3d".to_string()));
}

#[test]
fn test_with_snapshot_stale_generation() {
    let target_gen1 = make_vehicle_target(1);
    let target_gen2 = make_vehicle_target(2);
    let selection = InspectorSelection::new(target_gen2);
    let view = InspectorMainView::from(&selection);

    let snapshot = InspectorSnapshot {
        target: target_gen1, // Stale generation
        position: Some([10.0, 20.0, 30.0]),
        rotation: None,
        bounds: None,
        model_path: None,
        mesh_name: None,
        metadata: vec![],
    };

    let result = view.with_snapshot(&snapshot);
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("mismatch") || err_msg.contains("do not match"));
}

#[test]
fn test_with_snapshot_mesh_identity_mismatch() {
    let mesh1 = make_mesh_identity("body", 0);
    let mesh2 = make_mesh_identity("wheel", 1);

    let target_mesh1 = make_vehicle_target_with_mesh(1, mesh1);
    let target_mesh2 = make_vehicle_target_with_mesh(1, mesh2);

    let selection = InspectorSelection::new(target_mesh1);
    let view = InspectorMainView::from(&selection);

    let snapshot = InspectorSnapshot {
        target: target_mesh2, // Different mesh
        position: Some([10.0, 20.0, 30.0]),
        rotation: None,
        bounds: None,
        model_path: None,
        mesh_name: None,
        metadata: vec![],
    };

    let result = view.with_snapshot(&snapshot);
    assert!(result.is_err());
}

#[test]
fn test_with_snapshot_mesh_identity_match() {
    let mesh = make_mesh_identity("body", 0);
    let target = make_vehicle_target_with_mesh(1, mesh.clone());

    let selection = InspectorSelection::new(target.clone());
    let view = InspectorMainView::from(&selection);

    let snapshot = InspectorSnapshot {
        target,
        position: Some([10.0, 20.0, 30.0]),
        rotation: None,
        bounds: None,
        model_path: Some("vehicle.o3d".to_string()),
        mesh_name: Some("body".to_string()),
        metadata: vec![],
    };

    let result = view.with_snapshot(&snapshot);
    assert!(result.is_ok());
}

#[test]
fn test_with_snapshot_different_entity_types() {
    let vehicle_target = make_vehicle_target(1);
    let human_target = make_human_target(1, 1);

    let selection = InspectorSelection::new(vehicle_target);
    let view = InspectorMainView::from(&selection);

    let snapshot = InspectorSnapshot {
        target: human_target, // Wrong type
        position: Some([10.0, 20.0, 30.0]),
        rotation: None,
        bounds: None,
        model_path: None,
        mesh_name: None,
        metadata: vec![],
    };

    let result = view.with_snapshot(&snapshot);
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("mismatch") || err_msg.contains("do not match"));
}

#[test]
fn test_with_snapshot_no_selection() {
    let selection = InspectorSelection::default();
    let view = InspectorMainView::from(&selection);

    let snapshot = InspectorSnapshot {
        target: make_vehicle_target(1),
        position: Some([10.0, 20.0, 30.0]),
        rotation: None,
        bounds: None,
        model_path: None,
        mesh_name: None,
        metadata: vec![],
    };

    let result = view.with_snapshot(&snapshot);
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("no selection"));
}

#[test]
fn test_with_snapshot_invalidated_selection() {
    let mut selection = InspectorSelection::new(make_vehicle_target(1));
    selection.invalidate("Entity was destroyed".to_string());
    let view = InspectorMainView::from(&selection);

    let snapshot = InspectorSnapshot {
        target: make_vehicle_target(1),
        position: Some([10.0, 20.0, 30.0]),
        rotation: None,
        bounds: None,
        model_path: None,
        mesh_name: None,
        metadata: vec![],
    };

    let result = view.with_snapshot(&snapshot);
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("invalidated") || err_msg.contains("Invalidated"));
}

#[test]
fn test_with_snapshot_human_generation_mismatch() {
    let human_gen1 = make_human_target(42, 1);
    let human_gen2 = make_human_target(42, 2);

    let selection = InspectorSelection::new(human_gen2);
    let view = InspectorMainView::from(&selection);

    let snapshot = InspectorSnapshot {
        target: human_gen1,
        position: Some([10.0, 20.0, 30.0]),
        rotation: None,
        bounds: None,
        model_path: None,
        mesh_name: None,
        metadata: vec![],
    };

    let result = view.with_snapshot(&snapshot);
    assert!(result.is_err());
}

#[test]
fn test_with_snapshot_scenery_key_mismatch() {
    let scenery1 = make_scenery_target(100);
    let scenery2 = make_scenery_target(200);

    let selection = InspectorSelection::new(scenery1);
    let view = InspectorMainView::from(&selection);

    let snapshot = InspectorSnapshot {
        target: scenery2,
        position: Some([10.0, 20.0, 30.0]),
        rotation: None,
        bounds: None,
        model_path: None,
        mesh_name: None,
        metadata: vec![],
    };

    let result = view.with_snapshot(&snapshot);
    assert!(result.is_err());
}
