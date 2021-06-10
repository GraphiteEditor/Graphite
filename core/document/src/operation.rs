use crate::{layers::style, LayerId};

use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum Operation {
	AddEllipse {
		path: Vec<LayerId>,
		insert_index: isize,
		cols: [f64; 6],
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
