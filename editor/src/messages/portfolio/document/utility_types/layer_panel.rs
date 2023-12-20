use document_legacy::document::LayerId;
use document_legacy::layers::{LayerDataTypeDiscriminant, LegacyLayerType};

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
pub struct RawBuffer(Vec<u8>);

impl From<&[u64]> for RawBuffer {
	fn from(iter: &[u64]) -> Self {
		let v_from_raw: Vec<u8> = iter.iter().flat_map(|x| x.to_ne_bytes()).collect();
		Self(v_from_raw)
	}
}
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, specta::Type)]
pub struct JsRawBuffer(Vec<u8>);

impl From<RawBuffer> for JsRawBuffer {
	fn from(buffer: RawBuffer) -> Self {
		Self(buffer.0)
	}
}
impl Serialize for JsRawBuffer {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		let mut buffer = serializer.serialize_struct("Buffer", 2)?;
		buffer.serialize_field("pointer", &(self.0.as_ptr() as usize))?;
		buffer.serialize_field("length", &(self.0.len()))?;
		buffer.end()
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Copy, specta::Type)]
pub struct LayerMetadata {
	pub selected: bool,
	pub expanded: bool,
}

impl LayerMetadata {
	pub fn new(expanded: bool) -> Self {
		Self { selected: false, expanded }
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
pub struct LayerPanelEntry {
	pub name: String,
	pub tooltip: String,
	#[serde(rename = "layerType")]
	pub layer_type: LayerDataTypeDiscriminant,
	#[serde(rename = "layerMetadata")]
	pub layer_metadata: LayerMetadata,
	pub path: Vec<LayerId>,
	pub thumbnail: String,
}

impl LayerPanelEntry {
	// TODO: Deprecate this because it's using document-legacy layer data which is no longer linked to data from the node graph,
	// TODO: so this doesn't feed `name` (that's fed elsewhere) or `visible` (that's broken entirely), etc.
	pub fn new(layer_metadata: &LayerMetadata, layer: &LegacyLayerType, path: Vec<LayerId>) -> Self {
		Self {
			name: "".to_string(),    // Replaced before it gets used
			tooltip: "".to_string(), // Replaced before it gets used
			layer_type: layer.into(),
			layer_metadata: *layer_metadata,
			path,
			thumbnail: r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 0 0"></svg>"#.to_string(),
		}
	}
}
