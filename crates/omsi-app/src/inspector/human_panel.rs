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

