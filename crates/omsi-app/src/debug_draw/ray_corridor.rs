//! Ray corridor visualization for multi-hit penetration stack.
//!
//! Draws a visual ray from camera to furthest hit with color-coded segments
//! and sphere markers at intersection points for inspector debugging.

use glam::{Vec3, Vec4};
use crate::inspector_core::PenetrationHit;

/// Ray corridor renderer for visualizing penetration stack hits.
pub struct RayCorridorRenderer {
    /// Whether ray corridor rendering is enabled.
    enabled: bool,
    /// Sphere marker radius at intersection points.
    marker_radius: f32,
}

impl RayCorridorRenderer {
    /// Create a new ray corridor renderer.
    pub fn new() -> Self {
        Self {
            enabled: false,
            marker_radius: 0.05,
        }
    }

    /// Enable or disable ray corridor rendering.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if ray corridor rendering is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set sphere marker radius.
    pub fn set_marker_radius(&mut self, radius: f32) {
        self.marker_radius = radius.max(0.01);
    }

    /// Get sphere marker radius.
    pub fn marker_radius(&self) -> f32 {
        self.marker_radius
    }
}

impl Default for RayCorridorRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Draw visual ray corridor from camera to hits with color-coded segments.
///
/// Renders:
/// - Debug line from camera to furthest hit
/// - Color-coded segments between hits (gradient from green→yellow→red by depth)
/// - Sphere markers at each intersection point (radius 0.05m)
///
/// # Parameters
/// - `camera_origin`: Camera position in world space
/// - `ray_direction`: Ray direction (normalized)
/// - `hits`: Penetration stack hits, sorted by distance
/// - `current_index`: Currently selected hit index
/// - `scene`: Scene to add debug rendering to
///
/// # Performance
/// - Direct corona insertion: O(n) where n = number of hits
/// - No allocations beyond hit iteration
/// - Target: <0.1ms for up to 20 hits
pub fn draw_ray_corridor(
    camera_origin: Vec3,
    ray_direction: Vec3,
    hits: &[PenetrationHit],
    current_index: usize,
    scene: &mut omsi_render::Scene,
) {
    if hits.is_empty() {
        return;
    }

    let marker_radius = 0.05;
    let max_hits = hits.len();

    // Draw sphere markers at each intersection point with color gradient
    for (i, hit) in hits.iter().enumerate() {
        let hit_pos = camera_origin + ray_direction * hit.distance;
        
        // Color gradient: green → yellow → red by depth index
        let t = i as f32 / max_hits.max(1) as f32;
        let color = interpolate_corridor_color(t);
        
        // Larger marker for currently selected hit
        let size = if i == current_index { 0.12 } else { 0.08 };
        let brightness = if i == current_index { 3.0 } else { 2.0 };

        scene.coronas.push(omsi_render::Corona {
            position: glam::DVec3::from(hit_pos.as_dvec3()),
            size,
            color,
            brightness,
            ..Default::default()
        });

        // Draw small sphere marker at exact intersection
        scene.coronas.push(omsi_render::Corona {
            position: glam::DVec3::from(hit_pos.as_dvec3()),
            size: marker_radius as f64,
            color: [1.0, 1.0, 1.0], // White center dot
            brightness: 1.5,
            ..Default::default()
        });
    }

    // Draw line segments between consecutive hits with gradient colors
    for i in 0..hits.len().saturating_sub(1) {
        let start_pos = camera_origin + ray_direction * hits[i].distance;
        let end_pos = camera_origin + ray_direction * hits[i + 1].distance;
        
        // Interpolate positions along segment for smooth color gradient
        let segment_steps = 5;
        for step in 0..segment_steps {
            let t_segment = step as f32 / segment_steps as f32;
            let pos = start_pos + (end_pos - start_pos) * t_segment;
            
            // Overall gradient position considering both hit index and segment position
            let t_overall = (i as f32 + t_segment) / max_hits as f32;
            let color = interpolate_corridor_color(t_overall);

            scene.coronas.push(omsi_render::Corona {
                position: glam::DVec3::from(pos.as_dvec3()),
                size: 0.03,
                color,
                brightness: 1.0,
                ..Default::default()
            });
        }
    }

    // Draw line from camera to first hit (entry segment) - cyan
    if let Some(first_hit) = hits.first() {
        let first_pos = camera_origin + ray_direction * first_hit.distance;
        let steps = (first_hit.distance * 10.0).min(50.0) as usize; // 10 markers per meter, max 50
        
        for step in 0..steps {
            let t = step as f32 / steps.max(1) as f32;
            let pos = camera_origin + (first_pos - camera_origin) * t;
            
            scene.coronas.push(omsi_render::Corona {
                position: glam::DVec3::from(pos.as_dvec3()),
                size: 0.02,
                color: [0.2, 0.8, 0.9], // Cyan - matches inspector marker color
                brightness: 0.8,
                ..Default::default()
            });
        }
    }
}

