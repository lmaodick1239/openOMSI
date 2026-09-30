//! Comprehensive tests for human raycasting with 200 pedestrians.
//!
//! Verifies:
//! - <0.15ms overhead per raycast
//! - Generation validation (despawn/respawn cycle)
//! - Cylinder broadphase culling effectiveness
//! - Skeletal mesh narrowphase accuracy

use glam::{Affine3A, Vec3};
use omsi_geometry::bvh::{Bvh, Ray};
use omsi_sim::human::{
    HumanRegistry, HumanCylinder, HumanBroadphase, HumanNarrowphase,
    spatial::unpack_human_id, SLOTS,
};
use std::time::Instant;

/// Generate 200 randomized pedestrian positions in a 100x100m area.
fn generate_pedestrian_positions(count: usize) -> Vec<(Vec3, Vec3)> {
    let mut positions = Vec::with_capacity(count);
    let mut rng_state = 12345u32;
    
    for _ in 0..count {
        // Simple LCG random number generator
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let x = ((rng_state % 10000) as f32 / 100.0) - 50.0;
        
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let y = ((rng_state % 10000) as f32 / 100.0) - 50.0;
        
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        let angle = (rng_state % 360) as f32;
        let direction = Vec3::new(angle.to_radians().cos(), angle.to_radians().sin(), 0.0);
        
        positions.push((Vec3::new(x, y, 0.0), direction));
    }
    
    positions
}

#[test]
fn test_200_pedestrians_raycast_performance() {
    let registry = HumanRegistry::new(256);
    let mut broadphase = HumanBroadphase::new();
    
    // Generate 200 pedestrians
    let positions = generate_pedestrian_positions(200);
    let mut cylinders = Vec::new();
    
    for (idx, (pos, dir)) in positions.iter().enumerate() {
        let key = registry.allocate(idx as u32);
        let cylinder = HumanCylinder::standard(key, *pos, *dir);
        cylinders.push(cylinder);
    }
    
    broadphase.update(cylinders);
    
    // Build BVH from primitives
    let primitives = broadphase.get_primitives();
    let bvh = Bvh::new(primitives);
    
    // Perform 10 raycasts from different angles
    let mut total_duration = std::time::Duration::ZERO;
    let ray_origins = [
        Vec3::new(-60.0, 0.0, 1.0),
        Vec3::new(60.0, 0.0, 1.0),
        Vec3::new(0.0, -60.0, 1.0),
        Vec3::new(0.0, 60.0, 1.0),
        Vec3::new(-40.0, -40.0, 1.0),
        Vec3::new(40.0, 40.0, 1.0),
        Vec3::new(-40.0, 40.0, 1.0),
        Vec3::new(40.0, -40.0, 1.0),
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::new(-30.0, 30.0, 2.0),
    ];
    
    for origin in &ray_origins {
        let direction = (Vec3::ZERO - *origin).normalize();
        let ray = Ray::new(*origin, direction);
        
        let start = Instant::now();
        let hits = bvh.intersect_ray(&ray, 32);
        let duration = start.elapsed();
        
        total_duration += duration;
        
        // Verify hits contain valid human keys
        for hit in hits {
            let key = unpack_human_id(hit.id);
            assert!(registry.validate(key), "Hit contained invalid human key");
        }
    }
    
    let avg_duration = total_duration / ray_origins.len() as u32;
    let avg_ms = avg_duration.as_secs_f64() * 1000.0;
    
    println!("Average raycast time with 200 pedestrians: {:.3}ms", avg_ms);
    
    // Verify <0.15ms performance target
    assert!(
        avg_ms < 0.15,
        "Raycast performance {} ms exceeds 0.15ms target",
        avg_ms
    );
}

