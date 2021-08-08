use crate::consts::VIEWPORT_ROTATE_SNAP_INTERVAL;
use glam::{DAffine2, DVec2};
use graphene::layers::{BlendMode, LayerDataType};
use graphene::{
	layers::{Layer, LayerData as DocumentLayerData},
	LayerId,
};
use serde::{ser::SerializeSeq, Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy)]
pub struct LayerData {
	pub selected: bool,
	pub expanded: bool,
	pub translation: DVec2,
	pub rotation: f64,
	pub snap_rotate: bool,
	pub scale: f64,
}

impl LayerData {
	pub fn new(expanded: bool) -> LayerData {
		LayerData {
			selected: false,
			expanded,
			translation: DVec2::ZERO,
			rotation: 0.,
			snap_rotate: false,
			scale: 1.,
		}
	}

	pub fn snapped_angle(&self) -> f64 {
		let increment_radians: f64 = VIEWPORT_ROTATE_SNAP_INTERVAL.to_radians();
		if self.snap_rotate {
			(self.rotation / increment_radians).round() * increment_radians
		} else {
			self.rotation
		}
	}

	pub fn calculate_offset_transform(&self, offset: DVec2) -> DAffine2 {
		// TODO: replace with DAffine2::from_scale_angle_translation and fix the errors
		let offset_transform = DAffine2::from_translation(offset);
		let scale_transform = DAffine2::from_scale(DVec2::new(self.scale, self.scale));
		let angle_transform = DAffine2::from_angle(self.snapped_angle());
		let translation_transform = DAffine2::from_translation(self.translation);
		scale_transform * offset_transform * angle_transform * translation_transform
	}
}

pub fn layer_data<'a>(layer_data: &'a mut HashMap<Vec<LayerId>, LayerData>, path: &[LayerId]) -> &'a mut LayerData {
	if !layer_data.contains_key(path) {
		layer_data.insert(path.to_vec(), LayerData::new(false));
	}
	layer_data.get_mut(path).unwrap()
}

pub fn layer_panel_entry(layer_data: &LayerData, transform: DAffine2, layer: &Layer, path: Vec<LayerId>) -> LayerPanelEntry {
	let layer_type: LayerType = (&layer.data).into();
	let name = layer.name.clone().unwrap_or_else(|| format!("Unnamed {}", layer_type));
	let arr = layer.data.bounding_box(transform).unwrap_or([DVec2::ZERO, DVec2::ZERO]);
	let arr = arr.iter().map(|x| (*x).into()).collect::<Vec<(f64, f64)>>();

	let mut thumbnail = String::new();
	layer.data.clone().render(&mut thumbnail, &mut vec![transform]);
	let transform = transform.to_cols_array().iter().map(ToString::to_string).collect::<Vec<_>>().join(",");
	let thumbnail = if let [(x_min, y_min), (x_max, y_max)] = arr.as_slice() {
		format!(
			r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}"><g transform="matrix({})">{}</g></svg>"#,
			x_min,
			y_min,
			x_max - x_min,
			y_max - y_min,
			transform,
			thumbnail,
		)
	} else {
		String::new()
	};

	LayerPanelEntry {
		name,
		visible: layer.visible,
		blend_mode: layer.blend_mode,
		opacity: layer.opacity,
		layer_type: (&layer.data).into(),
		layer_data: *layer_data,
		path: path.into(),
		thumbnail,
	}
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Path(Vec<LayerId>);

impl From<Vec<LayerId>> for Path {
	fn from(iter: Vec<LayerId>) -> Self {
		Self(iter)
	}
}
impl Serialize for Path {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
		for e in self.0.iter() {
			#[cfg(target_arch = "wasm32")]
			{
				// LayerIds are sent as (u32, u32) because json does not support u64s
				let id = ((e >> 32) as u32, (e << 32 >> 32) as u32);
				seq.serialize_element(&id)?;
			}
			#[cfg(not(target_arch = "wasm32"))]
			seq.serialize_element(e)?;
		}
		seq.end()
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayerPanelEntry {
	pub name: String,
	pub visible: bool,
	pub blend_mode: BlendMode,
	pub opacity: f64,
	pub layer_type: LayerType,
	pub layer_data: LayerData,
	pub path: crate::document::layer_panel::Path,
	pub thumbnail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LayerType {
	Folder,
	Shape,
}

impl fmt::Display for LayerType {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		let name = match self {
			LayerType::Folder => "Folder",
			LayerType::Shape => "Shape",
		};

		formatter.write_str(name)
	}
}

impl From<&LayerDataType> for LayerType {
	fn from(data: &LayerDataType) -> Self {
		use LayerDataType::*;
		match data {
			Folder(_) => LayerType::Folder,
			Shape(_) => LayerType::Shape,
		}
	}
}