/// Interpolate corridor color from green (near) → yellow (mid) → red (far).
///
/// # Parameters
/// - `t`: Normalized depth position [0.0, 1.0]
///
/// # Returns
/// RGB color [r, g, b] in range [0.0, 1.0]
fn interpolate_corridor_color(t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    
    if t < 0.5 {
        // Green → Yellow (first half)
        let local_t = t * 2.0;
        [local_t, 1.0, 0.0]
    } else {
        // Yellow → Red (second half)
        let local_t = (t - 0.5) * 2.0;
        [1.0, 1.0 - local_t, 0.0]
    }
}

/// Generate penetration depth indicator text for UI display.
///
/// Returns formatted string like "Object 3 of 7 (2.4m deep)".
///
/// # Parameters
/// - `current_index`: Currently selected hit index (0-based)
/// - `total_hits`: Total number of hits in stack
/// - `current_distance`: Distance to currently selected hit
///
/// # Returns
/// Formatted string for UI display
pub fn format_penetration_indicator(
    current_index: usize,
    total_hits: usize,
    current_distance: f32,
) -> String {
    if total_hits == 0 {
        return String::new();
    }
    
    format!(
        "Object {} of {} ({:.1}m deep)",
        current_index + 1,
        total_hits,
        current_distance
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_corridor_color_interpolation() {
        // Green at start
        let color_start = interpolate_corridor_color(0.0);
        assert_eq!(color_start, [0.0, 1.0, 0.0]);

        // Yellow at midpoint
        let color_mid = interpolate_corridor_color(0.5);
        assert_eq!(color_mid, [1.0, 1.0, 0.0]);

        // Red at end
        let color_end = interpolate_corridor_color(1.0);
        assert_eq!(color_end, [1.0, 0.0, 0.0]);

        // Gradient between green and yellow
        let color_quarter = interpolate_corridor_color(0.25);
        assert_eq!(color_quarter, [0.5, 1.0, 0.0]);
    }

    #[test]
    fn test_penetration_indicator_formatting() {
        let text = format_penetration_indicator(0, 5, 1.2);
        assert_eq!(text, "Object 1 of 5 (1.2m deep)");

        let text = format_penetration_indicator(2, 7, 2.4);
        assert_eq!(text, "Object 3 of 7 (2.4m deep)");

        let text = format_penetration_indicator(0, 0, 0.0);
        assert_eq!(text, "");
    }

    #[test]
    fn test_marker_radius_bounds() {
        let mut renderer = RayCorridorRenderer::new();
        
        renderer.set_marker_radius(0.1);
        assert_eq!(renderer.marker_radius(), 0.1);

        // Should clamp to minimum
        renderer.set_marker_radius(0.001);
        assert_eq!(renderer.marker_radius(), 0.01);

        renderer.set_marker_radius(-1.0);
        assert_eq!(renderer.marker_radius(), 0.01);
    }

    #[test]
    fn test_renderer_enable_disable() {
        let mut renderer = RayCorridorRenderer::new();
        
        assert!(!renderer.is_enabled());
        
        renderer.set_enabled(true);
        assert!(renderer.is_enabled());
        
        renderer.set_enabled(false);
        assert!(!renderer.is_enabled());
    }
}
