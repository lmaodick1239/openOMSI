//! Collision hull and debug wireframe rendering.
//!
//! Provides visualization of physics collision shapes (AABB boxes, sphere bounds)
//! and wireframe overlay rendering for mesh debugging.

use glam::{Vec3, Mat4};
use wgpu;
use bytemuck::{Pod, Zeroable};

/// Vertex format for debug wireframe lines.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct WireframeVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

/// Collision hull types that can be visualized.
#[derive(Debug, Clone)]
pub enum CollisionHull {
    /// Axis-aligned bounding box (min, max).
    Aabb { min: Vec3, max: Vec3 },
    /// Sphere (center, radius).
    Sphere { center: Vec3, radius: f32 },
    /// Oriented bounding box (center, half_extents, rotation).
    Obb {
        center: Vec3,
        half_extents: Vec3,
        rotation: Mat4,
    },
    /// Cylinder (base, height, radius).
    Cylinder {
        base: Vec3,
        height: f32,
        radius: f32,
    },
}

/// Wireframe rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireframeMode {
    /// No wireframe rendering.
    Off,
    /// Overlay wireframe on top of solid geometry.
    Overlay,
    /// Replace solid geometry with wireframe only.
    Replace,
}

impl Default for WireframeMode {
    fn default() -> Self {
        Self::Off
    }
}

/// Debug wireframe and collision hull renderer.
pub struct DebugWireframeRenderer {
    /// Wireframe rendering mode.
    mode: WireframeMode,
    /// Wireframe line color.
    wireframe_color: [f32; 4],
    /// Line thickness (in pixels).
    line_thickness: f32,
    /// Collision hull wireframes to render this frame.
    collision_hulls: Vec<(CollisionHull, [f32; 4])>,
}

impl DebugWireframeRenderer {
    /// Create a new debug wireframe renderer.
    pub fn new() -> Self {
        Self {
            mode: WireframeMode::Off,
            wireframe_color: [0.0, 1.0, 0.0, 1.0], // Green
            line_thickness: 1.5,
            collision_hulls: Vec::new(),
        }
    }

    /// Set wireframe rendering mode.
    pub fn set_mode(&mut self, mode: WireframeMode) {
        self.mode = mode;
    }

    /// Get current wireframe mode.
    pub fn mode(&self) -> WireframeMode {
        self.mode
    }

    /// Set wireframe line color.
    pub fn set_color(&mut self, color: [f32; 4]) {
        self.wireframe_color = color;
    }

    /// Get wireframe line color.
    pub fn color(&self) -> [f32; 4] {
        self.wireframe_color
    }

    /// Set line thickness.
    pub fn set_line_thickness(&mut self, thickness: f32) {
        self.line_thickness = thickness.max(0.5);
    }

    /// Get line thickness.
    pub fn line_thickness(&self) -> f32 {
        self.line_thickness
    }

    /// Add a collision hull to render this frame.
    pub fn add_collision_hull(&mut self, hull: CollisionHull, color: [f32; 4]) {
        self.collision_hulls.push((hull, color));
    }

    /// Clear all collision hulls (called at frame start).
    pub fn clear_collision_hulls(&mut self) {
        self.collision_hulls.clear();
    }

    /// Generate wireframe vertices for an AABB.
    pub fn generate_aabb_wireframe(min: Vec3, max: Vec3, color: [f32; 4]) -> Vec<WireframeVertex> {
        let corners = [
            [min.x, min.y, min.z],
            [max.x, min.y, min.z],
            [max.x, max.y, min.z],
            [min.x, max.y, min.z],
            [min.x, min.y, max.z],
            [max.x, min.y, max.z],
            [max.x, max.y, max.z],
            [min.x, max.y, max.z],
        ];

        // 12 edges of the box
        let indices = [
            // Bottom face
            0, 1, 1, 2, 2, 3, 3, 0,
            // Top face
            4, 5, 5, 6, 6, 7, 7, 4,
            // Vertical edges
            0, 4, 1, 5, 2, 6, 3, 7,
        ];

        indices
            .iter()
            .map(|&i| WireframeVertex {
                position: corners[i],
                color,
            })
            .collect()
    }

