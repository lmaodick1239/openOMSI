# Rendering Optimization Feasibility Assessment

**Scope:** Grundorf with the SL202, reported VRAM use around 1.8 GB, crashes at startup, and low FPS in dense scenarios.

**Evidence level:** Static source review. No SL202/Grundorf installation, GPU capture, crash log, or target hardware was available, so this document does not claim a reproduced crash or measured speedup.

## Executive Assessment

Improvement is feasible. The code already has useful foundations: tile streaming, frustum/fog/distance/LOD culling, BC texture compression, render-scale control, shadow caching, target eviction, and per-scene GPU byte accounting. The current design still has several credible sources of excess memory and frame time:

1. **High-confidence, low-risk:** eliminate an unused RGBA8 multisample color target on Enhanced + MSAA frames. The Enhanced path renders into HDR MSAA, but `msaa_targets` also allocates a plain-format MSAA color attachment that is not selected by the main pass.
2. **High-confidence, low-risk:** make GPU memory visible in the user log, split into scene textures, meshes, buffers, and transient render targets. Existing `Scene::gpu_bytes` does not include renderer-owned targets such as shadow/HDR/AO caches.
3. **Medium-risk:** add a bounded VRAM budget and reduce texture mip residency for distant objects before changing geometry or visibility behavior.
4. **Medium/high-risk:** reduce mesh residency and improve tile/object eviction. This can help dense maps substantially, but requires careful handling of shared object types, scripted objects, and tile-boundary visibility.
5. **High-risk:** change LOD distances, aggressive occlusion culling, or pass ordering. These may improve FPS, but they can alter visibility, shadows, alpha ordering, or deterministic behavior and should follow profiling rather than precede it.

The reported 1.8 GB is plausible as an aggregate of scene textures, mesh/index buffers, duplicated content variants, shadows, HDR/MSAA/post targets, mirrors, and driver allocations. It is not possible to assign that total to one subsystem from the current logs.

## Why VRAM Can Become Large

### Fixed renderer allocations

The default shadow size is 2048 per cascade. The renderer creates a 2-cascade near/close depth texture plus a far depth texture at `Depth32Float` in [`crates/omsi-render/src/lib.rs`](../crates/omsi-render/src/lib.rs). At 2048 this is approximately 48 MiB of logical depth storage; at 4096 it is approximately 192 MiB. This cost exists independently of map complexity.

Enhanced rendering adds full-resolution HDR and post-processing targets in [`crates/omsi-render/src/lib.rs`](../crates/omsi-render/src/lib.rs):

- `Rgba16Float` HDR color;
- optional HDR MSAA color;
- protected-screen masks;
- tone-mapped LDR color;
- glow downsample and upsample chains;
- AO depth and half-resolution AO buffers when enabled.

At 1920x1080, one full-resolution `Rgba16Float` surface is about 15.8 MiB before driver alignment. A 4x surface is about 63.3 MiB. These figures are logical estimates, not driver allocations.

### Redundant Enhanced + MSAA allocation

[`Renderer::render_inner`](../crates/omsi-render/src/lib.rs) requests `msaa_targets` whenever depth is not shared. [`Renderer::msaa_targets`](../crates/omsi-render/src/lib.rs) allocates both an RGBA8 MSAA color target and a depth target. In Enhanced + MSAA mode, the main pass chooses the HDR MSAA view from `hdr_targets`; the RGBA8 MSAA color view is therefore not used for the main color output. At 1920x1080 with 4x MSAA, the redundant RGBA8 color allocation is approximately 31.6 MiB logical storage. At 4K it is approximately 126.6 MiB.

This is the best first memory fix because it does not change geometry, shading, visibility, or image output.

### Content residency

Tile upload creates GPU meshes, materials, and textures for every loaded tile. The scene retains shared object types while referenced, and releases tile-owned resources through [`World::unload_tile`](../crates/omsi-app/src/scene.rs). The important unknown is the number and size of unique textures and meshes in the loaded Grundorf radius, especially with the SL202's body, interior, night maps, transmaps, bump/PBR maps, and reflection assets.

Texture compression is supported in [`crates/omsi-texture/src/gpu.rs`](../crates/omsi-texture/src/gpu.rs), but it is conditional: devices without BC support use RGBA, small textures remain RGBA, and quality rejection can retain an uncompressed image. A map with many unique or rejected textures can therefore dominate memory.

The loader also supports dynamic texture replacements and PBR maps. These can create additional texture slots when variants are active. This needs accounting before attempting deduplication.

## FPS in Dense Scenarios

The likely cost categories are different from VRAM:

- CPU visibility and batch construction scale with instance count. The main culling path is sequential by design because an earlier parallel implementation was slower under simulation contention.
- Draw-list construction and transparent grouping can become expensive with many object/material combinations.
- Enhanced fragment work is costly in overdraw-heavy scenes. The depth prepass reduces hidden enhanced shading, but MSAA requires a separate compatible depth prepass.
- Shadow candidate scans and shadow rendering add geometry work even when the main view is fill-rate limited.
- Large numbers of unique materials reduce batching and increase bind-group/state work.
- Higher render resolution, MSAA, SSAO, HDR glow, and mirrors multiply pixel work independently of map geometry.

