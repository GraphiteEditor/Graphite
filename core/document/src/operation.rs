use crate::{
	color::Color,
	layers::{style, BlendMode, Layer},
	LayerId,
};

use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum Operation {
	AddEllipse {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		style: style::PathStyle,
	},
	AddRect {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		style: style::PathStyle,
	},
	AddLine {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		style: style::PathStyle,
	},
	AddPen {
		path: Vec<LayerId>,
		transform: [f64; 6],
		insert_index: isize,
		points: Vec<(f64, f64)>,
		style: style::PathStyle,
	},
	AddShape {
		path: Vec<LayerId>,
		insert_index: isize,
		transform: [f64; 6],
		equal_sides: bool,
		sides: u8,
		style: style::PathStyle,
	},
	DeleteLayer {
		path: Vec<LayerId>,
	},
	DuplicateLayer {
		path: Vec<LayerId>,
	},
	PasteLayer {
		layer: Layer,
		path: Vec<LayerId>,
	},
	AddFolder {
		path: Vec<LayerId>,
	},
	MountWorkingFolder {
		path: Vec<LayerId>,
	},
	TransformLayer {
		path: Vec<LayerId>,
		transform: [f64; 6],
	},
	SetLayerTransform {
		path: Vec<LayerId>,
		transform: [f64; 6],
	},
	DiscardWorkingFolder,
	ClearWorkingFolder,
	CommitTransaction,
	ToggleVisibility {
		path: Vec<LayerId>,
	},
	SetLayerBlendMode {
		path: Vec<LayerId>,
		blend_mode: BlendMode,
	},
	FillLayer {
		path: Vec<LayerId>,
		color: Color,
	},
	ReorderLayers {
		source_paths: Vec<Vec<LayerId>>,
		target_path: Vec<LayerId>,
	},
}
