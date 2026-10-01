# Launcher Inspector Toggle Design

## Goal

Allow users to opt into the inspector from the launcher. When disabled, the game must not initialize the inspector runtime and must not expose an Inspector pause-menu action. When enabled, the pause menu exposes an `Inspector` button; inspector-local key handling remains available inside the inspector implementation.

## Behavior

- Add a persisted launcher duty choice named `inspector`, defaulting to `false` for new and legacy choice files.
- Add an Inspector toggle to the launcher settings UI. Changing it persists through the existing choice save path.
- Propagate the enabled state through launcher-core duty arguments as `--inspector`.
- Add a CLI `--inspector` flag and an `App` capability field. Inspector construction, event forwarding, frame rendering, layout persistence, selection highlighting, and related work are guarded by that capability.
- Add the `Inspector` pause-menu item only when enabled. Selecting it enters the existing inspector mode; there is no global Ctrl+I toggle.
- Remove the global inspector toggle/keybind path from event dispatch. Keep any key handling required by an already-open inspector within `imgui_inspector`.

## Compatibility and migration

Serde defaults make older `launcher-duty.json` files load with the feature disabled. Existing inspector APIs remain intact unless they are needed to guard initialization or remove the global toggle.

## Testing

Add focused launcher-core argument coverage for enabled/disabled values, choice default coverage, and app menu coverage proving the item is capability-gated. Run the relevant Rust tests and `cargo check` for the workspace.
