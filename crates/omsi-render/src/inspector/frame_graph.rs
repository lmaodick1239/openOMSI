//! Frame graph introspection and render pass management.
//!
//! Captures structural information about the render pipeline execution, including
//! pass hierarchy, GPU timing, draw call counts, and triangle counts. Supports
//! selective pass toggling and mesh isolation for debugging.

use std::collections::HashMap;

/// Entity identifier for mesh isolation modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityKey {
    Vehicle { id: u64 },
    Scenery { id: u64 },
}

/// Isolation rendering modes for debugging individual meshes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationMode {
    /// Normal rendering - all geometry visible.
    None,
    /// Render only the specified mesh, dim everything else.
    IsolateMesh(EntityKey),
    /// Hide the specified mesh completely.
    HideMesh(EntityKey),
    /// Ghost mode: dim/desaturate unselected geometry, highlight selected.
    GhostMode { selected: EntityKey },
}

impl Default for IsolationMode {
    fn default() -> Self {
        Self::None
    }
}

/// Information about a single render pass execution.
#[derive(Debug, Clone)]
pub struct PassInfo {
    /// Human-readable pass name (e.g., "Shadow Cascade 0", "Main PBR Pass").
    pub name: String,
    /// GPU execution time in milliseconds (0.0 if timing unavailable).
    pub gpu_time_ms: f32,
    /// Number of draw calls issued in this pass.
    pub draw_calls: usize,
    /// Total triangle count rendered in this pass.
    pub triangles: usize,
    /// Whether this pass is currently enabled (can be toggled off).
    pub enabled: bool,
    /// Child passes (for hierarchical rendering like shadow cascades).
    pub children: Vec<PassInfo>,
}

impl PassInfo {
    /// Create a new pass info entry.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            gpu_time_ms: 0.0,
            draw_calls: 0,
            triangles: 0,
            enabled: true,
            children: Vec::new(),
        }
    }

    /// Add a child pass to this pass.
    pub fn add_child(&mut self, child: PassInfo) {
        self.children.push(child);
    }

    /// Calculate total GPU time including all children.
    pub fn total_gpu_time_ms(&self) -> f32 {
        self.gpu_time_ms + self.children.iter().map(|c| c.total_gpu_time_ms()).sum::<f32>()
    }

    /// Calculate total draw calls including all children.
    pub fn total_draw_calls(&self) -> usize {
        self.draw_calls + self.children.iter().map(|c| c.total_draw_calls()).sum::<usize>()
    }

    /// Calculate total triangles including all children.
    pub fn total_triangles(&self) -> usize {
        self.triangles + self.children.iter().map(|c| c.total_triangles()).sum::<usize>()
    }
}

/// Complete snapshot of the render pipeline for a single frame.
#[derive(Debug, Clone)]
pub struct RenderSnapshot {
    /// Frame counter / unique frame ID.
    pub frame_id: u64,
    /// Hierarchical list of render passes executed this frame.
    pub passes: Vec<PassInfo>,
    /// Total GPU time for the entire frame (ms).
    pub total_gpu_time_ms: f32,
    /// List of active pipeline names (for debugging).
    pub active_pipelines: Vec<String>,
    /// Current isolation mode.
    pub isolation_mode: IsolationMode,
    /// Active pass toggles (pass name -> enabled).
    pub pass_toggles: HashMap<String, bool>,
}

impl RenderSnapshot {
    /// Create a new empty snapshot.
    pub fn new(frame_id: u64) -> Self {
        Self {
            frame_id,
            passes: Vec::new(),
            total_gpu_time_ms: 0.0,
            active_pipelines: Vec::new(),
            isolation_mode: IsolationMode::None,
            pass_toggles: HashMap::new(),
        }
    }

    /// Add a top-level pass to the snapshot.
    pub fn add_pass(&mut self, pass: PassInfo) {
        self.passes.push(pass);
    }

    /// Calculate total frame statistics.
    pub fn finalize(&mut self) {
        self.total_gpu_time_ms = self.passes.iter().map(|p| p.total_gpu_time_ms()).sum();
    }

    /// Check if a specific pass is enabled.
    pub fn is_pass_enabled(&self, pass_name: &str) -> bool {
        self.pass_toggles.get(pass_name).copied().unwrap_or(true)
    }
}

