//! Inspector panel state persistence.
//!
//! Serializes panel geometry, active tab, watch table entries, and filter queries to
//! `~/.config/openomsi/inspector_layout.json`. Auto-saves on inspector exit and
//! auto-loads on inspector activation.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

/// Panel geometry and position on screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelGeometry {
    /// Screen X position in pixels.
    pub x: i32,
    /// Screen Y position in pixels.
    pub y: i32,
    /// Panel width in pixels.
    pub width: u32,
    /// Collapsed section flags (bitfield).
    pub collapsed_sections: u32,
}

impl Default for PanelGeometry {
    fn default() -> Self {
        Self {
            x: 100,
            y: 100,
            width: 420,
            collapsed_sections: 0,
        }
    }
}

/// Active inspector tab identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InspectorTab {
    Transforms,
    Material,
    ScriptVM,
    RenderPass,
    Kinematics,
    AudioEnv,
}

impl Default for InspectorTab {
    fn default() -> Self {
        Self::Transforms
    }
}

/// A watch expression for script variable monitoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchExpression {
    /// Variable path (e.g., "L.throttle", "S.door_state[0]").
    pub expression: String,
    /// User-assigned label (optional).
    pub label: Option<String>,
    /// Enable sparkline history graph.
    pub show_sparkline: bool,
}

/// Persistent inspector layout state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectorLayout {
    /// Panel geometry and screen position.
    pub panel_geometry: PanelGeometry,
    /// Currently active tab.
    pub active_tab: InspectorTab,
    /// Watch table expressions.
    #[serde(default)]
    pub watch_table: Vec<WatchExpression>,
    /// Filter search query string.
    #[serde(default)]
    pub filter_query: String,
    /// Schema version for migration.
    #[serde(default)]
    pub version: u32,
}

impl Default for InspectorLayout {
    fn default() -> Self {
        Self {
            panel_geometry: PanelGeometry::default(),
            active_tab: InspectorTab::default(),
            watch_table: Vec::new(),
            filter_query: String::new(),
            version: 1,
        }
    }
}

impl InspectorLayout {
    /// Get the default config file path: `~/.config/openomsi/inspector_layout.json`.
    pub fn config_path() -> Result<PathBuf, io::Error> {
        let config_dir = if cfg!(target_os = "windows") {
            // Windows: %APPDATA%/openomsi
            dirs::config_dir()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No config directory"))?
                .join("openomsi")
        } else if cfg!(target_os = "macos") {
            // macOS: ~/Library/Application Support/openomsi
            dirs::config_dir()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No config directory"))?
                .join("openomsi")
        } else {
            // Linux/Unix: ~/.config/openomsi
            dirs::config_dir()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No config directory"))?
                .join("openomsi")
        };

        fs::create_dir_all(&config_dir)?;
        Ok(config_dir.join("inspector_layout.json"))
    }

    /// Save the layout to the config file.
    pub fn save(&self) -> Result<(), io::Error> {
        let path = Self::config_path()?;
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(&path, json)?;
        log::debug!("Saved inspector layout to {:?}", path);
        Ok(())
    }

    /// Load the layout from the config file. Returns default on missing/corrupt file.
    pub fn load() -> Result<Self, io::Error> {
        let path = Self::config_path()?;
        
        if !path.exists() {
            log::debug!("No inspector layout found, using defaults");
            return Ok(Self::default());
        }

        match fs::read_to_string(&path) {
            Ok(json) => match serde_json::from_str(&json) {
                Ok(layout) => {
                    log::debug!("Loaded inspector layout from {:?}", path);
                    Ok(layout)
                }
                Err(e) => {
                    log::warn!("Corrupt inspector layout file, using defaults: {}", e);
                    Ok(Self::default())
                }
            },
            Err(e) => {
                log::warn!("Failed to read inspector layout, using defaults: {}", e);
                Ok(Self::default())
            }
        }
    }

    /// Load or create with graceful error handling.
    pub fn load_or_default() -> Self {
        Self::load().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialization_roundtrip() {
        let layout = InspectorLayout {
            panel_geometry: PanelGeometry {
                x: 200,
                y: 300,
                width: 500,
                collapsed_sections: 0b0101,
            },
            active_tab: InspectorTab::ScriptVM,
            watch_table: vec![
                WatchExpression {
                    expression: "L.throttle".to_string(),
                    label: Some("Throttle".to_string()),
                    show_sparkline: true,
                },
                WatchExpression {
                    expression: "S.door_state[0]".to_string(),
                    label: None,
                    show_sparkline: false,
                },
            ],
            filter_query: "door".to_string(),
            version: 1,
        };

        let json = serde_json::to_string(&layout).unwrap();
        let decoded: InspectorLayout = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.panel_geometry.x, 200);
        assert_eq!(decoded.panel_geometry.y, 300);
        assert_eq!(decoded.active_tab, InspectorTab::ScriptVM);
        assert_eq!(decoded.watch_table.len(), 2);
        assert_eq!(decoded.filter_query, "door");
    }

    #[test]
    fn test_default_layout() {
        let layout = InspectorLayout::default();
        assert_eq!(layout.active_tab, InspectorTab::Transforms);
        assert_eq!(layout.watch_table.len(), 0);
        assert_eq!(layout.filter_query, "");
    }
}
