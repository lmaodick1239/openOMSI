//! 3D skeletal bone visualization for the debug renderer.
//!
//! Renders color-coded debug lines connecting joint positions to visualize
//! the active bone hierarchy in 3D space.

use glam::{Mat4, Vec3};

/// Color palette for bone visualization.
pub struct BoneColors {
    /// Spine and torso (cyan).
    pub spine: [f32; 4],
    /// Head and neck (yellow).
    pub head: [f32; 4],
    /// Arms (green).
    pub arms: [f32; 4],
    /// Legs (magenta).
    pub legs: [f32; 4],
    /// Feet (orange).
    pub feet: [f32; 4],
}

impl Default for BoneColors {
    fn default() -> Self {
        BoneColors {
            spine: [0.0, 1.0, 1.0, 1.0],   // Cyan
            head: [1.0, 1.0, 0.0, 1.0],    // Yellow
            arms: [0.0, 1.0, 0.0, 1.0],    // Green
            legs: [1.0, 0.0, 1.0, 1.0],    // Magenta
            feet: [1.0, 0.5, 0.0, 1.0],    // Orange
        }
    }
}

/// A debug line segment for bone visualization.
#[derive(Debug, Clone, Copy)]
pub struct BoneLine {
    /// Start position (world space).
    pub start: Vec3,
    /// End position (world space).
    pub end: Vec3,
    /// RGBA color.
    pub color: [f32; 4],
}

/// Bone hierarchy connections for a human skeleton.
pub struct SkeletonHierarchy {
    /// Hip to spine.
    pub hip_to_spine: Option<(Vec3, Vec3)>,
    /// Spine to neck.
    pub spine_to_neck: Option<(Vec3, Vec3)>,
    /// Neck to head.
    pub neck_to_head: Option<(Vec3, Vec3)>,
    /// Hip to thighs (left, right).
    pub hip_to_thighs: [(Vec3, Vec3); 2],
    /// Thighs to shins (left, right).
    pub thighs_to_shins: [(Vec3, Vec3); 2],
    /// Shins to feet (left, right).
    pub shins_to_feet: [(Vec3, Vec3); 2],
    /// Shoulders to upper arms (left, right).
    pub shoulders_to_upper: [(Vec3, Vec3); 2],
    /// Upper arms to forearms (left, right).
    pub upper_to_fore: [(Vec3, Vec3); 2],
    /// Forearms to hands (left, right).
    pub fore_to_hands: [(Vec3, Vec3); 2],
}

impl SkeletonHierarchy {
    /// Extract skeleton hierarchy from bone transforms.
    pub fn from_bones(
        bones: &[omsi_sim::human::BoneTransform],
        transform: Mat4,
    ) -> Self {
        let transform_pos = |pos: Vec3| -> Vec3 {
            let p4 = transform * pos.extend(1.0);
            Vec3::new(p4.x, p4.y, p4.z)
        };

        let find_bone = |name: &str| -> Option<Vec3> {
            bones
                .iter()
                .find(|b| b.name == name)
                .map(|b| transform_pos(b.position))
        };

        let hip = find_bone("Hip");
        let spine = find_bone("Spine");
        let head = find_bone("Head");

        let left_thigh = find_bone("LeftThigh");
        let right_thigh = find_bone("RightThigh");
        let left_shin = find_bone("LeftShin");
        let right_shin = find_bone("RightShin");
        let left_foot = find_bone("LeftFoot");
        let right_foot = find_bone("RightFoot");

        let left_upper = find_bone("LeftUpperArm");
        let right_upper = find_bone("RightUpperArm");
        let left_fore = find_bone("LeftForearm");
        let right_fore = find_bone("RightForearm");
        let left_hand = find_bone("LeftHand");
        let right_hand = find_bone("RightHand");

        // Construct connections
        let hip_to_spine = hip.and_then(|h| spine.map(|s| (h, s)));
        let spine_to_neck = spine.and_then(|s| head.map(|h| (s, h)));
        let neck_to_head = spine_to_neck;

        let hip_to_thighs = [
            hip.and_then(|h| left_thigh.map(|t| (h, t))).unwrap_or((Vec3::ZERO, Vec3::ZERO)),
            hip.and_then(|h| right_thigh.map(|t| (h, t))).unwrap_or((Vec3::ZERO, Vec3::ZERO)),
        ];

        let thighs_to_shins = [
            left_thigh.and_then(|t| left_shin.map(|s| (t, s))).unwrap_or((Vec3::ZERO, Vec3::ZERO)),
            right_thigh.and_then(|t| right_shin.map(|s| (t, s))).unwrap_or((Vec3::ZERO, Vec3::ZERO)),
        ];

        let shins_to_feet = [
            left_shin.and_then(|s| left_foot.map(|f| (s, f))).unwrap_or((Vec3::ZERO, Vec3::ZERO)),
            right_shin.and_then(|s| right_foot.map(|f| (s, f))).unwrap_or((Vec3::ZERO, Vec3::ZERO)),
        ];

        let shoulders_to_upper = [
            spine.and_then(|s| left_upper.map(|u| (s, u))).unwrap_or((Vec3::ZERO, Vec3::ZERO)),
            spine.and_then(|s| right_upper.map(|u| (s, u))).unwrap_or((Vec3::ZERO, Vec3::ZERO)),
        ];

        let upper_to_fore = [
            left_upper.and_then(|u| left_fore.map(|f| (u, f))).unwrap_or((Vec3::ZERO, Vec3::ZERO)),
            right_upper.and_then(|u| right_fore.map(|f| (u, f))).unwrap_or((Vec3::ZERO, Vec3::ZERO)),
        ];

        let fore_to_hands = [
            left_fore.and_then(|f| left_hand.map(|h| (f, h))).unwrap_or((Vec3::ZERO, Vec3::ZERO)),
            right_fore.and_then(|f| right_hand.map(|h| (f, h))).unwrap_or((Vec3::ZERO, Vec3::ZERO)),
        ];

        SkeletonHierarchy {
            hip_to_spine,
            spine_to_neck,
            neck_to_head,
            hip_to_thighs,
            thighs_to_shins,
            shins_to_feet,
            shoulders_to_upper,
            upper_to_fore,
            fore_to_hands,
        }
    }

