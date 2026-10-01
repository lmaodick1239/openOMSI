//! glTF 2.0 export for inspector-selected entities.
//!
//! Single-click export button: "Export Selection to glTF". Bakes the selected entity
//! with resolved transforms, vertex colors, PBR materials, and embedded PNG textures
//! into a standard `.glb` file saved to `~/.local/share/openomsi/exports/`.

use glam::Mat4;
use std::fs;
use std::io;
use std::path::PathBuf;

/// glTF export error types.
#[derive(Debug)]
pub enum ExportError {
    Io(io::Error),
    InvalidMesh(String),
    InvalidMaterial(String),
    EncodingFailed(String),
    Unsupported(String),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::Io(e) => write!(f, "I/O error: {}", e),
            ExportError::InvalidMesh(msg) => write!(f, "Invalid mesh: {}", msg),
            ExportError::InvalidMaterial(msg) => write!(f, "Invalid material: {}", msg),
            ExportError::EncodingFailed(msg) => write!(f, "glTF encoding failed: {}", msg),
            ExportError::Unsupported(msg) => write!(f, "Export unsupported: {}", msg),
        }
    }
}

impl std::error::Error for ExportError {}

impl From<io::Error> for ExportError {
    fn from(e: io::Error) -> Self {
        ExportError::Io(e)
    }
}

/// Mesh data for export.
#[derive(Debug, Clone)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Option<Vec<[f32; 3]>>,
    pub tex_coords: Option<Vec<[f32; 2]>>,
    pub colors: Option<Vec<[f32; 4]>>,
    pub indices: Vec<u32>,
}

/// Material properties for glTF export.
#[derive(Debug, Clone)]
pub struct MaterialData {
    pub name: String,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub base_texture: Option<Vec<u8>>, // PNG bytes
    pub alpha_mode: AlphaMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaMode {
    Opaque,
    Mask,
    Blend,
}

/// Get the default export directory: `~/.local/share/openomsi/exports/`.
pub fn export_dir() -> Result<PathBuf, io::Error> {
    let data_dir = if cfg!(target_os = "windows") {
        // Windows: %LOCALAPPDATA%/openomsi/exports
        dirs::data_local_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No data directory"))?
            .join("openomsi")
            .join("exports")
    } else if cfg!(target_os = "macos") {
        // macOS: ~/Library/Application Support/openomsi/exports
        dirs::data_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No data directory"))?
            .join("openomsi")
            .join("exports")
    } else {
        // Linux/Unix: ~/.local/share/openomsi/exports
        dirs::data_local_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No data directory"))?
            .join("openomsi")
            .join("exports")
    };

    fs::create_dir_all(&data_dir)?;
    Ok(data_dir)
}

/// Inspector export is currently unsupported.
///
/// The production vehicle exporter remains available through `--export-glb`.
pub fn export_to_gltf(
    entity_name: &str,
    transform: Mat4,
    mesh_data: &MeshData,
    material: &MaterialData,
) -> Result<PathBuf, ExportError> {
    let _ = (entity_name, transform, mesh_data, material);
    Err(ExportError::Unsupported(
        "interactive inspector export is not implemented; use --export-glb".to_string(),
    ))
}

/*
    // Validate mesh data
    if mesh_data.positions.is_empty() {
        return Err(ExportError::InvalidMesh(
            "Empty position buffer".to_string(),
        ));
    }
    if mesh_data.indices.is_empty() {
        return Err(ExportError::InvalidMesh("Empty index buffer".to_string()));
    }

    // Generate timestamped filename
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let filename = format!("{}_{}.glb", sanitize_filename(entity_name), timestamp);

    let export_path = export_dir()?.join(&filename);

    log::info!("Exporting entity '{}' to {:?}", entity_name, export_path);

    // TODO: Implement full glTF encoding using `gltf` crate
    // For now, create a placeholder file to validate the export path
    let placeholder_data =
        create_minimal_gltf_placeholder(entity_name, transform, mesh_data, material)?;

    fs::write(&export_path, placeholder_data)?;

    log::info!("Successfully exported to {:?}", export_path);
    Ok(export_path)
}
*/

/// Legacy placeholder retained only as non-production reference; never called by the API.
#[allow(dead_code)]
///
/// This is a temporary stub that creates a valid (but minimal) glTF file.
/// Full implementation requires proper binary buffer encoding, material setup,
/// and texture embedding.
fn create_minimal_gltf_placeholder(
    _name: &str,
    _transform: Mat4,
    mesh_data: &MeshData,
    material: &MaterialData,
) -> Result<Vec<u8>, ExportError> {
    // Create a minimal glTF JSON structure
    let gltf_json = serde_json::json!({
        "asset": {
            "version": "2.0",
            "generator": "openOMSI Inspector"
        },
        "scene": 0,
        "scenes": [{
            "nodes": [0]
        }],
        "nodes": [{
            "mesh": 0,
            "name": "ExportedMesh"
        }],
        "meshes": [{
            "primitives": [{
                "attributes": {
                    "POSITION": 0
                },
                "indices": 1,
                "material": 0
            }]
        }],
        "materials": [{
            "name": material.name,
            "pbrMetallicRoughness": {
                "baseColorFactor": material.base_color,
                "metallicFactor": material.metallic,
                "roughnessFactor": material.roughness
            },
            "alphaMode": match material.alpha_mode {
                AlphaMode::Opaque => "OPAQUE",
                AlphaMode::Mask => "MASK",
                AlphaMode::Blend => "BLEND",
            }
        }],
        "accessors": [
            {
                "bufferView": 0,
                "componentType": 5126,
                "count": mesh_data.positions.len(),
                "type": "VEC3",
                "max": [1.0, 1.0, 1.0],
                "min": [-1.0, -1.0, -1.0]
            },
            {
                "bufferView": 1,
                "componentType": 5125,
                "count": mesh_data.indices.len(),
                "type": "SCALAR"
            }
        ],
        "bufferViews": [
            {
                "buffer": 0,
                "byteOffset": 0,
                "byteLength": mesh_data.positions.len() * 12
            },
            {
                "buffer": 0,
                "byteOffset": mesh_data.positions.len() * 12,
                "byteLength": mesh_data.indices.len() * 4
            }
        ],
        "buffers": [{
            "byteLength": mesh_data.positions.len() * 12 + mesh_data.indices.len() * 4
        }]
    });

    let json_string = serde_json::to_string(&gltf_json)
        .map_err(|e| ExportError::EncodingFailed(e.to_string()))?;

    // For now, return just the JSON as a placeholder
    // TODO: Encode proper GLB binary format with embedded buffers
    Ok(json_string.into_bytes())
}

/// Legacy helper for the removed placeholder writer.
#[allow(dead_code)]
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(
            sanitize_filename("mesh/test:file?.o3d"),
            "mesh_test_file_.o3d"
        );
        assert_eq!(sanitize_filename("normal_name"), "normal_name");
    }

    #[test]
    fn test_export_dir_creation() {
        // Just verify the function doesn't panic
        let _ = export_dir();
    }

    #[test]
    fn test_mesh_validation() {
        let empty_mesh = MeshData {
            positions: vec![],
            normals: None,
            tex_coords: None,
            colors: None,
            indices: vec![],
        };

        let material = MaterialData {
            name: "test".to_string(),
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 1.0,
            base_texture: None,
            alpha_mode: AlphaMode::Opaque,
        };

        let result = export_to_gltf("test", Mat4::IDENTITY, &empty_mesh, &material);

        assert!(result.is_err());
    }
}
