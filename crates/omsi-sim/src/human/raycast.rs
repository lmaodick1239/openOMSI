//! Human skeletal mesh narrowphase raycasting.
//!
//! After BVH cylinder hit, tests against actual skeletal mesh using 15-bone hierarchy.
//! Returns precise hit point, bone index, and distance.

use glam::{Affine3A, Vec3};
use super::{HumanKey, SLOTS};

/// Narrowphase raycast result with bone-level detail.
#[derive(Debug, Clone, Copy)]
pub struct HumanRaycastResult {
    /// Human key for identity.
    pub key: HumanKey,
    /// Hit bone index (0-14 for the 15 bones).
    pub bone_index: usize,
    /// World-space hit point.
    pub hit_point: Vec3,
    /// Distance from ray origin.
    pub distance: f32,
}

impl HumanRaycastResult {
    /// Create a new raycast result.
    pub fn new(key: HumanKey, bone_index: usize, hit_point: Vec3, distance: f32) -> Self {
        Self {
            key,
            bone_index,
            hit_point,
            distance,
        }
    }
}

/// Bone capsule approximation for narrowphase testing.
#[derive(Debug, Clone, Copy)]
struct BoneCapsule {
    /// Bone start position (world space).
    start: Vec3,
    /// Bone end position (world space).
    end: Vec3,
    /// Capsule radius (m).
    radius: f32,
}

impl BoneCapsule {
    /// Create a capsule from bone transform and length.
    fn from_transform(transform: &Affine3A, length: f32, radius: f32) -> Self {
        let start = Vec3::from(transform.translation);
        let direction = transform.matrix3.y_axis.normalize();
        let end = start + Vec3::from(direction) * length;
        
        Self { start, end, radius }
    }

    /// Test ray-capsule intersection.
    fn intersect_ray(&self, origin: Vec3, direction: Vec3) -> Option<f32> {
        let segment = self.end - self.start;
        let segment_len = segment.length();
        
        if segment_len < 1e-6 {
            // Degenerate capsule, treat as sphere
            return intersect_ray_sphere(origin, direction, self.start, self.radius);
        }
        
        let segment_dir = segment / segment_len;
        let oc = origin - self.start;
        
        // Project ray onto capsule axis to find closest approach
        let dot_seg_dir = direction.dot(segment_dir);
        let dot_oc_dir = oc.dot(direction);
        let dot_oc_seg = oc.dot(segment_dir);
        
        // Solve for closest point on infinite line to capsule axis
        let a = 1.0 - dot_seg_dir * dot_seg_dir;
        
        if a < 1e-6 {
            // Ray parallel to capsule, use cylinder test
            let dist_to_axis = (oc - segment_dir * dot_oc_seg).length();
            if dist_to_axis > self.radius {
                return None;
            }
            
            // Find entry point along ray
            let t = dot_oc_seg / direction.dot(segment_dir).max(1e-6);
            return if t >= 0.0 { Some(t) } else { None };
        }
        
        let b = dot_oc_dir - dot_seg_dir * dot_oc_seg;
        let c = oc.dot(oc) - dot_oc_seg * dot_oc_seg - self.radius * self.radius;
        
        let discriminant = b * b - a * c;
        if discriminant < 0.0 {
            return None;
        }
        
        let sqrt_disc = discriminant.sqrt();
        let t1 = (-b - sqrt_disc) / a;
        let t2 = (-b + sqrt_disc) / a;
        
        // Check both intersection points
        for &t in &[t1, t2] {
            if t < 0.0 {
                continue;
            }
            
            let point = origin + direction * t;
            let point_on_axis = self.start + segment_dir * (point - self.start).dot(segment_dir);
            let axis_t = (point_on_axis - self.start).dot(segment_dir);
            
            if axis_t >= 0.0 && axis_t <= segment_len {
                return Some(t);
            }
        }
        
        // Check sphere caps
        let t_start = intersect_ray_sphere(origin, direction, self.start, self.radius);
        let t_end = intersect_ray_sphere(origin, direction, self.end, self.radius);
        
        match (t_start, t_end) {
            (Some(t1), Some(t2)) => Some(t1.min(t2)),
            (Some(t), None) | (None, Some(t)) => Some(t),
            (None, None) => None,
        }
    }
}

/// Ray-sphere intersection helper.
fn intersect_ray_sphere(origin: Vec3, direction: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let oc = origin - center;
    let a = direction.dot(direction);
    let b = 2.0 * oc.dot(direction);
    let c = oc.dot(oc) - radius * radius;
    
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }
    
    let sqrt_disc = discriminant.sqrt();
    let t1 = (-b - sqrt_disc) / (2.0 * a);
    let t2 = (-b + sqrt_disc) / (2.0 * a);
    
    if t1 >= 0.0 {
        Some(t1)
    } else if t2 >= 0.0 {
        Some(t2)
    } else {
        None
    }
}

/// Human skeletal mesh narrowphase raycaster.
pub struct HumanNarrowphase {
    /// Cached bone capsules for current frame.
    bone_capsules: Vec<BoneCapsule>,
}