    /// Generate debug lines for rendering.
    pub fn to_lines(&self, colors: &BoneColors) -> Vec<BoneLine> {
        let mut lines = Vec::new();

        // Spine
        if let Some((start, end)) = self.hip_to_spine {
            lines.push(BoneLine { start, end, color: colors.spine });
        }
        if let Some((start, end)) = self.spine_to_neck {
            lines.push(BoneLine { start, end, color: colors.spine });
        }

        // Head
        if let Some((start, end)) = self.neck_to_head {
            lines.push(BoneLine { start, end, color: colors.head });
        }

        // Legs
        for (start, end) in &self.hip_to_thighs {
            if start.length() > 0.0 && end.length() > 0.0 {
                lines.push(BoneLine { start: *start, end: *end, color: colors.legs });
            }
        }
        for (start, end) in &self.thighs_to_shins {
            if start.length() > 0.0 && end.length() > 0.0 {
                lines.push(BoneLine { start: *start, end: *end, color: colors.legs });
            }
        }

        // Feet
        for (start, end) in &self.shins_to_feet {
            if start.length() > 0.0 && end.length() > 0.0 {
                lines.push(BoneLine { start: *start, end: *end, color: colors.feet });
            }
        }

        // Arms
        for (start, end) in &self.shoulders_to_upper {
            if start.length() > 0.0 && end.length() > 0.0 {
                lines.push(BoneLine { start: *start, end: *end, color: colors.arms });
            }
        }
        for (start, end) in &self.upper_to_fore {
            if start.length() > 0.0 && end.length() > 0.0 {
                lines.push(BoneLine { start: *start, end: *end, color: colors.arms });
            }
        }
        for (start, end) in &self.fore_to_hands {
            if start.length() > 0.0 && end.length() > 0.0 {
                lines.push(BoneLine { start: *start, end: *end, color: colors.arms });
            }
        }

        lines
    }
}

/// 3D skeleton visualizer.
pub struct SkeletonVisualizer {
    /// Color scheme.
    pub colors: BoneColors,
    /// Whether to draw joint spheres.
    pub draw_joints: bool,
    /// Joint sphere radius.
    pub joint_radius: f32,
}

impl Default for SkeletonVisualizer {
    fn default() -> Self {
        SkeletonVisualizer {
            colors: BoneColors::default(),
            draw_joints: true,
            joint_radius: 0.02,
        }
    }
}

impl SkeletonVisualizer {
    /// Create a new skeleton visualizer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate all debug primitives for a skeleton.
    pub fn generate_primitives(
        &self,
        bones: &[omsi_sim::human::BoneTransform],
        transform: Mat4,
    ) -> SkeletonDebugPrimitives {
        let hierarchy = SkeletonHierarchy::from_bones(bones, transform);
        let lines = hierarchy.to_lines(&self.colors);

        let joints = if self.draw_joints {
            bones
                .iter()
                .map(|b| {
                    let pos4 = transform * b.position.extend(1.0);
                    Vec3::new(pos4.x, pos4.y, pos4.z)
                })
                .collect()
        } else {
            Vec::new()
        };

        SkeletonDebugPrimitives {
            lines,
            joints,
            joint_radius: self.joint_radius,
        }
    }
}

/// Debug primitives for skeleton rendering.
pub struct SkeletonDebugPrimitives {
    /// Bone lines.
    pub lines: Vec<BoneLine>,
    /// Joint positions.
    pub joints: Vec<Vec3>,
    /// Joint sphere radius.
    pub joint_radius: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use omsi_sim::human::BoneTransform;

    #[test]
    fn test_bone_colors_default() {
        let colors = BoneColors::default();
        assert_eq!(colors.spine[0], 0.0); // Cyan
        assert_eq!(colors.spine[1], 1.0);
        assert_eq!(colors.spine[2], 1.0);
    }

    #[test]
    fn test_skeleton_hierarchy_empty() {
        let bones = Vec::new();
        let transform = Mat4::IDENTITY;
        let hierarchy = SkeletonHierarchy::from_bones(&bones, transform);
        
        assert!(hierarchy.hip_to_spine.is_none());
        assert!(hierarchy.spine_to_neck.is_none());
    }

    #[test]
    fn test_skeleton_visualizer() {
        let visualizer = SkeletonVisualizer::new();
        assert!(visualizer.draw_joints);
        assert!(visualizer.joint_radius > 0.0);
    }

    #[test]
    fn test_bone_line() {
        let line = BoneLine {
            start: Vec3::ZERO,
            end: Vec3::new(1.0, 0.0, 0.0),
            color: [1.0, 0.0, 0.0, 1.0],
        };
        
        assert_eq!(line.start, Vec3::ZERO);
        assert_eq!(line.end.x, 1.0);
    }
}
