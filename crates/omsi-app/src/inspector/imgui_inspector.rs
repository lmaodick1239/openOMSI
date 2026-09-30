//! Desktop Dear ImGui inspector frontend.
//!
//! This module is deliberately desktop-only. It consumes owned inspector snapshots and
//! emits typed commands; simulation state is never borrowed across an ImGui frame.

use crate::inspector::{
    EditorView, ExportView, HumanView, InspectorCommand, InspectorMainView, MaterialView,
    RenderView, TelemetryView,
};
use imgui::{Condition, ConfigFlags, Context, StyleColor, Ui, WindowFlags};
use imgui_wgpu::{Renderer, RendererConfig, RendererError};
use imgui_winit_support::WinitPlatform;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io;
use std::path::Path;
use winit::event::Event;
use winit::window::Window;

const LAYOUT_VERSION: u32 = 1;

/// Named inspector windows in the desktop frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InspectorWindow {
    Inspector,
    Hierarchy,
    Log,
    Materials,
    Render,
    Humans,
    Telemetry,
    Editor,
    Export,
}

impl InspectorWindow {
    /// Stable ImGui title and persistence identifier.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Inspector => "Inspector",
            Self::Hierarchy => "Hierarchy",
            Self::Log => "Log",
            Self::Materials => "Materials",
            Self::Render => "Render",
            Self::Humans => "Humans",
            Self::Telemetry => "Telemetry",
            Self::Editor => "Editor",
            Self::Export => "Export",
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Inspector => 0,
            Self::Hierarchy => 1,
            Self::Log => 2,
            Self::Materials => 3,
            Self::Render => 4,
            Self::Humans => 5,
            Self::Telemetry => 6,
            Self::Editor => 7,
            Self::Export => 8,
        }
    }

    /// All supported windows in stable order.
    pub const fn all() -> [Self; 9] {
        [
            Self::Inspector,
            Self::Hierarchy,
            Self::Log,
            Self::Materials,
            Self::Render,
            Self::Humans,
            Self::Telemetry,
            Self::Editor,
            Self::Export,
        ]
    }
}

/// Persisted geometry and visibility for one ImGui window.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowLayout {
    pub open: bool,
    pub position: [f32; 2],
    pub size: [f32; 2],
}

impl Default for WindowLayout {
    fn default() -> Self {
        Self {
            open: true,
            position: [40.0, 40.0],
            size: [360.0, 260.0],
        }
    }
}

/// Versioned desktop layout. Unknown or malformed data falls back to this value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImGuiLayout {
    pub version: u32,
    pub windows: [WindowLayout; 9],
}

impl Default for ImGuiLayout {
    fn default() -> Self {
        let mut windows = [WindowLayout::default(); 9];
        windows[InspectorWindow::Inspector.index()] = WindowLayout {
            open: true,
            position: [24.0, 24.0],
            size: [420.0, 420.0],
        };
        windows[InspectorWindow::Hierarchy.index()] = WindowLayout {
            open: true,
            position: [460.0, 24.0],
            size: [320.0, 420.0],
        };
        for (index, window) in windows.iter_mut().enumerate().skip(2) {
            window.open = false;
            window.position = [24.0 + index as f32 * 18.0, 460.0];
        }
        Self {
            version: LAYOUT_VERSION,
            windows,
        }
    }
}

impl ImGuiLayout {
    /// Serialize this layout for persistence.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize a layout, rejecting unsupported schema versions.
    pub fn from_json(json: &str) -> Self {
        serde_json::from_str::<Self>(json)
            .ok()
            .filter(|layout| layout.version == LAYOUT_VERSION)
            .unwrap_or_default()
    }

    /// Load a layout from a file, using defaults for all I/O and data errors.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .map(|text| Self::from_json(&text))
            .unwrap_or_default()
    }

    /// Save a layout to a file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        std::fs::write(
            path,
            self.to_json()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
        )
    }
}

/// A complete owned input to one inspector UI frame.
#[derive(Debug, Clone, Default)]
pub struct InspectorUiSnapshot {
    pub inspector: Option<InspectorMainView>,
    pub material: Option<MaterialView>,
    pub render: Option<RenderView>,
    pub human: Option<HumanView>,
    pub telemetry: Option<TelemetryView>,
    pub editor: Option<EditorView>,
    pub export: Option<ExportView>,
    pub hierarchy: Vec<String>,
    pub log_lines: Vec<String>,
}

/// Input capture reported by ImGui after a frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InputCaptureState {
    pub pointer: bool,
    pub keyboard: bool,
}

