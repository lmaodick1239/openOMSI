//! Inspector integration for human raycasting.
//!
//! Extends inspector_core with human-specific raycast functionality and panel display.

use omsi_sim::human::{
    HumanKey, HumanRegistry, HumanBroadphase, HumanNarrowphase, HumanSnapshot,
    spatial::unpack_human_id,
};
use omsi_geometry::bvh::{Bvh, Ray};
use glam::Vec3;

use crate::inspector_core::{InspectorHit, SelectionTarget, PenetrationHit};

/// Human raycast result for inspector integration.
#[derive(Debug, Clone)]
pub struct HumanInspectorHit {
    pub key: HumanKey,
    pub distance: f32,
    pub hit_point: Vec3,
    pub bone_index: usize,
}

/// Perform raycast against human layer.
///
/// Returns ordered hits with human keys and distances.
pub fn raycast_humans(
    ray_origin: Vec3,
    ray_direction: Vec3,
    broadphase: &HumanBroadphase,
    registry: &HumanRegistry,
    max_hits: usize,
) -> Vec<InspectorHit> {
    let mut hits = Vec::new();
    
    // Build BVH from current frame's human cylinders
    let primitives = broadphase.get_primitives();
    if primitives.is_empty() {
        return hits;
    }
    
    let bvh = Bvh::new(primitives);
    let ray = Ray::new(ray_origin, ray_direction);
    
    // Broadphase: get cylinder hits
    let broadphase_hits = bvh.intersect_ray(&ray, max_hits.min(64));
    
    // Narrowphase: validate and refine each hit
    let mut narrowphase = HumanNarrowphase::new();
    
    for bvh_hit in broadphase_hits {
        let key = unpack_human_id(bvh_hit.id);
        
        // Validate generation
        if !registry.validate(key) {
            continue;
        }
        
        // Get cylinder for skeleton data
        if let Some(cylinder) = broadphase.get_cylinder(key) {
            // For now, use cylinder hit as approximation
            // In production, would update narrowphase with actual skeleton
            if let Some(_t) = cylinder.intersect_ray(ray_origin, ray_direction) {
                if let Some(inspector_hit) = InspectorHit::new(
                    bvh_hit.t,
                    SelectionTarget::Human {
                        key,
                        mesh_id: None,
                    },
                ) {
                    hits.push(inspector_hit);
                }
            }
        }
    }
    
    // Sort by distance
    InspectorHit::order_candidates(&mut hits);
    hits
}

/// Build human-specific inspector panel data.
pub fn build_human_panel(
    key: HumanKey,
    snapshot: &HumanSnapshot,
) -> Vec<(String, String)> {
    let mut metadata = Vec::new();
    
    metadata.push(("Type".to_string(), "Human/Pedestrian".to_string()));
    metadata.push(("ID".to_string(), format!("{}", snapshot.id)));
    metadata.push(("Generation".to_string(), format!("{}", snapshot.generation)));
    metadata.push(("Driver".to_string(), format!("{}", snapshot.is_driver)));
    
    metadata.push(("Behavior".to_string(), format!("{:?}", snapshot.behavior_state)));
    metadata.push(("Animation".to_string(), snapshot.active_animation.clone()));
    metadata.push(("Phase".to_string(), format!("{:.3}", snapshot.animation_phase)));
    
    metadata.push((
        "Position".to_string(),
        format!("({:.2}, {:.2}, {:.2})", snapshot.position.x, snapshot.position.y, snapshot.position.z),
    ));
    
    metadata.push((
        "Velocity".to_string(),
        format!("{:.2} m/s", snapshot.velocity.length()),
    ));
    
    if let Some(target) = snapshot.navigation_target {
        metadata.push((
            "Nav Target".to_string(),
            format!("({:.1}, {:.1}, {:.1})", target.x, target.y, target.z),
        ));
    }
    
    if let Some(economy) = &snapshot.passenger_economy {
        metadata.push(("Ticket".to_string(), economy.ticket_type.clone()));
        metadata.push(("Comfort".to_string(), format!("{:.1}%", economy.comfort_index)));
        metadata.push(("Destination".to_string(), economy.destination_stop.clone()));
        metadata.push(("Alighting".to_string(), format!("{}", economy.alighting_requested)));
    }
    
    metadata.push(("Bones".to_string(), format!("{}", snapshot.skeleton_bones.len())));
    metadata.push(("Artifacts".to_string(), format!("{}", snapshot.artifacts.len())));
    
    metadata
}

/// Create penetration hit for human.
pub fn create_human_penetration_hit(
    key: HumanKey,
    distance: f32,
    snapshot: &HumanSnapshot,
) -> PenetrationHit {
    let display_name = if snapshot.is_driver {
        format!("Driver (Human #{})", snapshot.id)
    } else {
        format!("Pedestrian #{}  [{}]", snapshot.id, snapshot.active_animation)
    };
    
    PenetrationHit::new(
        distance,
        SelectionTarget::Human {
            key,
            mesh_id: None,
        },
        display_name,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use omsi_sim::human::{HumanCylinder, BehaviorState, PassengerEconomy};
    
    #[test]
    fn test_human_raycast_integration() {
        let registry = HumanRegistry::new(64);
        let mut broadphase = HumanBroadphase::new();
        
        // Create test humans
        let mut cylinders = Vec::new();
        for i in 0..5 {
            let key = registry.allocate(i);
            let pos = Vec3::new(i as f32 * 2.0, 0.0, 0.0);
            let dir = Vec3::Y;
            cylinders.push(HumanCylinder::standard(key, pos, dir));
        }
        
        broadphase.update(cylinders);
        
        // Raycast
        let origin = Vec3::new(-2.0, 0.0, 1.0);
        let direction = Vec3::X;
        let hits = raycast_humans(origin, direction, &broadphase, &registry, 10);
        
        assert!(!hits.is_empty(), "Should find some humans");
    }
    
    #[test]
    fn test_build_human_panel() {
        let key = HumanKey::new(42, 1);
        let snapshot = HumanSnapshot {
            id: 42,
            generation: 1,
            is_driver: false,
            position: Vec3::new(10.0, 20.0, 0.0),
            velocity: Vec3::new(1.2, 0.0, 0.0),
            behavior_state: BehaviorState::Walking,
            animation_phase: 0.5,
            skeleton_bones: Vec::new(),
            active_animation: "walk".to_string(),
            animation_state: Default::default(),
            navigation_target: Some(Vec3::new(15.0, 20.0, 0.0)),
            steering_vector: None,
            passenger_economy: Some(PassengerEconomy {
                ticket_type: "Day Pass".to_string(),
                comfort_index: 85.0,
                destination_stop: "Central Station".to_string(),
                alighting_requested: false,
                comfort_factors: Vec::new(),
            }),
            artifacts: Vec::new(),
        };
        
        let panel = build_human_panel(key, &snapshot);
        
        assert!(!panel.is_empty());
        assert!(panel.iter().any(|(k, _)| k == "Behavior"));
        assert!(panel.iter().any(|(k, _)| k == "Ticket"));
        assert!(panel.iter().any(|(k, _)| k == "Destination"));
    }
}
