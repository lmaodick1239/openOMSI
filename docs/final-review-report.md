# Final Review Report

## Fix wave

- Inspector dispatch preserves selection, hit cycling, hit jumping, and view toggles; material, render, human, editor, export, and telemetry mutations return explicit `NotSupported` diagnostics.
- Removed telemetry watch mutation controls and all UI controls that would queue unavailable mutations. Render counters remain visible even when debug snapshots are unavailable. Editor and export panels show explicit unavailable state.
- Telemetry view timestamps now use Unix epoch milliseconds without changing the serialized `u64` contract.
- User guide distinguishes available selection/layout controls from unavailable mutation, watch, and interactive export features. The CLI `--export-glb` path remains documented as the production exporter.
- Inspector glTF API/tests now treat the placeholder writer as unsupported/non-production instead of reporting successful export.

## Validation

Commands were run serially on commit base `a744329`:

- `cargo test -p omsi-app inspector -- --test-threads=1` — PASS; 81 unit tests and focused inspector integration tests passed.
- `cargo check --workspace` — PASS.
- `git diff --check` — PASS.

Known residual risk: the full `ApplicationHandler` event-loop integration path was not exercised in this headless validation environment.
