# openOMSI FPS and VRAM Code Review

**Review date:** 2026-10-01  
**Scope:** Static source review of the renderer, scene streaming, texture residency, mirrors, post-processing, and frame loop.  
**Evidence level:** Source-based. No GPU capture, target hardware profile, or representative OMSI asset installation was available, so impact estimates are hypotheses to validate with measurements.

## Executive summary

The renderer already contains several important optimizations: instancing and batching, frustum/distance/LOD culling, BC texture support, texture mip eviction, tile unloading, shadow caching, mirror throttling, render scaling, and frame-stage profiling. The highest-value next steps are therefore measurement and removal of avoidable allocations rather than broad architectural rewrites.

> **Important observation:** The game appears to show relatively low GPU usage rather than saturating the GPU. This makes a purely GPU-focused optimization strategy inappropriate as the primary path. First determine whether the frame is CPU-bound, synchronization/pacing-bound, submission-bound, or limited by insufficient parallelism. Lowering resolution or shader cost may reduce GPU time without increasing FPS if the CPU remains the bottleneck.

### Priorities

| Priority | Area | Expected benefit | Risk |
|---|---|---:|---:|
| P0 | Avoid redundant Enhanced+MSAA color allocation | Lower peak VRAM; fewer transient allocations | Low |
| P0 | Add renderer-owned target accounting and per-frame telemetry | Identifies the actual bottleneck and OOM source | Low |
| P1 | Make texture residency budgets stricter and hysteretic | Often the largest VRAM reduction on dense maps | Medium |
| P1 | Reduce mirror and post-processing work on slow/GPU-bound frames | Better GPU frame time | Low–medium |
| P2 | Tune visibility, shadow, and transparency work using captures | Better CPU/GPU FPS in dense scenes | Medium |
| P3 | Investigate occlusion culling or GPU-driven submission | Potentially substantial FPS gain | High |

## Findings

### 1. Enhanced + MSAA allocates an unused RGBA8 MSAA color target

**Evidence:** [`crates/omsi-render/src/lib.rs`](crates/omsi-render/src/lib.rs), `msaa_targets` around line 5406, allocates both `msaa colour` and `msaa depth`. The Enhanced path separately creates HDR MSAA color targets in `hdr_targets`; the main pass selects the HDR target when Enhanced is enabled. The plain RGBA8 color target is therefore unnecessary in that configuration.

**Impact:** A 1920×1080 4× RGBA8 target is approximately 31.6 MiB of logical storage; at 4K it is approximately 126.6 MiB. The exact driver allocation varies.

**Recommendation:** Split the target cache into depth-only and plain-color+depth paths. When `options.enhanced && msaa > 1`, allocate only the depth attachment in `msaa_targets` and use `HdrTargets::msaa_view` for color. Add a branch/regression test that checks the selected color attachment for vanilla and Enhanced modes.

**Expected result:** Immediate VRAM reduction with no intended image-quality change.

### 2. Render-target caches can retain several full-resolution target sets

**Evidence:** [`crates/omsi-render/src/lib.rs`](crates/omsi-render/src/lib.rs), `evict_targets` around line 5380 retains up to `KEEP = 10` recently used sizes across `scale_targets`, `msaa_targets`, and `hdr_targets`. Window resizing and render-scale changes can create multiple large HDR/MSAA/post-processing sets before stale entries are removed.

**Recommendation:** Track estimated bytes per cached target set and evict by a byte budget, not only by count and age. Keep one active size plus a small number of recent sizes; use a lower cap for Enhanced+MSAA. Record cache bytes in the renderer diagnostics.

**Risk:** Recreating targets during repeated resize operations can cause allocation spikes. Keep a short debounce or retain the active size until resize settles.

### 3. The 1024² HDR sky cubemap is a fixed VRAM and update cost

**Evidence:** [`crates/omsi-render/src/lib.rs`](crates/omsi-render/src/lib.rs), `SKY_CUBE_SIZE` around line 143 and `cube_tex` around line 3156. The cubemap is six 1024² `Rgba16Float` faces: about 48 MiB before implementation-specific alignment. It is refreshed one face every four window frames.

**Recommendation:** Make sky-cube size configurable by quality tier (for example 512/768/1024), and expose an Enhanced “sky quality” or “reflection quality” option. Prefer 512 on low-VRAM devices and when the cubemap occupies a small on-screen footprint. Measure cloud shimmer and GPU time before selecting a default.

**Expected result:** Roughly 36 MiB logical savings when reducing 1024 to 512, plus lower sky-face update cost.

### 4. Mirror rendering is throttled but still multiplies scene work