/// Frame graph inspector state.
///
/// Maintains render pass enable/disable state, isolation modes, and snapshot history.
pub struct FrameGraphInspector {
    /// Current isolation mode.
    isolation_mode: IsolationMode,
    /// Pass toggle states (pass name -> enabled).
    pass_enabled: HashMap<String, bool>,
    /// Most recent snapshot (for UI display).
    latest_snapshot: Option<RenderSnapshot>,
    /// Frame counter.
    frame_counter: u64,
}

impl FrameGraphInspector {
    /// Create a new frame graph inspector.
    pub fn new() -> Self {
        Self {
            isolation_mode: IsolationMode::None,
            pass_enabled: HashMap::new(),
            latest_snapshot: None,
            frame_counter: 0,
        }
    }

    /// Begin a new frame, returning the frame ID.
    pub fn begin_frame(&mut self) -> u64 {
        self.frame_counter += 1;
        self.frame_counter
    }

    /// Store a completed snapshot.
    pub fn store_snapshot(&mut self, snapshot: RenderSnapshot) {
        self.latest_snapshot = Some(snapshot);
    }

    /// Get the latest snapshot.
    pub fn latest_snapshot(&self) -> Option<&RenderSnapshot> {
        self.latest_snapshot.as_ref()
    }

    /// Toggle a render pass on or off.
    pub fn toggle_pass(&mut self, pass_name: &str, enabled: bool) {
        self.pass_enabled.insert(pass_name.to_string(), enabled);
    }

    /// Check if a pass is enabled.
    pub fn is_pass_enabled(&self, pass_name: &str) -> bool {
        self.pass_enabled.get(pass_name).copied().unwrap_or(true)
    }

    /// Set the isolation mode.
    pub fn set_isolation_mode(&mut self, mode: IsolationMode) {
        self.isolation_mode = mode;
    }

    /// Get the current isolation mode.
    pub fn isolation_mode(&self) -> IsolationMode {
        self.isolation_mode
    }

    /// Reset all overrides (called on inspector exit).
    pub fn reset(&mut self) {
        self.isolation_mode = IsolationMode::None;
        self.pass_enabled.clear();
    }

    /// Get all pass toggle states (for building snapshots).
    pub fn pass_toggles(&self) -> &HashMap<String, bool> {
        &self.pass_enabled
    }
}

impl Default for FrameGraphInspector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pass_info_hierarchy() {
        let mut root = PassInfo::new("Main Pass");
        root.gpu_time_ms = 5.0;
        root.draw_calls = 100;
        root.triangles = 50000;

        let mut child1 = PassInfo::new("Shadow Cascade 0");
        child1.gpu_time_ms = 1.5;
        child1.draw_calls = 30;
        child1.triangles = 15000;

        let mut child2 = PassInfo::new("Shadow Cascade 1");
        child2.gpu_time_ms = 1.2;
        child2.draw_calls = 25;
        child2.triangles = 12000;

        root.add_child(child1);
        root.add_child(child2);

        assert_eq!(root.total_gpu_time_ms(), 7.7);
        assert_eq!(root.total_draw_calls(), 155);
        assert_eq!(root.total_triangles(), 77000);
    }

    #[test]
    fn test_snapshot_finalize() {
        let mut snapshot = RenderSnapshot::new(1);

        let mut pass1 = PassInfo::new("Opaque");
        pass1.gpu_time_ms = 3.0;
        snapshot.add_pass(pass1);

        let mut pass2 = PassInfo::new("Transparent");
        pass2.gpu_time_ms = 2.0;
        snapshot.add_pass(pass2);

        snapshot.finalize();

        assert_eq!(snapshot.total_gpu_time_ms, 5.0);
    }

    #[test]
    fn test_inspector_pass_toggle() {
        let mut inspector = FrameGraphInspector::new();

        assert!(inspector.is_pass_enabled("Shadow"));

        inspector.toggle_pass("Shadow", false);
        assert!(!inspector.is_pass_enabled("Shadow"));

        inspector.toggle_pass("Shadow", true);
        assert!(inspector.is_pass_enabled("Shadow"));
    }

    #[test]
    fn test_isolation_mode() {
        let mut inspector = FrameGraphInspector::new();

        assert_eq!(inspector.isolation_mode(), IsolationMode::None);

        let entity = EntityKey::Vehicle { id: 42 };
        inspector.set_isolation_mode(IsolationMode::IsolateMesh(entity));

        assert_eq!(inspector.isolation_mode(), IsolationMode::IsolateMesh(entity));

        inspector.reset();
        assert_eq!(inspector.isolation_mode(), IsolationMode::None);
    }
}