Changing LOD or distance limits may raise FPS quickly, but it changes visible content. It should not be the first “no regression” optimization.

## Recommended Work Packages

### P0: Diagnose and remove waste

**Feasibility: very high.**

1. Add a startup/frame diagnostic containing:
   - loaded tile count;
   - instance, mesh, material, and texture counts;
   - compressed versus RGBA texture bytes;
   - mesh/index buffer bytes;
   - render-target configuration and estimated bytes;
   - shadow size, MSAA, SSAO, Enhanced state, render scale, and mirror count.
2. Split `msaa_targets` so Enhanced + MSAA creates only the depth target when HDR MSAA is active.
3. Add a device-lost/OOM diagnostic that records the last completed stage and allocation category before the existing orderly exit path.
4. Keep the existing output path and add a regression test for target-selection branches.

**Expected result:** lower peak VRAM in the affected configuration and enough evidence to distinguish scene residency from transient target pressure. No visual change intended.

### P1: Budget texture residency

**Feasibility: high, but requires policy decisions.**

Use existing `texture_bytes`, `texture_levels`, and `drop_top_levels` APIs to enforce a configurable budget. Prefer dropping only the finest mip levels of textures used by distant objects, and restore them when the object becomes important. Maintain full resolution for UI, cockpit instruments, alpha-critical masks, bump maps, and near-camera assets.

Verification must compare screenshots at fixed camera/time, texture identity/format logs, shimmer/aliasing metrics, and frame-time distributions. The budget must have hysteresis to avoid upload/drop thrashing.

### P2: Bound tile and mesh residency

**Feasibility: medium.**

The current tile release path is real, but the policy deciding how many tiles remain loaded and how shared object types are trimmed needs measurement. Add residency telemetry first. Then consider:

- a hard upper bound on loaded tile radius based on camera and simulation requirements;
- keeping a smaller visual radius than the simulation/navigation radius where behavior permits;
- releasing unreferenced mesh buffers after object-type references disappear;
- avoiding duplicate material/texture variants when keys are semantically identical.

This package must preserve collision, AI, timetable, scripted-object, and camera-blocker behavior. It should be implemented after a tile-boundary replay test exists.

### P3: Reduce dense-scene frame time

**Feasibility: medium/high.**

Profile before changing behavior. The safest candidates are CPU-side:

- cache transformed bounds and object-level culling inputs;
- replace repeated linear searches in transparent grouping with an indexed map while preserving the current stable ordering;
- reuse scratch vectors and upload staging allocations;
- avoid rebuilding unchanged light grids and bind groups;
- continue using the existing depth prepass for Enhanced overdraw reduction.

GPU-side candidates such as conservative hierarchical-Z occlusion can be valuable, but need a debug mode that shows rejected geometry and a conservative fallback for uncertain depth.

## What Should Not Be Done First

- Do not globally lower LOD or max distance and call it a memory fix.
- Do not enable lossy texture compression for every asset without preserving alpha/bump quality gates.
- Do not reduce shadow resolution unconditionally; expose it as a budgeted fallback and verify shadow stability.
- Do not assume system-reported VRAM equals wgpu resource bytes. Driver heaps, staging allocations, caches, and residency policy can exceed logical texture sizes.
- Do not claim the SL202 is the root cause without its crash log and GPU configuration.

## Required Reproduction Data

A useful issue report should include:

- GPU model, dedicated/shared VRAM, driver version, backend, resolution, MSAA, Enhanced, SSAO, shadow size, render scale, and mirror size;
- whether the crash happens before the first frame, during tile streaming, or after entering the bus;
- the last 100-200 log lines with paths/usernames redacted;
- whether `OMSI_PROFILE=1` changes the behavior;
- a run with the SL202 removed or a different bus on the same spawn;
- a run with Enhanced/MSAA/SSAO disabled one at a time.

The existing renderer has controlled device-loss handling, but a device-loss message is not enough to identify whether the trigger was allocation failure, driver reset, validation failure, or another backend error.

## Priority and Feasibility Summary

| Work item | Feasibility | Regression risk | Expected value | Recommendation |
|---|---:|---:|---:|---|
| Remove unused Enhanced+MSAA color target | Very high | Very low | Medium VRAM reduction | Implement first |
| Add complete VRAM/resource diagnostics | Very high | None | Enables root-cause diagnosis | Implement first |
| Budget finest texture mips | High | Low/medium | High on texture-heavy maps | Prototype behind setting |
| Improve tile/mesh residency | Medium | Medium/high | High if scene residency dominates | Instrument first |
| Cache CPU culling/grouping work | High | Low | Medium FPS improvement | Profile and implement |
| Conservative occlusion culling | Medium | Medium | Potentially high FPS improvement | Later prototype |
| Global LOD/distance reductions | Easy | High visual change | High FPS improvement | User quality option only |

## Bottom Line

There is a credible path to materially better low-VRAM behavior and better dense-scene FPS without changing the rendered result. The first patch should be diagnostic plus the redundant-target removal. The next decision should be driven by measured per-category bytes on the actual Grundorf/SL202 setup: if textures dominate, implement mip residency; if meshes dominate, fix tile/object residency; if GPU time dominates, tune target resolution, prepass, shadows, and conservative visibility separately.