**Evidence:** [`crates/omsi-app/src/app_events.rs`](crates/omsi-app/src/app_events.rs), mirror scheduling around lines 1830–1870, can render up to two mirrors per frame. [`crates/omsi-app/src/camera_util.rs`](crates/omsi-app/src/camera_util.rs), `render_mirrors` around line 322, performs a full renderer invocation for each selected mirror. [`crates/omsi-app/src/scene.rs`](crates/omsi-app/src/scene.rs), `mirror_texture`, creates textures up to the configured mirror size, clamped to 2048.

**Recommendation:** Add adaptive mirror quality: reduce mirror resolution and/or mirror redraw rate when GPU frame time exceeds the frame budget. Keep a minimum refresh rate only for mirrors visible in the current view. Consider a “mirror resolution scale” independent of the window render scale.

**Expected result:** High benefit in buses with several visible mirrors; no effect when outside the bus or when mirrors are disabled.

### 5. Texture compression is conditional, so rejected and small textures remain RGBA8

**Evidence:** [`crates/omsi-texture/src/gpu.rs`](crates/omsi-texture/src/gpu.rs), `MIN_COMPRESS_TEXELS` around line 121 and PSNR rejection in `prepare_image_with` around line 180. Textures smaller than 64×64 texels, non-block-compatible images, unsupported BC devices, and images below the quality thresholds remain uncompressed. [`crates/omsi-app/src/scene.rs`](crates/omsi-app/src/scene.rs), texture upgrade handling around lines 7400–7550, can restore higher-resolution data when a texture becomes important.

**Recommendation:**

- Keep BC formats for opaque and alpha textures whenever quality permits.
- Add telemetry for compressed/rejected/RGBA texture counts and bytes by source category (vehicle, tile, UI, dynamic replacement).
- Apply the existing texture budget with hysteresis: drop finest mips only after a sustained over-budget period and restore them only after a sustained under-budget period.
- Protect cockpit displays, UI, alpha-critical masks, and nearby materials from aggressive mip dropping.
- Avoid duplicate texture allocations for semantically identical dynamic texture replacements.

**Expected result:** Usually the largest VRAM reduction on content-heavy maps, but visual quality needs screenshot comparison.

### 6. Texture and mesh residency needs a total budget, not only per-texture control

**Evidence:** [`crates/omsi-app/src/scene.rs`](crates/omsi-app/src/scene.rs), `gpu_summary` around lines 7200–7260 reports texture, mesh, draw-data, vehicle, and tile categories. The renderer exposes texture mip controls (`texture_levels` and `drop_top_levels` around lines 4768–4780 in [`crates/omsi-render/src/lib.rs`](crates/omsi-render/src/lib.rs)). Tile unloading exists around line 6861 in `scene.rs`, but renderer-owned targets and all residency categories should be considered together.

**Recommendation:** Build an explicit VRAM budget controller with categories and priorities:

1. active frame targets and depth attachments;
2. cockpit/player assets;
3. visible nearby tile assets;
4. visible traffic and passenger assets;
5. distant tile assets;
6. optional reflection and glow resources.

Evict or reduce lower-priority resources first, and log the reason for every downgrade. Do not unload data still required for collision, scripts, AI, or timetable simulation.

### 7. Dense-scene CPU cost is likely dominated by visibility, sorting, submission preparation, or waiting

Because GPU usage appears lower than expected, the first optimization target should be the CPU/render orchestration path, not fragment-shader complexity. A frame can have low GPU usage while still being slow because the CPU is preparing commands, waiting on presentation or resource work, or submitting too little parallel work. This observation should be confirmed with GPU busy time from a graphics profiler; desktop “GPU usage” counters can be misleading when only one engine or queue is reported.

**Evidence:** [`crates/omsi-render/src/lib.rs`](crates/omsi-render/src/lib.rs), the render preparation and batching paths around `prepare`, `batch_items`, `encode_batches_filtered`, and `record_bundles` (roughly lines 5600–5900 and 8800–9050) perform per-view visibility tests, LOD selection, sorting, and batch construction. Existing documentation notes that the current visibility loop is sequential because prior worker hand-off overhead outweighed the culling work.

**Recommendations:**

- Use `OMSI_PROFILE` and `OMSI_PROFILE_GPU` to classify CPU-bound versus GPU-bound scenes before changing culling.
- Cache stable visibility results for static scenery and invalidate them only when the camera cell, view direction threshold, tile residency, or settings change.
- Measure shadow-cascade candidate scans separately; do not optimize main-view culling while shadow preparation is dominant.
- Keep transparent ordering semantics intact; do not globally sort blended objects by material.
- Investigate parallel culling only after measuring contention and allocation overhead on current hardware.