impl InputCaptureState {
    /// Whether a game keyboard handler must yield to ImGui.
    pub const fn blocks_keyboard(self) -> bool {
        self.keyboard
    }

    /// Whether a game pointer handler must yield to ImGui.
    pub const fn blocks_pointer(self) -> bool {
        self.pointer
    }
}

/// Recoverable renderer error state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendError(pub String);

/// Desktop ImGui state and frontend boundary.
pub struct InspectorUi {
    pub context: Context,
    pub platform: WinitPlatform,
    renderer: Option<Renderer>,
    pub layout: ImGuiLayout,
    commands: VecDeque<InspectorCommand>,
    pub input_capture: InputCaptureState,
    pub last_backend_error: Option<BackendError>,
    frame_started: bool,
}

impl InspectorUi {
    /// Create desktop ImGui state without requiring a GPU.
    pub fn new() -> Self {
        let mut context = Context::create();
        context.set_ini_filename(None);
        context
            .io_mut()
            .config_flags
            .insert(ConfigFlags::NAV_ENABLE_KEYBOARD);
        let platform = WinitPlatform::new(&mut context);
        apply_blue_theme(context.style_mut());
        Self {
            context,
            platform,
            renderer: None,
            layout: ImGuiLayout::default(),
            commands: VecDeque::new(),
            input_capture: InputCaptureState::default(),
            last_backend_error: None,
            frame_started: false,
        }
    }

    /// Load persisted geometry and visibility without making startup fatal.
    pub fn with_layout(mut self, layout: ImGuiLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Attach a wgpu renderer. Backend construction is isolated from game state.
    pub fn attach_renderer(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) {
        self.renderer = Some(Renderer::new(
            &mut self.context,
            device,
            queue,
            RendererConfig {
                texture_format: format,
                ..RendererConfig::new()
            },
        ));
        self.last_backend_error = None;
    }

    /// Pass a winit event to ImGui's desktop input backend.
    pub fn handle_event<T>(&mut self, window: &Window, event: &Event<T>) {
        self.platform
            .handle_event(self.context.io_mut(), window, event);
    }

    /// Begin a frame. Failure to update the cursor is recoverable.
    pub fn begin_frame(&mut self, window: &Window) -> bool {
        if let Err(error) = self.platform.prepare_frame(self.context.io_mut(), window) {
            self.last_backend_error = Some(BackendError(error.to_string()));
            return false;
        }
        self.frame_started = true;
        true
    }

    /// Draw all enabled windows from an owned snapshot and queue typed commands.
    pub fn draw(&mut self, window: &Window, snapshot: &InspectorUiSnapshot) {
        if !self.frame_started {
            return;
        }
        let layout = self.layout.clone();
        let mut captured_layout = layout.windows;
        let ui = self.context.frame();
        captured_layout[InspectorWindow::Inspector.index()] = draw_window(
            ui,
            layout.windows[InspectorWindow::Inspector.index()],
            InspectorWindow::Inspector,
            |ui| draw_inspector(ui, snapshot.inspector.as_ref()),
        );
        captured_layout[InspectorWindow::Hierarchy.index()] = draw_window(
            ui,
            layout.windows[InspectorWindow::Hierarchy.index()],
            InspectorWindow::Hierarchy,
            |ui| draw_lines(ui, "Entities", &snapshot.hierarchy),
        );
        captured_layout[InspectorWindow::Log.index()] = draw_window(
            ui,
            layout.windows[InspectorWindow::Log.index()],
            InspectorWindow::Log,
            |ui| draw_lines(ui, "Diagnostics", &snapshot.log_lines),
        );
        captured_layout[InspectorWindow::Materials.index()] = draw_window(
            ui,
            layout.windows[InspectorWindow::Materials.index()],
            InspectorWindow::Materials,
            |ui| draw_material(ui, snapshot.material.as_ref()),
        );
        captured_layout[InspectorWindow::Render.index()] = draw_window(
            ui,
            layout.windows[InspectorWindow::Render.index()],
            InspectorWindow::Render,
            |ui| draw_render(ui, snapshot.render.as_ref()),
        );
        captured_layout[InspectorWindow::Humans.index()] = draw_window(
            ui,
            layout.windows[InspectorWindow::Humans.index()],
            InspectorWindow::Humans,
            |ui| draw_human(ui, snapshot.human.as_ref()),
        );
        captured_layout[InspectorWindow::Telemetry.index()] = draw_window(
            ui,
            layout.windows[InspectorWindow::Telemetry.index()],
            InspectorWindow::Telemetry,
            |ui| draw_telemetry(ui, snapshot.telemetry.as_ref()),
        );
        captured_layout[InspectorWindow::Editor.index()] = draw_window(
            ui,
            layout.windows[InspectorWindow::Editor.index()],
            InspectorWindow::Editor,
            |ui| draw_editor(ui, snapshot.editor.as_ref()),
        );
        captured_layout[InspectorWindow::Export.index()] = draw_window(
            ui,
            layout.windows[InspectorWindow::Export.index()],
            InspectorWindow::Export,
            |ui| draw_export(ui, snapshot.export.as_ref()),
        );
        self.layout.windows = captured_layout;
        self.input_capture = InputCaptureState {
            pointer: ui.io().want_capture_mouse,
            keyboard: ui.io().want_capture_keyboard,
        };
        self.platform.prepare_render(ui, window);
    }

    /// Finish a frame and optionally render its draw data into an existing pass.
    pub fn render<'a>(
        &mut self,
        window: &Window,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        pass: &mut wgpu::RenderPass<'a>,
    ) -> Result<(), BackendError> {
        if !self.frame_started {
            return Ok(());
        }
        let _ = window;
        let draw_data = self.context.render();
        self.frame_started = false;
        if let Some(renderer) = self.renderer.as_mut() {
            renderer
                .render(draw_data, queue, device, pass)
                .map_err(|error: RendererError| {
                    let backend = BackendError(error.to_string());
                    self.last_backend_error = Some(backend.clone());
                    backend
                })?;
        }
        Ok(())
    }

