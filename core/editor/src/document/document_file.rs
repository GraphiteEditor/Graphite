use crate::{consts::ROTATE_SNAP_INTERVAL, frontend::layer_panel::*, EditorError};
use document_core::{document::Document as InternalDocument, layers::Layer, LayerId};
use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Document {
	pub document: InternalDocument,
	pub name: String,
	pub layer_data: HashMap<Vec<LayerId>, LayerData>,
}

impl Default for Document {
	fn default() -> Self {
		Self {
			document: InternalDocument::default(),
			name: String::from("Untitled Document"),
			layer_data: vec![(vec![], LayerData::new(true))].into_iter().collect(),
		}
	}
}

impl Document {
	pub fn with_name(name: String) -> Self {
		Self {
			document: InternalDocument::default(),
			name,
			layer_data: vec![(vec![], LayerData::new(true))].into_iter().collect(),
		}
	}
}

fn layer_data<'a>(layer_data: &'a mut HashMap<Vec<LayerId>, LayerData>, path: &[LayerId]) -> &'a mut LayerData {
	if !layer_data.contains_key(path) {
		layer_data.insert(path.to_vec(), LayerData::new(false));
	}
	layer_data.get_mut(path).unwrap()
}

pub fn layer_panel_entry(layer_data: &mut LayerData, layer: &Layer, path: Vec<LayerId>) -> LayerPanelEntry {
	let layer_type: LayerType = (&layer.data).into();
	let name = layer.name.clone().unwrap_or_else(|| format!("Unnamed {}", layer_type));
	LayerPanelEntry {
		name,
		visible: layer.visible,
		layer_type,
		layer_data: *layer_data,
		path,
	}
}

impl Document {
	pub fn layer_data(&mut self, path: &[LayerId]) -> &mut LayerData {
		layer_data(&mut self.layer_data, path)
	}

	/// Returns a list of `LayerPanelEntry`s intended for display purposes. These don't contain
	/// any actual data, but rather metadata such as visibility and names of the layers.
	pub fn layer_panel(&mut self, path: &[LayerId]) -> Result<Vec<LayerPanelEntry>, EditorError> {
		let folder = self.document.document_folder(path)?;
		let self_layer_data = &mut self.layer_data;
		let entries = folder
			.as_folder()?
			.layers()
			.iter()
			.zip(folder.as_folder()?.layer_ids.iter())
			.rev()
			.map(|(layer, id)| {
				let path = [path, &[*id]].concat();
				layer_panel_entry(layer_data(self_layer_data, &path), layer, path)
			})
			.collect();
		Ok(entries)
	}
}

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
		let translation_transform = DAffine2::from_translation(self.translation.into());
		scale_transform * offset_transform * angle_transform * scale_transform * translation_transform
	}
	pub fn calculate_transform(&self) -> DAffine2 {
		self.calculate_offset_transform(DVec2::ZERO)
	}
}
