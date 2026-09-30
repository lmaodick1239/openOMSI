//! Human inspector panel UI component.
//!
//! Displays skeletal hierarchy, animation state telemetry, navigation vectors,
//! and passenger economy data for selected human entities.

use glam::Vec3;
use omsi_sim::human::{AnimationArtifact, BoneTransform, HumanSnapshot};

/// Human inspector panel state.
pub struct HumanPanel {
    /// Currently selected human snapshot.
    pub snapshot: Option<HumanSnapshot>,
    /// UI state: collapsed bone tree nodes.
    pub collapsed_bones: Vec<String>,
    /// Animation playback control.
    pub playback_control: PlaybackControl,
}

/// Animation playback control state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackControl {
    /// Normal playback.
    Playing,
    /// Paused at current frame.
    Paused,
    /// Slow motion (0.1× speed).
    SlowMotion,
}

impl Default for PlaybackControl {
    fn default() -> Self {
        PlaybackControl::Playing
    }
}

impl Default for HumanPanel {
    fn default() -> Self {
        HumanPanel {
            snapshot: None,
            collapsed_bones: Vec::new(),
            playback_control: PlaybackControl::Playing,
        }
    }
}

impl HumanPanel {
    /// Create a new human panel.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update the panel with a new snapshot.
    pub fn update_snapshot(&mut self, snapshot: HumanSnapshot) {
        self.snapshot = Some(snapshot);
    }

    /// Clear the current selection.
    pub fn clear(&mut self) {
        self.snapshot = None;
    }

    /// Toggle bone tree node collapsed state.
    pub fn toggle_bone(&mut self, bone_name: &str) {
        if let Some(pos) = self.collapsed_bones.iter().position(|b| b == bone_name) {
            self.collapsed_bones.remove(pos);
        } else {
            self.collapsed_bones.push(bone_name.to_string());
        }
    }

    /// Check if a bone is collapsed.
    pub fn is_bone_collapsed(&self, bone_name: &str) -> bool {
        self.collapsed_bones.iter().any(|b| b == bone_name)
    }

    /// Set playback control mode.
    pub fn set_playback(&mut self, mode: PlaybackControl) {
        self.playback_control = mode;
    }

    /// Get current playback speed multiplier.
    pub fn playback_speed(&self) -> f32 {
        match self.playback_control {
            PlaybackControl::Playing => 1.0,
            PlaybackControl::Paused => 0.0,
            PlaybackControl::SlowMotion => 0.1,
        }
    }


/// Generate UI text description for a human snapshot (fallback for non-egui builds).
pub fn describe_human(snapshot: &HumanSnapshot) -> String {
    let mut lines = Vec::new();

    lines.push(format!("Human #{} (gen {})", snapshot.id, snapshot.generation));
    lines.push(format!("State: {:?}", snapshot.behavior_state));
    lines.push(format!(
        "Position: ({:.2}, {:.2}, {:.2})",
        snapshot.position.x, snapshot.position.y, snapshot.position.z
    ));
    lines.push(format!("Velocity: {:.2} m/s", snapshot.velocity.length()));
    lines.push(format!("Animation: {} @ {:.1}%", snapshot.active_animation, snapshot.animation_phase * 100.0));

    if !snapshot.skeleton_bones.is_empty() {
        lines.push(format!("Bones: {}", snapshot.skeleton_bones.len()));
    }

    if let Some(target) = snapshot.navigation_target {
        lines.push(format!(
            "Nav Target: ({:.2}, {:.2}, {:.2})",
            target.x, target.y, target.z
        ));
    }

    if let Some(economy) = &snapshot.passenger_economy {
        lines.push(format!(
            "Ticket: {} | Comfort: {:.0}% | Dest: {}",
            economy.ticket_type, economy.comfort_index, economy.destination_stop
        ));
    }

    if !snapshot.artifacts.is_empty() {
        lines.push(format!("⚠ {} artifact(s) detected", snapshot.artifacts.len()));
    }

    lines.join("\n")
}
