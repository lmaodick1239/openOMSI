//! Tests for serialization round-trip with identity preservation.

use openomsi_game::inspector::*;

fn make_vehicle_target(generation: u64) -> SelectionTarget {
    SelectionTarget::Vehicle {
        key: VehicleKey::Player { generation },
        mesh: None,
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

#[test]
fn test_view_serialization_preserves_selection_target() {
    let target = make_vehicle_target(42);
    let selection = InspectorSelection::new(target.clone());
    let view = InspectorMainView::from(&selection);
    
    // Serialize and deserialize
    let json = serde_json::to_string(&view).expect("Serialization failed");
    let deserialized: InspectorMainView = serde_json::from_str(&json).expect("Deserialization failed");
    
    // Verify selection_target is preserved
    assert!(deserialized.selection_target.is_some());
    let restored_target = deserialized.selection_target.unwrap();
    
    match (&target, &restored_target) {
        (
            SelectionTarget::Vehicle { key: k1, mesh: m1 },
            SelectionTarget::Vehicle { key: k2, mesh: m2 },
        ) => {
            assert_eq!(k1, k2);
            assert_eq!(m1, m2);
        }
        _ => panic!("Target type mismatch after deserialization"),
    }
}

#[test]
fn test_with_snapshot_after_deserialization_validates_generation() {
    let target_gen42 = make_vehicle_target(42);
    let target_gen43 = make_vehicle_target(43);
    
    let selection = InspectorSelection::new(target_gen42.clone());
    let view = InspectorMainView::from(&selection);
    
    // Serialize and deserialize
    let json = serde_json::to_string(&view).expect("Serialization failed");
    let deserialized: InspectorMainView = serde_json::from_str(&json).expect("Deserialization failed");
    
    // Create snapshot with different generation
    let snapshot = InspectorSnapshot {
        target: target_gen43,
        position: Some([10.0, 20.0, 30.0]),
        rotation: None,
        bounds: None,
        model_path: None,
        mesh_name: None,
        metadata: vec![],
    };
    
    // Should reject stale snapshot even after deserialization
    let result = deserialized.with_snapshot(&snapshot);
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("mismatch") || err_msg.contains("do not match"));
}

#[test]
fn test_with_snapshot_after_deserialization_accepts_matching() {
    let target = make_vehicle_target(42);
    
    let selection = InspectorSelection::new(target.clone());
    let view = InspectorMainView::from(&selection);
    
    // Serialize and deserialize
    let json = serde_json::to_string(&view).expect("Serialization failed");
    let deserialized: InspectorMainView = serde_json::from_str(&json).expect("Deserialization failed");
    
    // Create snapshot with matching target
    let snapshot = InspectorSnapshot {
        target: target.clone(),
        position: Some([10.0, 20.0, 30.0]),
        rotation: Some([0.0, 0.0, 0.0, 1.0]),
        bounds: None,
        model_path: Some("vehicle.o3d".to_string()),
        mesh_name: Some("body".to_string()),
        metadata: vec![],
    };
    
    // Should succeed with matching target after deserialization
    let result = deserialized.with_snapshot(&snapshot);
    assert!(result.is_ok());
    let enriched = result.unwrap();
    assert_eq!(enriched.position, Some([10.0, 20.0, 30.0]));
    assert_eq!(enriched.model_path, Some("vehicle.o3d".to_string()));
}

#[test]
fn test_with_snapshot_human_generation_after_roundtrip() {
    let human_gen1 = make_human_target(5, 100);
    let human_gen2 = make_human_target(5, 101);
    
    let selection = InspectorSelection::new(human_gen1.clone());
    let view = InspectorMainView::from(&selection);
    
    // Serialize and deserialize
    let json = serde_json::to_string(&view).expect("Serialization failed");
    let deserialized: InspectorMainView = serde_json::from_str(&json).expect("Deserialization failed");
    
    // Try snapshot with different generation
    let snapshot = InspectorSnapshot {
        target: human_gen2,
        position: Some([10.0, 20.0, 30.0]),
        rotation: None,
        bounds: None,
        model_path: None,
        mesh_name: None,
        metadata: vec![],
    };
    
    // Should reject mismatched generation
    let result = deserialized.with_snapshot(&snapshot);
    assert!(result.is_err());
}

#[test]
fn test_deserialized_view_without_target_rejects_snapshot() {
    // Manually construct JSON without selection_target
    let json = r#"{
        "selection_status": "Selected",
        "entity_type": "Vehicle",
        "entity_identity": "Player Vehicle (gen 42)",
        "position": null,
        "rotation": null,
        "model_path": null,
        "mesh_name": null,
        "metadata": [],
        "penetration_stack": [],
        "current_hit_index": 0,
        "selection_target": null
    }"#;
    
    let view: InspectorMainView = serde_json::from_str(json).expect("Deserialization failed");
    
    let snapshot = InspectorSnapshot {
        target: make_vehicle_target(42),
        position: Some([10.0, 20.0, 30.0]),
        rotation: None,
        bounds: None,
        model_path: None,
        mesh_name: None,
        metadata: vec![],
    };
    
    // Should reject because identity is missing
    let result = view.with_snapshot(&snapshot);
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("missing selection target identity"));
}
