# Task 4 Report

## Implemented

- Connected the ImGui inspector windows to typed commands for selection hit cycling, view toggles, material sandbox actions, render/debug toggles, human playback, editor sandbox actions, export, and telemetry watches.
- Preserved stable selection targets by cloning the owned snapshot identity into each command; application-side validation remains authoritative.
- Removed direct egui rendering methods from the migrated material and human inspector modules.
- Documented desktop-only ImGui windows, persisted layout, controls, input capture limitations, Android behavior, and VR scope in `docs/USER_GUIDE.md`.
- Added input-routing coverage proving Ctrl+I remains available while ImGui captures keyboard input.

## Verification

Commands run from the worktree root:

- `cargo fmt --all` — passed.
- `cargo test -p omsi-app inspector --lib --tests --quiet` — passed: 76 library tests, plus all selected integration test binaries completed with 0 failures.
- `cargo check --workspace --quiet` — passed.
- `git diff --check` — passed.
- `! rg -n 'egui::|egui::Ui|feature = "egui"' crates/omsi-app/src/inspector/material_panel.rs crates/omsi-app/src/inspector/human_panel.rs` — passed; no direct egui rendering dependencies remain in migrated modules.

## Scope and concerns

Only inspector frontend integration, input routing coverage, related panel cleanup, and user documentation were changed. Android, simulation, renderer, XR, HUD, and normal game-control behavior were not refactored. Full interactive GPU/window verification was not run in this environment; desktop compilation and focused tests cover the integration boundary.