impl HumanNarrowphase {
    /// Create a new narrowphase raycaster.
    pub fn new() -> Self {
        Self {
            bone_capsules: Vec::new(),
        }
    }

    /// Update bone capsules from skeletal pose.
    ///
    /// Uses the 15-bone hierarchy from animation system:
    /// - Hip (pelvis)
    /// - Spine (torso)
    /// - Head
    /// - Left/Right: Thigh, Shin, Foot, UpperArm, Forearm, Hand
    pub fn update_skeleton(&mut self, bone_transforms: &[Affine3A; SLOTS]) {
        self.bone_capsules.clear();
        
        // Bone definitions: (index, length, radius)
        let bone_specs = [
            (8, 0.15, 0.12),   // Hip
            (9, 0.45, 0.14),   // Spine
            (10, 0.20, 0.10),  // Head
            (0, 0.40, 0.09),   // Left Thigh
            (1, 0.40, 0.09),   // Right Thigh
            (2, 0.40, 0.07),   // Left Shin
            (3, 0.40, 0.07),   // Right Shin
            (13, 0.15, 0.05),  // Left Foot
            (14, 0.15, 0.05),  // Right Foot
            (4, 0.28, 0.07),   // Left UpperArm
            (5, 0.28, 0.07),   // Right UpperArm
            (6, 0.25, 0.06),   // Left Forearm
            (7, 0.25, 0.06),   // Right Forearm
            (11, 0.10, 0.04),  // Left Hand
            (12, 0.10, 0.04),  // Right Hand
        ];
        
        for (bone_idx, length, radius) in bone_specs {
            if bone_idx < SLOTS {
                let capsule = BoneCapsule::from_transform(
                    &bone_transforms[bone_idx],
                    length,
                    radius,
                );
                self.bone_capsules.push(capsule);
            }
        }
    }

    /// Perform narrowphase raycast against skeletal mesh.
    ///
    /// Returns the closest hit with bone index and precise hit point.
    pub fn raycast(
        &self,
        key: HumanKey,
        origin: Vec3,
        direction: Vec3,
    ) -> Option<HumanRaycastResult> {
        let mut closest_hit: Option<(usize, f32, Vec3)> = None;
        
        for (bone_idx, capsule) in self.bone_capsules.iter().enumerate() {
            if let Some(t) = capsule.intersect_ray(origin, direction) {
                match closest_hit {
                    None => {
                        let hit_point = origin + direction * t;
                        closest_hit = Some((bone_idx, t, hit_point));
                    }
                    Some((_, closest_t, _)) if t < closest_t => {
                        let hit_point = origin + direction * t;
                        closest_hit = Some((bone_idx, t, hit_point));
                    }
                    _ => {}
                }
            }
        }
        
        closest_hit.map(|(bone_idx, distance, hit_point)| {
            HumanRaycastResult::new(key, bone_idx, hit_point, distance)
        })
    }
}

impl Default for HumanNarrowphase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ray_sphere_intersection() {
        let origin = Vec3::new(-5.0, 0.0, 0.0);
        let direction = Vec3::new(1.0, 0.0, 0.0);
        let center = Vec3::ZERO;
        let radius = 1.0;
        
        let hit = intersect_ray_sphere(origin, direction, center, radius);
        assert!(hit.is_some());
        assert!((hit.unwrap() - 4.0).abs() < 1e-5);
    }

    #[test]
    fn test_bone_capsule_intersection() {
        let capsule = BoneCapsule {
            start: Vec3::ZERO,
            end: Vec3::new(0.0, 1.0, 0.0),
            radius: 0.1,
        };
        
        // Ray hitting capsule
        let origin = Vec3::new(-1.0, 0.5, 0.0);
        let direction = Vec3::new(1.0, 0.0, 0.0);
        
        let hit = capsule.intersect_ray(origin, direction);
        assert!(hit.is_some());
        
        // Ray missing capsule
        let origin_miss = Vec3::new(-1.0, 2.0, 0.0);
        let hit_miss = capsule.intersect_ray(origin_miss, direction);
        assert!(hit_miss.is_none());
    }

    #[test]
    fn test_narrowphase_raycast() {
        let mut narrowphase = HumanNarrowphase::new();
        
        // Create simple T-pose skeleton
        let mut bones = [Affine3A::IDENTITY; SLOTS];
        
        // Hip at origin
        bones[8] = Affine3A::from_translation(Vec3::ZERO);
        
        // Spine above hip
        bones[9] = Affine3A::from_translation(Vec3::new(0.0, 0.0, 0.2));
        
        narrowphase.update_skeleton(&bones);
        
        let key = HumanKey::new(0, 0);
        let origin = Vec3::new(-2.0, 0.0, 0.2);
        let direction = Vec3::new(1.0, 0.0, 0.0);
        
        let result = narrowphase.raycast(key, origin, direction);
        assert!(result.is_some());
        
        if let Some(hit) = result {
            assert_eq!(hit.key, key);
            assert!(hit.distance > 0.0);
        }
    }
}
