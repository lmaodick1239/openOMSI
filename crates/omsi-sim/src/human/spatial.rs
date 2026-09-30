//! Human spatial broadphase using cylinder approximation.
//!
//! Registers humans as cylinders in the main BVH system for efficient raycasting.
//! Cylinder dimensions: radius 0.3m, height 1.75m (adjusted per human scale).

use glam::Vec3;
use omsi_geometry::bvh::{Aabb, BvhPrimitive};
use super::HumanKey;

/// Cylinder approximation for a human in world space.
#[derive(Debug, Clone, Copy)]
pub struct HumanCylinder {
    /// Human key for identity.
    pub key: HumanKey,
    /// World-space position (center bottom of cylinder).
    pub position: Vec3,
    /// Facing direction (normalized, for oriented cylinder).
    pub direction: Vec3,
    /// Cylinder radius (m).
    pub radius: f32,
    /// Cylinder height (m).
    pub height: f32,
}

impl HumanCylinder {
    /// Create a cylinder approximation from human data.
    pub fn new(key: HumanKey, position: Vec3, direction: Vec3, height: f32) -> Self {
        Self {
            key,
            position,
            direction: direction.normalize_or_zero(),
            radius: 0.3,
            height,
        }
    }

    /// Create a standard adult-sized cylinder (1.75m tall).
    pub fn standard(key: HumanKey, position: Vec3, direction: Vec3) -> Self {
        Self::new(key, position, direction, 1.75)
    }

    /// Compute axis-aligned bounding box for BVH insertion.
    pub fn compute_aabb(&self) -> Aabb {
        let half_extent = Vec3::new(self.radius, self.radius, self.height * 0.5);
        let center = self.position + Vec3::new(0.0, 0.0, self.height * 0.5);
        
        Aabb::new(center - half_extent, center + half_extent)
    }

    /// Convert to BVH primitive with packed key in ID field.
    pub fn to_bvh_primitive(&self) -> BvhPrimitive {
        let aabb = self.compute_aabb();
        let id = pack_human_id(self.key);
        BvhPrimitive::new(aabb, id)
    }

    /// Test ray-cylinder intersection (used in narrowphase pre-filter).
    pub fn intersect_ray(&self, origin: Vec3, direction: Vec3) -> Option<f32> {
        let cylinder_base = self.position;
        
        // Project ray onto XY plane for infinite cylinder test
        let ray_origin_2d = Vec3::new(origin.x, origin.y, 0.0);
        let ray_dir_2d = Vec3::new(direction.x, direction.y, 0.0);
        let cylinder_base_2d = Vec3::new(cylinder_base.x, cylinder_base.y, 0.0);
        
        let oc = ray_origin_2d - cylinder_base_2d;
        let a = ray_dir_2d.dot(ray_dir_2d);
        
        if a < 1e-6 {
            // Ray parallel to cylinder axis, check if inside
            return if oc.length() < self.radius {
                Some(0.0)
            } else {
                None
            };
        }
        
        let b = 2.0 * oc.dot(ray_dir_2d);
        let c = oc.dot(oc) - self.radius * self.radius;
        let discriminant = b * b - 4.0 * a * c;
        
        if discriminant < 0.0 {
            return None;
        }
        
        let sqrt_disc = discriminant.sqrt();
        let t1 = (-b - sqrt_disc) / (2.0 * a);
        let t2 = (-b + sqrt_disc) / (2.0 * a);
        
        // Find first valid intersection within height bounds
        for &t in &[t1, t2] {
            if t < 0.0 {
                continue;
            }
            
            let hit_point = origin + direction * t;
            let height_offset = hit_point.z - cylinder_base.z;
            
            if height_offset >= 0.0 && height_offset <= self.height {
                return Some(t);
            }
        }
        
        None
    }
}

/// Pack HumanKey into u64 for BVH ID field.
#[inline]
fn pack_human_id(key: HumanKey) -> u64 {
    ((key.index as u64) << 32) | (key.generation as u64)
}

/// Unpack HumanKey from BVH ID field.
#[inline]
pub fn unpack_human_id(id: u64) -> HumanKey {
    let index = (id >> 32) as u32;
    let generation = (id & 0xFFFFFFFF) as u32;
    HumanKey::new(index, generation)
}

/// Human broadphase manager integrating with BVH system.
pub struct HumanBroadphase {
    /// Current frame's human cylinders.
    cylinders: Vec<HumanCylinder>,
}

impl HumanBroadphase {
    /// Create a new broadphase manager.
    pub fn new() -> Self {
        Self {
            cylinders: Vec::new(),
        }
    }

    /// Update cylinders for the current frame.
    pub fn update(&mut self, cylinders: Vec<HumanCylinder>) {
        self.cylinders = cylinders;
    }

    /// Get BVH primitives for current frame.
    pub fn get_primitives(&self) -> Vec<BvhPrimitive> {
        self.cylinders
            .iter()
            .map(|c| c.to_bvh_primitive())
            .collect()
    }

    /// Get cylinder by key (for narrowphase).
    pub fn get_cylinder(&self, key: HumanKey) -> Option<&HumanCylinder> {
        self.cylinders.iter().find(|c| c.key == key)
    }

    /// Get all cylinders (for testing).
    pub fn cylinders(&self) -> &[HumanCylinder] {
        &self.cylinders
    }
}

impl Default for HumanBroadphase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cylinder_aabb() {
        let key = HumanKey::new(0, 0);
        let cylinder = HumanCylinder::standard(
            key,
            Vec3::new(10.0, 20.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
        );
        
        let aabb = cylinder.compute_aabb();
        let center = aabb.center();
        
        assert!((center.x - 10.0).abs() < 1e-5);
        assert!((center.y - 20.0).abs() < 1e-5);
        assert!((center.z - 0.875).abs() < 1e-5);
    }

    #[test]
    fn test_pack_unpack_id() {
        let key = HumanKey::new(12345, 67890);
        let packed = pack_human_id(key);
        let unpacked = unpack_human_id(packed);
        
        assert_eq!(key, unpacked);
    }

    #[test]
    fn test_ray_cylinder_intersection() {
        let key = HumanKey::new(0, 0);
        let cylinder = HumanCylinder::standard(
            key,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
        );
        
        // Ray pointing at cylinder center
        let origin = Vec3::new(-5.0, 0.0, 0.875);
        let direction = Vec3::new(1.0, 0.0, 0.0);
        
        let hit = cylinder.intersect_ray(origin, direction);
        assert!(hit.is_some());
        
        // Ray missing cylinder
        let origin_miss = Vec3::new(-5.0, 5.0, 0.875);
        let hit_miss = cylinder.intersect_ray(origin_miss, direction);
        assert!(hit_miss.is_none());
    }
}
