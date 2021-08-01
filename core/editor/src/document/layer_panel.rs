use crate::{consts::ROTATE_SNAP_INTERVAL, frontend::layer_panel::*};
use document_core::{
	layers::{Layer, LayerData as DocumentLayerData},
	LayerId,
};
use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
		let increment_radians: f64 = ROTATE_SNAP_INTERVAL.to_radians();
		if self.snap_rotate {
			(self.rotation / increment_radians).round() * increment_radians
		} else {
			self.rotation
		}
	}
	pub fn calculate_offset_transform(&self, offset: DVec2) -> DAffine2 {
		let offset_transform = DAffine2::from_translation(offset);
		let scale_transform = DAffine2::from_scale(DVec2::new(self.scale, self.scale));
		let angle_transform = DAffine2::from_angle(self.snapped_angle());
		let translation_transform = DAffine2::from_translation(self.translation);
		scale_transform * offset_transform * angle_transform * scale_transform * translation_transform
	}
	pub fn calculate_transform(&self) -> DAffine2 {
		self.calculate_offset_transform(DVec2::ZERO)
	}
}

pub fn layer_data<'a>(layer_data: &'a mut HashMap<Vec<LayerId>, LayerData>, path: &[LayerId]) -> &'a mut LayerData {
	if !layer_data.contains_key(path) {
		layer_data.insert(path.to_vec(), LayerData::new(false));
	}
	layer_data.get_mut(path).unwrap()
}

pub fn layer_panel_entry(layer_data: &LayerData, transform: DAffine2, layer: &Layer, path: Vec<LayerId>) -> LayerPanelEntry {
	let blend_mode = layer.blend_mode;
	let opacity = layer.opacity;
	let layer_type: LayerType = (&layer.data).into();
	let name = layer.name.clone().unwrap_or_else(|| format!("Unnamed {}", layer_type));
	let arr = layer.data.bounding_box(transform).unwrap_or([DVec2::ZERO, DVec2::ZERO]);
	let arr = arr.iter().map(|x| (*x).into()).collect::<Vec<(f64, f64)>>();

	let thumbnail = if let [(x_min, y_min), (x_max, y_max)] = arr.as_slice() {
		format!(
			r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}">{}</svg>"#,
			x_min,
			y_min,
			x_max - x_min,
			y_max - y_min,
			layer.thumbnail_cache.clone()
		)
	} else {
		String::new()
	};
	let path = path.iter().map(|id| ((id >> 32) as u32, (id << 32 >> 32) as u32)).collect::<Vec<_>>();

	LayerPanelEntry {
		name,
		visible: layer.visible,
		blend_mode,
		opacity,
		layer_type,
		layer_data: *layer_data,
		path,
		thumbnail,
	}
}
