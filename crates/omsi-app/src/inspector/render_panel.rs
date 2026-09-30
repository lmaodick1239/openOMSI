//! Render pipeline debugging panel UI for the inspector.
//!
//! Provides UI controls for frame graph visualization, pass toggles,
//! isolation modes, and collision hull overlays.

use omsi_render::inspector::{IsolationMode, PassInfo, RenderSnapshot};

/// UI state for the render debugging panel.
pub struct RenderPanelState {
    /// Whether the panel is expanded.
    pub expanded: bool,
    /// Selected pass name for detailed view.
    pub selected_pass: Option<String>,
    /// Wireframe mode enabled.
    pub wireframe_enabled: bool,
    /// Wireframe color (RGBA).
    pub wireframe_color: [f32; 4],
    /// Collision hulls overlay enabled.
    pub collision_hulls_enabled: bool,
    /// Surface normals visualization enabled.
    pub show_normals: bool,
    /// UV seam visualization enabled.
    pub show_uv_seams: bool,
}

impl Default for RenderPanelState {
    fn default() -> Self {
        Self {
            expanded: false,
            selected_pass: None,
            wireframe_enabled: false,
            wireframe_color: [0.0, 1.0, 0.0, 1.0],
            collision_hulls_enabled: false,
            show_normals: false,
            show_uv_seams: false,
        }
    }
}

impl RenderPanelState {
    /// Create a new render panel state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle a specific pass on/off.
    pub fn toggle_pass(&self, pass_name: &str) -> bool {
        // Returns the new state (for now just toggle)
        // In a real UI, this would track individual pass states
        true
    }

    /// Set isolation mode from UI action.
    pub fn set_isolation_from_ui(&self, entity_id: u64) -> IsolationMode {
        // Convert UI selection to isolation mode
        IsolationMode::IsolateMesh(omsi_render::inspector::frame_graph::EntityKey::Vehicle {
            id: entity_id,
        })
    }

    /// Format pass timing for display.
    pub fn format_pass_timing(&self, pass: &PassInfo) -> String {
        format!(
            "{}: {:.2}ms ({} calls, {} tris)",
            pass.name, pass.gpu_time_ms, pass.draw_calls, pass.triangles
        )
    }

    /// Generate flamegraph data from snapshot.
    pub fn generate_flamegraph_data(&self, snapshot: &RenderSnapshot) -> Vec<(String, f32, usize)> {
        let mut data = Vec::new();

        fn collect_pass_data(
            pass: &PassInfo,
            depth: usize,
            data: &mut Vec<(String, f32, usize)>,
        ) {
            data.push((pass.name.clone(), pass.gpu_time_ms, depth));
            for child in &pass.children {
                collect_pass_data(child, depth + 1, data);
            }
        }

        for pass in &snapshot.passes {
            collect_pass_data(pass, 0, &mut data);
        }

        data
    }

    /// Format total frame time with breakdown.
    pub fn format_frame_summary(&self, snapshot: &RenderSnapshot) -> String {
        let total_draw_calls: usize = snapshot
            .passes
            .iter()
            .map(|p| p.total_draw_calls())
            .sum();

        let total_triangles: usize = snapshot
            .passes
            .iter()
            .map(|p| p.total_triangles())
            .sum();

        format!(
            "Frame #{}: {:.2}ms total, {} draw calls, {} triangles",
            snapshot.frame_id, snapshot.total_gpu_time_ms, total_draw_calls, total_triangles
        )
    }

    /// Get hierarchical pass tree as text (for debugging/export).
    pub fn generate_pass_tree_text(&self, snapshot: &RenderSnapshot) -> String {
        let mut output = String::new();
        output.push_str(&format!(
            "[Frame #{} - Total GPU: {:.2}ms]\n",
            snapshot.frame_id, snapshot.total_gpu_time_ms
        ));

        fn format_pass(pass: &PassInfo, indent: usize, output: &mut String) {
            let indent_str = "  ".repeat(indent);
            let status = if pass.enabled { "✓" } else { "✗" };
            output.push_str(&format!(
                "{}{} [{}] {} ({:.2}ms, {} calls, {} tris)\n",
                indent_str,
                status,
                if pass.children.is_empty() {
                    "Pass"
                } else {
                    "Group"
                },
                pass.name,
                pass.gpu_time_ms,
                pass.draw_calls,
                pass.triangles
            ));

            for child in &pass.children {
                format_pass(child, indent + 1, output);
            }
        }

        for pass in &snapshot.passes {
            format_pass(pass, 0, &mut output);
        }

        output
    }
}

/// Render panel UI messages/commands.
#[derive(Debug, Clone)]
pub enum RenderPanelMessage {
    /// Toggle a specific render pass.
    TogglePass(String),
    /// Set isolation mode.
    SetIsolationMode(IsolationMode),
    /// Clear isolation mode.
    ClearIsolation,
    /// Toggle wireframe overlay.
    ToggleWireframe,
    /// Set wireframe color.
    SetWireframeColor([f32; 4]),
    /// Toggle collision hull overlay.
    ToggleCollisionHulls,
    /// Toggle surface normal visualization.
    ToggleNormals,
    /// Export frame graph to JSON.
    ExportFrameGraph,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_panel_default() {
        let panel = RenderPanelState::default();

        assert!(!panel.expanded);
        assert!(panel.selected_pass.is_none());
        assert!(!panel.wireframe_enabled);
        assert!(!panel.collision_hulls_enabled);
    }

    #[test]
    fn test_format_pass_timing() {
        let panel = RenderPanelState::new();

        let mut pass = PassInfo::new("Shadow Cascade 0");
        pass.gpu_time_ms = 1.5;
        pass.draw_calls = 42;
        pass.triangles = 15000;

        let formatted = panel.format_pass_timing(&pass);
        assert!(formatted.contains("Shadow Cascade 0"));
        assert!(formatted.contains("1.50ms"));
        assert!(formatted.contains("42 calls"));
        assert!(formatted.contains("15000 tris"));
    }

    #[test]
    fn test_flamegraph_generation() {
        let panel = RenderPanelState::new();

        let mut snapshot = RenderSnapshot::new(1);

        let mut root = PassInfo::new("Main Pass");
        root.gpu_time_ms = 5.0;

        let mut child1 = PassInfo::new("Shadow 0");
        child1.gpu_time_ms = 1.5;

        let mut child2 = PassInfo::new("Shadow 1");
        child2.gpu_time_ms = 1.2;

        root.add_child(child1);
        root.add_child(child2);

        snapshot.add_pass(root);

        let flamegraph = panel.generate_flamegraph_data(&snapshot);

        assert_eq!(flamegraph.len(), 3); // Root + 2 children
        assert_eq!(flamegraph[0].0, "Main Pass");
        assert_eq!(flamegraph[0].2, 0); // Depth 0
        assert_eq!(flamegraph[1].2, 1); // Depth 1
    }

    #[test]
    fn test_pass_tree_text_generation() {
        let panel = RenderPanelState::new();

        let mut snapshot = RenderSnapshot::new(123);
        snapshot.total_gpu_time_ms = 10.5;

        let mut pass = PassInfo::new("Opaque Pass");
        pass.gpu_time_ms = 3.0;
        pass.draw_calls = 100;
        pass.triangles = 50000;

        snapshot.add_pass(pass);

        let text = panel.generate_pass_tree_text(&snapshot);

        assert!(text.contains("Frame #123"));
        assert!(text.contains("10.50ms"));
        assert!(text.contains("Opaque Pass"));
        assert!(text.contains("3.00ms"));
    }
}