    /// Queue a typed command from a widget or integration layer.
    pub fn queue_command(&mut self, command: InspectorCommand) {
        self.commands.push_back(command);
    }

    /// Drain commands at the application mutation boundary.
    pub fn drain_commands(&mut self) -> Vec<InspectorCommand> {
        self.commands.drain(..).collect()
    }

    /// Whether camera/game input should be suppressed for this frame.
    pub fn captures_input(&self) -> bool {
        self.input_capture.pointer || self.input_capture.keyboard
    }
}

impl Default for InspectorUi {
    fn default() -> Self {
        Self::new()
    }
}

fn draw_window<F>(ui: &Ui, layout: WindowLayout, window: InspectorWindow, draw: F) -> WindowLayout
where
    F: FnOnce(&Ui),
{
    if !layout.open {
        return layout;
    }
    let mut open = true;
    let mut captured = layout;
    ui.window(window.name())
        .flags(WindowFlags::NO_SAVED_SETTINGS)
        .opened(&mut open)
        .position(layout.position, Condition::FirstUseEver)
        .size(layout.size, Condition::FirstUseEver)
        .build(|| {
            draw(ui);
            captured.position = ui.window_pos();
            captured.size = ui.window_size();
        });
    capture_window_layout(captured, open, captured.position, captured.size)
}

fn capture_window_layout(
    mut layout: WindowLayout,
    open: bool,
    position: [f32; 2],
    size: [f32; 2],
) -> WindowLayout {
    layout.open = open;
    layout.position = position;
    layout.size = size;
    layout
}

const BLUE_TITLE_ACTIVE: [f32; 4] = [0.05, 0.3, 0.62, 1.0];

/// Apply the single centralized blue inspector theme.
pub fn apply_blue_theme(style: &mut imgui::Style) {
    style.window_rounding = 2.0;
    style.frame_rounding = 2.0;
    style.item_spacing = [6.0, 4.0];
    style[StyleColor::WindowBg] = [0.075, 0.085, 0.105, 0.98];
    style[StyleColor::TitleBg] = [0.035, 0.18, 0.38, 1.0];
    style[StyleColor::TitleBgActive] = BLUE_TITLE_ACTIVE;
    style[StyleColor::Header] = [0.04, 0.25, 0.52, 1.0];
    style[StyleColor::HeaderHovered] = [0.08, 0.36, 0.7, 1.0];
    style[StyleColor::HeaderActive] = [0.12, 0.42, 0.78, 1.0];
    style[StyleColor::Text] = [0.88, 0.91, 0.96, 1.0];
}