**Risk:** Incorrect caching can cause one-frame stale visibility or missing scripted objects. Start with static, non-animated scenery only.

### 8. Shadows, SSAO, Enhanced shading, and resolution are multiplicative quality costs

**Evidence:** [`crates/omsi-app/src/settings.rs`](crates/omsi-app/src/settings.rs), graphics settings around lines 10–110, expose MSAA, SSAO, shadow size, Enhanced graphics, post AA, render scale, mirror size, texture memory, and object culling controls. The renderer uses `Depth32Float` and configurable shadow sizes in [`crates/omsi-render/src/lib.rs`](crates/omsi-render/src/lib.rs) around lines 1174–1225.

**Recommendations for a practical low-FPS preset:**

- Set render scale to 0.75–0.85 before reducing texture quality.
- Use 2× MSAA or disable MSAA when GPU-bound; compare with post FXAA in Enhanced mode.
- Disable SSAO and reduce shadow size before reducing object distance.
- Reduce mirror size and mirror rate independently.
- Increase `min_obj_size` modestly and reduce `max_obj_dist` only after confirming the scene is object-count bound.
- Keep texture compression enabled and set a finite texture-memory budget.

These should be presented as explicit user-facing presets rather than hidden automatic changes, except for adaptive render scale already supported by the settings.

### 9. Mirror selection contains a small avoidable CPU inefficiency

**Evidence:** [`crates/omsi-app/src/camera_util.rs`](crates/omsi-app/src/camera_util.rs), `render_mirrors` around lines 372–390, constructs `seen` and then calls `seen.contains(&i)` inside a loop over all cameras.

**Recommendation:** Iterate directly over `seen` (or use a boolean mask) rather than scanning the vector repeatedly. This is a small optimization and will not materially improve FPS unless vehicles have unusually many reflection cameras, so it should be bundled with mirror work rather than prioritized alone.

## Measurement plan

### Required counters

Log once per second or on demand. Since GPU utilization is not full, distinguish useful work from waiting:

- CPU frame time and measured GPU busy time (not only an OS-level utilization percentage);
- CPU time in simulation, streaming, visibility, sorting/batching, command recording, queue submission, presentation/VSync, and explicit GPU waits;
- GPU timestamp duration for shadow, prepass, main picture, mirrors, and post-processing passes, plus queue idle gaps;
- visible instance count and draw-call/batch count;
- shadow candidate/draw counts per cascade;
- mirror renders and mirror texture size;
- loaded tiles, mesh/material/texture counts;
- GPU bytes by scene textures, meshes, draw data, mirror targets, HDR/post targets, shadows, SSAO, and sky/probe resources;
- compressed versus RGBA texture bytes and dropped mip levels.

### Reproducible scenarios

Use the existing recipe in [`docs/INSPECTOR_PERFORMANCE.md`](docs/INSPECTOR_PERFORMANCE.md) as a starting point, with inspector disabled for this review. Capture at a fixed camera/time in:

1. a dense Grundorf scene with traffic and passengers;
2. a mirror-heavy cockpit view;
3. an outside view with many visible tiles;
4. Enhanced+MSAA at 1080p and 4K;
5. a resize/render-scale sweep to exercise target-cache eviction.

For each scenario, record median and 95th-percentile frame time over at least 60 seconds. Validate every memory change with a fixed-camera screenshot comparison and a driver-level VRAM measurement where available.

## Suggested implementation order

1. Add category-level GPU target telemetry and cache-byte accounting.
2. Instrument CPU waiting/pacing and command-recording/submission time; verify why the GPU is underutilized.
3. Remove the redundant Enhanced+MSAA RGBA8 color target.
4. Add target-cache byte-budget eviction.
5. Profile visibility, batching, streaming, and submission parallelism; optimize the measured CPU bottleneck.
6. Tune adaptive mirror resolution/rate only if mirror renders are a meaningful cost.
7. Validate and tighten texture-budget hysteresis and dynamic-texture deduplication.
8. Consider higher-risk occlusion or GPU-driven submission only if captures show geometry submission remains dominant.

## Overall conclusion

The most defensible immediate wins are the redundant MSAA allocation, target-cache accounting/eviction, adaptive mirror quality, and disciplined texture residency. Reducing render scale, MSAA, SSAO, shadow size, and mirror quality will generally increase FPS quickly, while texture compression/mip budgets reduce VRAM. Aggressive LOD, distance, occlusion, or renderer rewrites should wait for GPU/CPU captures because this codebase already has substantial culling and batching logic, and unmeasured changes could trade visual correctness for little benefit.
