use document_legacy::layers::layer_info::{Layer, LayerData, LayerDataTypeDiscriminant};
use document_legacy::layers::style::RenderData;
use document_legacy::LayerId;

use glam::{DAffine2, DVec2};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
pub struct RawBuffer(Vec<u8>);

impl From<&[u64]> for RawBuffer {
	fn from(iter: &[u64]) -> Self {
		// https://github.com/rust-lang/rust-clippy/issues/4484
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
	pub visible: bool,
	#[serde(rename = "layerType")]
	pub layer_type: LayerDataTypeDiscriminant,
	#[serde(rename = "layerMetadata")]
	pub layer_metadata: LayerMetadata,
	pub path: Vec<LayerId>,
	pub thumbnail: String,
}

impl LayerPanelEntry {
	pub fn new(layer_metadata: &LayerMetadata, transform: DAffine2, layer: &Layer, path: Vec<LayerId>, render_data: &RenderData) -> Self {
		let name = layer.name.clone().unwrap_or_else(|| String::from(""));

		let mut tooltip = name.clone();
		if cfg!(debug_assertions) {
			tooltip += "\nLayer Path: ";
			tooltip += &path.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(" / ");
			tooltip = tooltip.trim().to_string();
		}

		let arr = layer.data.bounding_box(transform, render_data).unwrap_or([DVec2::ZERO, DVec2::ZERO]);
		let arr = arr.iter().map(|x| (*x).into()).collect::<Vec<(f64, f64)>>();
		let mut thumbnail = String::new();
		let mut svg_defs = String::new();
		layer.data.clone().render(&mut thumbnail, &mut svg_defs, &mut vec![transform], render_data);
		let transform = transform.to_cols_array().iter().map(ToString::to_string).collect::<Vec<_>>().join(",");
		let thumbnail = if let [(x_min, y_min), (x_max, y_max)] = arr.as_slice() {
			format!(
				r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}"><defs>{}</defs><g transform="matrix({})">{}</g></svg>"#,
				x_min,
				y_min,
				x_max - x_min,
				y_max - y_min,
				svg_defs,
				transform,
				thumbnail,
			)
		} else {
			String::new()
		};

		LayerPanelEntry {
			name,
			tooltip,
			visible: layer.visible,
			layer_type: (&layer.data).into(),
			layer_metadata: *layer_metadata,
			path,
			thumbnail,
		}
	}
}
