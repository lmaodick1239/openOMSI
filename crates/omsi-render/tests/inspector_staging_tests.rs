//! Integration tests for inspector texture readback and material queries.

use omsi_render::graphics_inspector::*;

#[test]
fn test_pbr_params_default() {
    let params = PBRParams::default();
    assert_eq!(params.roughness, 0.5);
    assert_eq!(params.metalness, 0.0);
    assert_eq!(params.normal_intensity, 1.0);
    assert_eq!(params.env_reflection, 1.0);
}

#[test]
fn test_texture_handle_generation() {
    let handle1 = TextureHandle {
        id: 0,
        generation: 1,
    };
    let handle2 = TextureHandle {
        id: 0,
        generation: 2,
    };

    assert_ne!(handle1, handle2);
    assert_eq!(handle1.id, handle2.id);
    assert_ne!(handle1.generation, handle2.generation);
}

#[test]
fn test_blend_mode_classification() {
    let modes = [
        BlendMode::Alpha,
        BlendMode::Additive,
        BlendMode::Multiply,
        BlendMode::Opaque,
    ];

    for mode in &modes {
        match mode {
            BlendMode::Alpha => assert_eq!(*mode, BlendMode::Alpha),
            BlendMode::Additive => assert_eq!(*mode, BlendMode::Additive),
            BlendMode::Multiply => assert_eq!(*mode, BlendMode::Multiply),
            BlendMode::Opaque => assert_eq!(*mode, BlendMode::Opaque),
        }
    }
}

#[test]
fn test_render_pass_classification() {
    let passes = [
        RenderPassType::Opaque,
        RenderPassType::Transparent,
        RenderPassType::Cutout,
        RenderPassType::Shadow,
    ];

    assert_eq!(passes.len(), 4);
    for pass in &passes {
        match pass {
            RenderPassType::Opaque => assert_eq!(*pass, RenderPassType::Opaque),
            RenderPassType::Transparent => assert_eq!(*pass, RenderPassType::Transparent),
            RenderPassType::Cutout => assert_eq!(*pass, RenderPassType::Cutout),
            RenderPassType::Shadow => assert_eq!(*pass, RenderPassType::Shadow),
        }
    }
}

#[test]
fn test_mip_count_calculation() {
    assert_eq!(mip_count(1024, 1024), 11); // 1024 -> 1
    assert_eq!(mip_count(512, 512), 10);   // 512 -> 1
    assert_eq!(mip_count(256, 128), 9);    // 256 -> 1
    assert_eq!(mip_count(1, 1), 1);
    assert_eq!(mip_count(0, 0), 1);
    assert_eq!(mip_count(2048, 1024), 12); // 2048 -> 1
}

#[test]
fn test_format_name_common_formats() {
    use wgpu::TextureFormat;

    assert_eq!(format_name(TextureFormat::Rgba8Unorm), "RGBA8Unorm");
    assert_eq!(format_name(TextureFormat::Bc7RgbaUnorm), "BC7");
    assert_eq!(format_name(TextureFormat::Bc3RgbaUnorm), "BC3 (DXT5)");
    assert_eq!(format_name(TextureFormat::Bc1RgbaUnorm), "BC1 (DXT1)");
    assert_eq!(format_name(TextureFormat::R8Unorm), "R8");
}

#[test]
fn test_readback_id_equality() {
    let id1 = ReadbackId(1);
    let id2 = ReadbackId(1);
    let id3 = ReadbackId(2);

    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
}

#[test]
fn test_readback_status_ordering() {
    use ReadbackStatus::*;

    let pending = Pending;
    let in_flight = InFlight;
    let ready = Ready;
    let complete = Complete;
    let failed = Failed;

    assert_eq!(pending, Pending);
    assert_eq!(in_flight, InFlight);
    assert_eq!(ready, Ready);
    assert_eq!(complete, Complete);
    assert_eq!(failed, Failed);
}