    /// Generate wireframe vertices for a sphere (approximated with lat/long lines).
    pub fn generate_sphere_wireframe(
        center: Vec3,
        radius: f32,
        color: [f32; 4],
        segments: u32,
    ) -> Vec<WireframeVertex> {
        let mut vertices = Vec::new();
        let segments = segments.max(8);

        // Latitude circles
        for lat in 0..segments {
            let theta1 = (lat as f32 * std::f32::consts::PI) / segments as f32;
            let theta2 = ((lat + 1) as f32 * std::f32::consts::PI) / segments as f32;

            for lon in 0..segments {
                let phi1 = (lon as f32 * 2.0 * std::f32::consts::PI) / segments as f32;
                let phi2 = ((lon + 1) as f32 * 2.0 * std::f32::consts::PI) / segments as f32;

                let p1 = Vec3::new(
                    radius * theta1.sin() * phi1.cos(),
                    radius * theta1.cos(),
                    radius * theta1.sin() * phi1.sin(),
                ) + center;

                let p2 = Vec3::new(
                    radius * theta1.sin() * phi2.cos(),
                    radius * theta1.cos(),
                    radius * theta1.sin() * phi2.sin(),
                ) + center;

                vertices.push(WireframeVertex {
                    position: p1.to_array(),
                    color,
                });
                vertices.push(WireframeVertex {
                    position: p2.to_array(),
                    color,
                });
            }
        }

        // Longitude circles
        for lon in 0..segments {
            let phi = (lon as f32 * 2.0 * std::f32::consts::PI) / segments as f32;

            for lat in 0..segments {
                let theta1 = (lat as f32 * std::f32::consts::PI) / segments as f32;
                let theta2 = ((lat + 1) as f32 * std::f32::consts::PI) / segments as f32;

                let p1 = Vec3::new(
                    radius * theta1.sin() * phi.cos(),
                    radius * theta1.cos(),
                    radius * theta1.sin() * phi.sin(),
                ) + center;

                let p2 = Vec3::new(
                    radius * theta2.sin() * phi.cos(),
                    radius * theta2.cos(),
                    radius * theta2.sin() * phi.sin(),
                ) + center;

                vertices.push(WireframeVertex {
                    position: p1.to_array(),
                    color,
                });
                vertices.push(WireframeVertex {
                    position: p2.to_array(),
                    color,
                });
            }
        }

        vertices
    }

    /// Get all collision hull wireframes for rendering.
    pub fn collision_hulls(&self) -> &[(CollisionHull, [f32; 4])] {
        &self.collision_hulls
    }
}

impl Default for DebugWireframeRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wireframe_mode() {
        let mut renderer = DebugWireframeRenderer::new();

        assert_eq!(renderer.mode(), WireframeMode::Off);

        renderer.set_mode(WireframeMode::Overlay);
        assert_eq!(renderer.mode(), WireframeMode::Overlay);
    }

    #[test]
    fn test_collision_hull_management() {
        let mut renderer = DebugWireframeRenderer::new();

        let hull = CollisionHull::Aabb {
            min: Vec3::new(-1.0, -1.0, -1.0),
            max: Vec3::new(1.0, 1.0, 1.0),
        };

        renderer.add_collision_hull(hull, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(renderer.collision_hulls().len(), 1);

        renderer.clear_collision_hulls();
        assert_eq!(renderer.collision_hulls().len(), 0);
    }

    #[test]
    fn test_aabb_wireframe_generation() {
        let min = Vec3::new(-1.0, -1.0, -1.0);
        let max = Vec3::new(1.0, 1.0, 1.0);
        let color = [1.0, 1.0, 1.0, 1.0];

        let vertices = DebugWireframeRenderer::generate_aabb_wireframe(min, max, color);

        // 12 edges * 2 vertices per edge = 24 vertices
        assert_eq!(vertices.len(), 24);
    }

    #[test]
    fn test_sphere_wireframe_generation() {
        let center = Vec3::ZERO;
        let radius = 1.0;
        let color = [1.0, 0.0, 1.0, 1.0];
        let segments = 16;

        let vertices = DebugWireframeRenderer::generate_sphere_wireframe(
            center, radius, color, segments,
        );

        // Should generate multiple line segments
        assert!(vertices.len() > 0);
        assert!(vertices.len() % 2 == 0); // Lines come in pairs
    }

    #[test]
    fn test_line_thickness_clamping() {
        let mut renderer = DebugWireframeRenderer::new();

        renderer.set_line_thickness(0.1); // Below minimum
        assert_eq!(renderer.line_thickness(), 0.5); // Clamped to minimum

        renderer.set_line_thickness(5.0);
        assert_eq!(renderer.line_thickness(), 5.0);
    }
}
