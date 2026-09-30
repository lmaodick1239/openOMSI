//! glTF 2.0 export utilities for OMSI model data.
//!
//! Provides conversion from OMSI mesh and material data to glTF 2.0 format.
//! This module handles the low-level glTF encoding, while the higher-level
//! export API is in `omsi-app/src/inspector/export.rs`.

use serde::{Deserialize, Serialize};

/// glTF 2.0 asset metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GltfAsset {
    pub version: String,
    pub generator: String,
}

impl Default for GltfAsset {
    fn default() -> Self {
        Self {
            version: "2.0".to_string(),
            generator: "openOMSI Inspector".to_string(),
        }
    }
}

/// glTF primitive attributes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GltfPrimitiveAttributes {
    #[serde(rename = "POSITION")]
    pub position: usize,
    #[serde(rename = "NORMAL", skip_serializing_if = "Option::is_none")]
    pub normal: Option<usize>,
    #[serde(rename = "TEXCOORD_0", skip_serializing_if = "Option::is_none")]
    pub texcoord_0: Option<usize>,
    #[serde(rename = "COLOR_0", skip_serializing_if = "Option::is_none")]
    pub color_0: Option<usize>,
}

/// glTF material PBR properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GltfPbrMetallicRoughness {
    #[serde(rename = "baseColorFactor")]
    pub base_color_factor: [f32; 4],
    #[serde(rename = "metallicFactor")]
    pub metallic_factor: f32,
    #[serde(rename = "roughnessFactor")]
    pub roughness_factor: f32,
}

/// glTF material definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GltfMaterial {
    pub name: String,
    #[serde(rename = "pbrMetallicRoughness")]
    pub pbr_metallic_roughness: GltfPbrMetallicRoughness,
    #[serde(rename = "alphaMode")]
    pub alpha_mode: String,
}

/// glTF buffer view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GltfBufferView {
    pub buffer: usize,
    #[serde(rename = "byteOffset")]
    pub byte_offset: usize,
    #[serde(rename = "byteLength")]
    pub byte_length: usize,
}

/// glTF accessor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GltfAccessor {
    #[serde(rename = "bufferView")]
    pub buffer_view: usize,
    #[serde(rename = "componentType")]
    pub component_type: u32,
    pub count: usize,
    #[serde(rename = "type")]
    pub accessor_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<Vec<f32>>,
}

/// Calculate bounding box for position data.
pub fn calculate_bounds(positions: &[[f32; 3]]) -> (Vec<f32>, Vec<f32>) {
    if positions.is_empty() {
        return (vec![0.0, 0.0, 0.0], vec![0.0, 0.0, 0.0]);
    }

    let mut min = positions[0];
    let mut max = positions[0];

    for pos in positions.iter().skip(1) {
        for i in 0..3 {
            min[i] = min[i].min(pos[i]);
            max[i] = max[i].max(pos[i]);
        }
    }

    (min.to_vec(), max.to_vec())
}

/// Encode float slice to byte buffer.
pub fn encode_floats(data: &[f32]) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(data.len() * 4);
    for &value in data {
        buffer.extend_from_slice(&value.to_le_bytes());
    }
    buffer
}

/// Encode u32 slice to byte buffer.
pub fn encode_u32s(data: &[u32]) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(data.len() * 4);
    for &value in data {
        buffer.extend_from_slice(&value.to_le_bytes());
    }
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_bounds() {
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 2.0, 3.0],
            [-1.0, -2.0, -3.0],
        ];

        let (min, max) = calculate_bounds(&positions);
        assert_eq!(min, vec![-1.0, -2.0, -3.0]);
        assert_eq!(max, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_encode_floats() {
        let data = vec![1.0f32, 2.0f32, 3.0f32];
        let encoded = encode_floats(&data);
        assert_eq!(encoded.len(), 12); // 3 floats * 4 bytes
    }

    #[test]
    fn test_encode_u32s() {
        let data = vec![0u32, 1u32, 2u32];
        let encoded = encode_u32s(&data);
        assert_eq!(encoded.len(), 12); // 3 u32s * 4 bytes
    }
}
