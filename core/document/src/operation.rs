use crate::{
	layers::{style, Layer},
	LayerId,
};

use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum Operation {
	AddCircle {
		path: Vec<LayerId>,
		insert_index: isize,
		cx: f64,
		cy: f64,
		r: f64,
		style: style::PathStyle,
	},
	AddEllipse {
		path: Vec<LayerId>,
		insert_index: isize,
		cx: f64,
		cy: f64,
		rx: f64,
		ry: f64,
		rot: f64,
		style: style::PathStyle,
	},
	AddRect {
		path: Vec<LayerId>,
		insert_index: isize,
		x0: f64,
		y0: f64,
		x1: f64,
		y1: f64,
		style: style::PathStyle,
	},
	AddLine {
		path: Vec<LayerId>,
		insert_index: isize,
		x0: f64,
		y0: f64,
		x1: f64,
		y1: f64,
		style: style::PathStyle,
	},
	AddPen {
		path: Vec<LayerId>,
		insert_index: isize,
		points: Vec<(f64, f64)>,
		style: style::PathStyle,
	},
	AddShape {
		path: Vec<LayerId>,
		insert_index: isize,
		x0: f64,
		y0: f64,
		x1: f64,
		y1: f64,
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
	DiscardWorkingFolder,
	ClearWorkingFolder,
	CommitTransaction,
	ToggleVisibility {
		path: Vec<LayerId>,
	},
}