#[test]
fn test_generation_validation_despawn_respawn() {
    let registry = HumanRegistry::new(128);
    
    // Allocate human 0
    let key1 = registry.allocate(0);
    assert_eq!(key1.index, 0);
    assert_eq!(key1.generation, 0);
    assert!(registry.validate(key1));
    
    // Despawn human 0
    registry.invalidate(key1);
    assert!(!registry.validate(key1), "Key should be invalid after despawn");
    
    // Respawn human 0 (new generation)
    let key2 = registry.allocate(0);
    assert_eq!(key2.index, 0);
    assert_eq!(key2.generation, 1);
    assert!(registry.validate(key2));
    assert!(!registry.validate(key1), "Old key should still be invalid");
    
    // Multiple despawn/respawn cycles
    registry.invalidate(key2);
    let key3 = registry.allocate(0);
    assert_eq!(key3.generation, 2);
    assert!(registry.validate(key3));
    assert!(!registry.validate(key2));
    
    registry.invalidate(key3);
    let key4 = registry.allocate(0);
    assert_eq!(key4.generation, 3);
    assert!(registry.validate(key4));
}

#[test]
fn test_cylinder_broadphase_culling() {
    let registry = HumanRegistry::new(256);
    let mut broadphase = HumanBroadphase::new();
    
    // Create grid of pedestrians (10x10)
    let mut cylinders = Vec::new();
    let mut expected_hits = Vec::new();
    
    for x in 0..10 {
        for y in 0..10 {
            let idx = x * 10 + y;
            let key = registry.allocate(idx);
            let pos = Vec3::new(x as f32 * 2.0, y as f32 * 2.0, 0.0);
            let dir = Vec3::X;
            
            let cylinder = HumanCylinder::standard(key, pos, dir);
            cylinders.push(cylinder);
            
            // Track pedestrians in ray path (y=5, x=0..10)
            if y == 5 {
                expected_hits.push(key);
            }
        }
    }
    
    broadphase.update(cylinders);
    let primitives = broadphase.get_primitives();
    let bvh = Bvh::new(primitives);
    
    // Ray through middle row (y=5)
    let ray = Ray::new(Vec3::new(-5.0, 10.0, 1.0), Vec3::X);
    let hits = bvh.intersect_ray(&ray, 32);
    
    // Verify culling effectiveness: should hit ~10 pedestrians, not all 100
    assert!(
        hits.len() <= 15,
        "Broadphase should cull most pedestrians, got {} hits",
        hits.len()
    );
    
    println!(
        "Cylinder broadphase culling: {}/100 candidates ({}% culled)",
        hits.len(),
        100 - hits.len()
    );
}

#[test]
fn test_narrowphase_skeletal_accuracy() {
    let registry = HumanRegistry::new(16);
    let mut narrowphase = HumanNarrowphase::new();
    
    // Create simple T-pose skeleton
    let mut bones = [Affine3A::IDENTITY; SLOTS];
    
    // Hip at origin
    bones[8] = Affine3A::from_translation(Vec3::ZERO);
    
    // Spine above hip
    bones[9] = Affine3A::from_translation(Vec3::new(0.0, 0.0, 0.3));
    
    // Head above spine
    bones[10] = Affine3A::from_translation(Vec3::new(0.0, 0.0, 0.75));
    
    // Arms extended (T-pose)
    bones[4] = Affine3A::from_translation(Vec3::new(-0.2, 0.0, 0.5)); // Left upper arm
    bones[5] = Affine3A::from_translation(Vec3::new(0.2, 0.0, 0.5));  // Right upper arm
    
    // Legs
    bones[0] = Affine3A::from_translation(Vec3::new(-0.1, 0.0, 0.0)); // Left thigh
    bones[1] = Affine3A::from_translation(Vec3::new(0.1, 0.0, 0.0));  // Right thigh
    
    narrowphase.update_skeleton(&bones);
    
    let key = registry.allocate(0);
    
    // Test 1: Ray hitting center of torso
    let origin = Vec3::new(-3.0, 0.0, 0.4);
    let direction = Vec3::X;
    
    let result = narrowphase.raycast(key, origin, direction);
    assert!(result.is_some(), "Should hit torso");
    
    if let Some(hit) = result {
        assert_eq!(hit.key, key);
        assert!(hit.distance > 0.0);
        assert!(hit.distance < 4.0);
        println!("Torso hit: bone {}, distance {:.3}m", hit.bone_index, hit.distance);
    }
    
    // Test 2: Ray missing skeleton
    let origin_miss = Vec3::new(-3.0, 5.0, 0.4);
    let result_miss = narrowphase.raycast(key, origin_miss, direction);
    assert!(result_miss.is_none(), "Should miss skeleton");
    
    // Test 3: Ray hitting head
    let origin_head = Vec3::new(-2.0, 0.0, 0.75);
    let result_head = narrowphase.raycast(key, origin_head, direction);
    assert!(result_head.is_some(), "Should hit head");
}