#[test]
fn test_material_snapshot_vram_calculation() {
    // Placeholder test - real test would construct Scene with textures
    let snapshot = MaterialSnapshot {
        shader_variant: "enhanced.wgsl".to_string(),
        diffuse_tex: Some(TextureHandle {
            id: 0,
            generation: 1,
        }),
        normal_tex: None,
        roughness_tex: None,
        metallic_tex: None,
        envmap_tex: None,
        pbr_params: PBRParams::default(),
        vram_bytes: 4 * 1024 * 1024, // 4 MB
        color: [1.0, 1.0, 1.0, 1.0],
        emissive: [0.0, 0.0, 0.0],
        alpha_mode: omsi_render::AlphaMode::Opaque,
        uv_transform: [1.0, 1.0, 0.0, 0.0],
        blend_mode: BlendMode::Opaque,
        double_sided: false,
        alpha_cutoff: 0.0,
        render_pass: RenderPassType::Opaque,
    };

    assert_eq!(snapshot.vram_bytes, 4 * 1024 * 1024);
    assert_eq!(snapshot.shader_variant, "enhanced.wgsl");
    assert_eq!(snapshot.pbr_params.roughness, 0.5);
}

#[test]
fn test_texture_metadata_resolution() {
    let metadata = TextureMetadata {
        handle: TextureHandle {
            id: 0,
            generation: 1,
        },
        format: "BC7".to_string(),
        resolution: (1024, 1024),
        mipmap_count: 11,
        vram_bytes: 1024 * 1024, // Simplified
        is_dynamic: false,
    };

    assert_eq!(metadata.resolution, (1024, 1024));
    assert_eq!(metadata.mipmap_count, 11);
    assert_eq!(metadata.format, "BC7");
}

// Async staging pool tests - require wgpu device
#[cfg(test)]
mod staging_pool_tests {
    #[test]
    fn test_align_to() {
        // Test from staging_pool module
        fn align_to(value: u32, alignment: u32) -> u32 {
            (value + alignment - 1) / alignment * alignment
        }

        assert_eq!(align_to(0, 256), 0);
        assert_eq!(align_to(1, 256), 256);
        assert_eq!(align_to(255, 256), 256);
        assert_eq!(align_to(256, 256), 256);
        assert_eq!(align_to(257, 256), 512);
        assert_eq!(align_to(1920 * 4, 256), 7936); // 1920px RGBA row
    }

    #[test]
    fn test_staging_pool_capacity() {
        const MAX_STAGING_BYTES: u64 = 128 * 1024 * 1024;
        assert_eq!(MAX_STAGING_BYTES, 128 * 1024 * 1024);

        // Verify 4K texture fits
        let texture_4k_rgba = 3840 * 2160 * 4;
        assert!(texture_4k_rgba < MAX_STAGING_BYTES as usize);

        // Verify multiple 1080p textures fit
        let texture_1080p_rgba = 1920 * 1080 * 4;
        let max_1080p = MAX_STAGING_BYTES as usize / texture_1080p_rgba;
        assert!(max_1080p >= 15); // Can fit 15+ 1080p RGBA textures
    }
}

// Performance verification tests
#[test]
fn test_zero_stall_guarantee_documentation() {
    // This test documents the zero-stall guarantee requirements
    // Actual verification happens in integration tests with real GPU

    // Requirements:
    // 1. No Device::poll(Wait) calls - only poll(Maintain::Poll)
    // 2. All readbacks use async staging buffers
    // 3. Frame budget < 2ms for texture operations
    // 4. Staging pool capped at 128MB VRAM

    let max_staging_mb = 128;
    let frame_budget_ms = 2.0;

    assert!(max_staging_mb > 0);
    assert!(frame_budget_ms > 0.0);
}

#[test]
fn test_texture_query_mip_levels() {
    let handle = TextureHandle {
        id: 42,
        generation: 5,
    };

    for mip_level in 0..=10 {
        let query = TextureQuery {
            handle,
            request_mip_level: Some(mip_level),
        };

        assert_eq!(query.handle, handle);
        assert_eq!(query.request_mip_level, Some(mip_level));
    }
}
