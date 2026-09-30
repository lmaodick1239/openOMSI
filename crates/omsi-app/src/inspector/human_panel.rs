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

    /// Render the panel UI (egui integration point).
    #[cfg(feature = "egui")]
    pub fn render(&mut self, ui: &mut egui::Ui) {
        use egui::{Color32, RichText};

        let Some(snapshot) = &self.snapshot else {
            ui.label("No human selected");
            return;
        };

        ui.heading(format!("Human #{}", snapshot.id));
        ui.separator();

        // Basic info
        ui.horizontal(|ui| {
            ui.label("State:");
            ui.label(format!("{:?}", snapshot.behavior_state));
        });

        ui.horizontal(|ui| {
            ui.label("Position:");
            ui.label(format!(
                "({:.2}, {:.2}, {:.2})",
                snapshot.position.x, snapshot.position.y, snapshot.position.z
            ));
        });

        ui.horizontal(|ui| {
            ui.label("Velocity:");
            ui.label(format!(
                "{:.2} m/s",
                snapshot.velocity.length()
            ));
        });

        ui.separator();

        // Animation state
        ui.collapsing("Animation State", |ui| {
            ui.horizontal(|ui| {
                ui.label("Active Clip:");
                ui.label(&snapshot.active_animation);
            });

            ui.horizontal(|ui| {
                ui.label("Phase:");
                ui.add(egui::ProgressBar::new(snapshot.animation_phase).text(format!("{:.1}%", snapshot.animation_phase * 100.0)));
            });

            for clip in &snapshot.animation_state.active_clips {
                ui.horizontal(|ui| {
                    ui.label(&clip.name);
                    ui.label(format!("weight: {:.2}", clip.weight));
                    ui.label(format!("speed: {:.2}×", clip.speed));
                });
            }

            ui.horizontal(|ui| {
                ui.label("Walk Weight:");
                ui.add(egui::ProgressBar::new(snapshot.animation_state.walk_weight));
            });

            ui.horizontal(|ui| {
                ui.label("Sit Weight:");
                ui.add(egui::ProgressBar::new(snapshot.animation_state.sit_weight));
            });
        });

        ui.separator();

        // Playback control
        ui.horizontal(|ui| {
            if ui.button("▶ Play").clicked() {
                self.playback_control = PlaybackControl::Playing;
            }
            if ui.button("⏸ Pause").clicked() {
                self.playback_control = PlaybackControl::Paused;
            }
            if ui.button("⏩ 0.1×").clicked() {
                self.playback_control = PlaybackControl::SlowMotion;
            }
        });

        ui.separator();

        // Skeletal hierarchy
        ui.collapsing("Skeleton", |ui| {
            self.render_bone_tree(ui, &snapshot.skeleton_bones);
        });

        ui.separator();

        // Navigation
        if let Some(target) = snapshot.navigation_target {
            ui.collapsing("Navigation", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Target:");
                    ui.label(format!("({:.2}, {:.2}, {:.2})", target.x, target.y, target.z));
                });

                if let Some(steering) = snapshot.steering_vector {
                    ui.horizontal(|ui| {
                        ui.label("Steering:");
                        ui.label(format!("({:.2}, {:.2}, {:.2})", steering.x, steering.y, steering.z));
                    });
                }
            });

            ui.separator();
        }

        // Passenger economy
        if let Some(economy) = &snapshot.passenger_economy {
            ui.collapsing("Passenger Economy", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Ticket:");
                    ui.label(&economy.ticket_type);
                });

                ui.horizontal(|ui| {
                    ui.label("Comfort:");
                    ui.add(egui::ProgressBar::new(economy.comfort_index / 100.0)
                        .text(format!("{:.0}%", economy.comfort_index)));
                });

                ui.horizontal(|ui| {
                    ui.label("Destination:");
                    ui.label(&economy.destination_stop);
                });

                ui.horizontal(|ui| {
                    ui.label("Alighting:");
                    ui.label(if economy.alighting_requested { "Requested" } else { "No" });
                });

                if !economy.comfort_factors.is_empty() {
                    ui.label("Comfort Factors:");
                    for (factor, value) in &economy.comfort_factors {
                        ui.horizontal(|ui| {
                            ui.label(format!("  • {}", factor));
                            ui.label(format!("{:.1}", value));
                        });
                    }
                }
            });

            ui.separator();
        }

        // Artifacts
        if !snapshot.artifacts.is_empty() {
            ui.collapsing("Artifacts", |ui| {
                for artifact in &snapshot.artifacts {
                    match artifact {
                        AnimationArtifact::FootSliding { side, velocity } => {
                            ui.colored_label(
                                Color32::YELLOW,
                                format!("⚠ Foot {} sliding: {:.2} m/s", if *side == 0 { "L" } else { "R" }, velocity)
                            );
                        }
                        AnimationArtifact::Clipping { bone_index } => {
                            ui.colored_label(
                                Color32::RED,
                                format!("⚠ Bone {} clipping", bone_index)
                            );
                        }
                        AnimationArtifact::IkMiss { side, distance } => {
                            ui.colored_label(
                                Color32::YELLOW,
                                format!("⚠ IK miss {} : {:.3} m", if *side == 0 { "L" } else { "R" }, distance)
                            );
                        }
                    }
                }
            });
        }
    }

    #[cfg(feature = "egui")]
    fn render_bone_tree(&mut self, ui: &mut egui::Ui, bones: &[BoneTransform]) {
        for bone in bones {
            let is_collapsed = self.is_bone_collapsed(&bone.name);
            let header = if is_collapsed {
                format!("▶ {}", bone.name)
            } else {
                format!("▼ {}", bone.name)
            };

            if ui.button(&header).clicked() {
                self.toggle_bone(&bone.name);
            }

            if !is_collapsed {
                ui.indent(&bone.name, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Pos:");
                        ui.label(format!(
                            "({:.3}, {:.3}, {:.3})",
                            bone.position.x, bone.position.y, bone.position.z
                        ));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Rot:");
                        ui.label(format!(
                            "P:{:.1}° Y:{:.1}° R:{:.1}°",
                            bone.euler_angles.x, bone.euler_angles.y, bone.euler_angles.z
                        ));
                    });
                });
            }
        }
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