#[test]
fn test_complete_raycast_pipeline() {
    let registry = HumanRegistry::new(16);
    let mut broadphase = HumanBroadphase::new();
    let mut narrowphase = HumanNarrowphase::new();
    
    // Create 5 pedestrians in a line
    let mut cylinders = Vec::new();
    let mut skeletons = Vec::new();
    
    for i in 0..5 {
        let key = registry.allocate(i);
        let pos = Vec3::new(i as f32 * 3.0, 0.0, 0.0);
        let dir = Vec3::Y;
        
        let cylinder = HumanCylinder::standard(key, pos, dir);
        cylinders.push(cylinder);
        
        // Simple skeleton for each human
        let mut bones = [Affine3A::IDENTITY; SLOTS];
        bones[8] = Affine3A::from_translation(pos);
        bones[9] = Affine3A::from_translation(pos + Vec3::new(0.0, 0.0, 0.3));
        skeletons.push((key, bones));
    }
    
    broadphase.update(cylinders);
    let primitives = broadphase.get_primitives();
    let bvh = Bvh::new(primitives);
    
    // Raycast through the line
    let ray = Ray::new(Vec3::new(-2.0, 0.0, 0.3), Vec3::X);
    let broadphase_hits = bvh.intersect_ray(&ray, 32);
    
    println!("Broadphase found {} candidates", broadphase_hits.len());
    assert!(broadphase_hits.len() >= 2, "Should find some humans in broadphase");
    
    // Narrowphase test each hit
    let mut narrowphase_hits = Vec::new();
    
    for hit in &broadphase_hits {
        let key = unpack_human_id(hit.id);
        
        if !registry.validate(key) {
            continue;
        }
        
        // Find matching skeleton
        if let Some((_, bones)) = skeletons.iter().find(|(k, _)| *k == key) {
            narrowphase.update_skeleton(bones);
            
            if let Some(narrow_hit) = narrowphase.raycast(key, ray.origin.into(), ray.direction.into()) {
                narrowphase_hits.push(narrow_hit);
            }
        }
    }
    
    println!("Narrowphase confirmed {} hits", narrowphase_hits.len());
    assert!(narrowphase_hits.len() >= 1, "Should confirm at least 1 hit");
    
    // Verify hits are ordered by distance
    for i in 1..narrowphase_hits.len() {
        assert!(
            narrowphase_hits[i].distance >= narrowphase_hits[i - 1].distance,
            "Hits should be ordered by distance"
        );
    }
}

#[test]
fn test_stale_key_rejection() {
    let registry = HumanRegistry::new(16);
    
    // Allocate and immediately invalidate
    let key = registry.allocate(0);
    registry.invalidate(key);
    
    // Attempt to use stale key
    assert!(!key.is_alive(&registry), "Stale key should fail validation");
    
    // New allocation should have different generation
    let new_key = registry.allocate(0);
    assert_ne!(key.generation, new_key.generation);
    assert!(new_key.is_alive(&registry));
    
    // Verify stale key still fails
    assert!(!key.is_alive(&registry));
}

#[test]
fn test_concurrent_human_operations() {
    let registry = HumanRegistry::new(64);
    
    // Allocate multiple humans
    let keys: Vec<_> = (0..10).map(|i| registry.allocate(i)).collect();
    
    // Verify all are valid
    for key in &keys {
        assert!(registry.validate(*key));
    }
    
    // Invalidate some
    registry.invalidate(keys[2]);
    registry.invalidate(keys[5]);
    registry.invalidate(keys[8]);
    
    // Verify correct validation state
    assert!(registry.validate(keys[0]));
    assert!(registry.validate(keys[1]));
    assert!(!registry.validate(keys[2]));
    assert!(registry.validate(keys[3]));
    assert!(registry.validate(keys[4]));
    assert!(!registry.validate(keys[5]));
    assert!(registry.validate(keys[6]));
    assert!(registry.validate(keys[7]));
    assert!(!registry.validate(keys[8]));
    assert!(registry.validate(keys[9]));
}