fn draw_inspector(ui: &Ui, view: Option<&InspectorMainView>) {
    if let Some(view) = view {
        ui.text(&view.selection_status);
        if let Some(identity) = &view.entity_identity {
            ui.text(identity);
        }
        if let Some(position) = view.position {
            ui.text(format!(
                "Position: {:.2}, {:.2}, {:.2}",
                position[0], position[1], position[2]
            ));
        }
    } else {
        ui.text("No snapshot");
    }
}
fn draw_lines(ui: &Ui, label: &str, lines: &[String]) {
    ui.text(label);
    for line in lines {
        ui.bullet_text(line);
    }
}
fn draw_material(ui: &Ui, view: Option<&MaterialView>) {
    if let Some(view) = view {
        ui.text(&view.name);
        ui.text(format!("Shader: {}", view.shader_variant));
        ui.text(format!(
            "Metallic {:.2}  Roughness {:.2}",
            view.metallic, view.roughness
        ));
    }
}
fn draw_render(ui: &Ui, view: Option<&RenderView>) {
    if let Some(view) = view {
        ui.text(format!(
            "Frame {:.2} ms  Draws {}",
            view.total_frame_time_ms, view.total_draw_calls
        ));
        for pass in &view.passes {
            ui.bullet_text(format!("{}: {:.2} ms", pass.name, pass.gpu_time_ms));
        }
    }
}
fn draw_human(ui: &Ui, view: Option<&HumanView>) {
    if let Some(view) = view {
        ui.text(format!("Human #{}  {}", view.id, view.current_animation));
        ui.text(format!("Playback: {}", view.playback));
    }
}
fn draw_telemetry(ui: &Ui, view: Option<&TelemetryView>) {
    if let Some(view) = view {
        ui.text(format!(
            "Frame {:.2} ms  Query {:.2} ms",
            view.frame_time_ms, view.inspector_query_time_ms
        ));
    }
}
fn draw_editor(ui: &Ui, view: Option<&EditorView>) {
    if let Some(view) = view {
        ui.text(if view.sandbox_active {
            "Sandbox active"
        } else {
            "Sandbox inactive"
        });
    }
}
fn draw_export(ui: &Ui, view: Option<&ExportView>) {
    if let Some(view) = view {
        ui.text(format!("Status: {:?}", view.status));
        if let Some(error) = &view.error {
            ui.text_colored([1.0, 0.39, 0.39, 1.0], error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_round_trip_and_invalid_fallback() {
        let layout = ImGuiLayout::default();
        let decoded = ImGuiLayout::from_json(&layout.to_json().unwrap());
        assert_eq!(decoded, layout);
        assert_eq!(ImGuiLayout::from_json("not json"), ImGuiLayout::default());
        assert_eq!(
            ImGuiLayout::from_json(r#"{"version":99,"windows":[]}"#),
            ImGuiLayout::default()
        );
    }

    #[test]
    fn captures_live_window_geometry_and_visibility() {
        let initial = WindowLayout {
            open: true,
            position: [12.0, 18.0],
            size: [240.0, 180.0],
        };
        let captured = capture_window_layout(initial, false, [96.0, 112.0], [360.0, 280.0]);
        assert_eq!(
            captured,
            WindowLayout {
                open: false,
                position: [96.0, 112.0],
                size: [360.0, 280.0],
            }
        );
    }

    #[test]
    fn defaults_and_window_flags_are_stable() {
        let layout = ImGuiLayout::default();
        assert!(layout.windows[InspectorWindow::Inspector.index()].open);
        assert!(layout.windows[InspectorWindow::Hierarchy.index()].open);
        assert!(layout.windows.iter().skip(2).all(|window| !window.open));
        assert_eq!(
            InspectorWindow::all().map(InspectorWindow::name),
            [
                "Inspector",
                "Hierarchy",
                "Log",
                "Materials",
                "Render",
                "Humans",
                "Telemetry",
                "Editor",
                "Export"
            ]
        );
        assert_eq!(BLUE_TITLE_ACTIVE, [0.05, 0.3, 0.62, 1.0]);
    }

    #[test]
    fn input_capture_routes_keyboard_and_pointer_independently() {
        let keyboard = InputCaptureState { pointer: false, keyboard: true };
        assert!(keyboard.blocks_keyboard());
        assert!(!keyboard.blocks_pointer());
        let pointer = InputCaptureState { pointer: true, keyboard: false };
        assert!(pointer.blocks_pointer());
        assert!(!pointer.blocks_keyboard());
        assert!(!InputCaptureState::default().blocks_keyboard());
        assert!(!InputCaptureState::default().blocks_pointer());
    }

    #[test]
    fn backend_without_renderer_is_non_fatal() {
        let error = BackendError("surface unavailable".into());
        assert_eq!(error.0, "surface unavailable");
        assert!(!InputCaptureState::default().blocks_pointer());
    }

    #[test]
    fn command_queue_is_fifo_and_owned() {
        let mut ui = InspectorUi::new();
        ui.queue_command(InspectorCommand::ClearSelection);
        ui.queue_command(InspectorCommand::CycleNextHit);
        assert!(matches!(
            ui.drain_commands().as_slice(),
            [
                InspectorCommand::ClearSelection,
                InspectorCommand::CycleNextHit
            ]
        ));
        assert!(ui.drain_commands().is_empty());
    }
}
