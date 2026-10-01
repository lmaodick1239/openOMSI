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

## Task 4 Review Fix Report

- Populated the ImGui snapshot from owned application/render/selection state for material, render, human, telemetry, editor, export, hierarchy, and log panels; no simulation references cross the UI frame boundary.
- Replaced silent non-selection command no-ops with `dispatch_inspector_command`, preserving selection/hit actions and returning/logging explicit `CommandError::NotSupported` results for missing subsystem mutation boundaries.
- Added an always-available Windows menu that reopens every named inspector window while Inspector and Hierarchy retain their visible defaults.
- Added human skeleton toggle command emission and diagnostic messaging in the ImGui panel using the owned human view model.
- Added `abort_frame` and invoke it when surface acquisition yields no render view, ensuring ImGui frame state and input capture are cleared safely.
- Added focused coverage for frame-abort behavior and retained the existing layout/window regression tests.

Validation:

- `cargo fmt --all` passed.
- `cargo test -p omsi-app inspector --lib --tests --quiet -- --test-threads=1` passed: 77 library tests and all selected integration binaries passed.
- A parallel focused test run hit the existing ImGui global-context race; the serial run passed.
- `cargo check --workspace --quiet` and `git diff --check` run after this report update.

## Task 4 Fix-Round Cleanup (2026-10-01)

- Restored all accidental repository-wide formatting changes outside the migration allowlist to `HEAD`.
- Retained only `Cargo.lock`, `crates/omsi-app/Cargo.toml`, `crates/omsi-app/src/app.rs`, `crates/omsi-app/src/app_events.rs`, `crates/omsi-app/src/inspector/**`, and `crates/omsi-app/tests/*inspector*`.
- Kept the intentional `serial_test = "3"` dev-dependency and matching lockfile entries for serial inspector coverage.
- Removed the unrelated `crates/omsi-app/tests/penetration_stack_tests.rs` formatting-only change.
- Verified retained panel-source, material/human mutation-boundary, and frame-lifecycle test changes in the diff.

Validation commands for this cleanup:

- `cargo test -p omsi-app inspector --lib --tests --quiet -- --test-threads=1`
- `cargo check --workspace --quiet`
- `cargo fmt --all -- --check` — failed because unrelated restored `HEAD` files (first reported: `crates/omsi-app/build.rs`) are not rustfmt-clean; no formatting sweep was reapplied.
- `git diff --check` — passed.

Observed validation results:

- `cargo test -p omsi-app inspector --lib --tests --quiet -- --test-threads=1` — passed: 79 library tests, plus selected integration binaries with 1 and 6 tests, all passed.
- `cargo check --workspace --quiet` — passed.
- The retained human snapshot method in `crates/omsi-app/src/humans.rs` was required by the retained `app.rs` panel-source change and is included as a directly coupled migration change.
