use crate::{layers::layer_props, LayerId};

pub enum Operation {
	AddCircle {
		path: Vec<LayerId>,
		insert_index: isize,
		cx: f64,
		cy: f64,
		r: f64,
		stroke: Option<layer_props::Stroke>,
		fill: Option<layer_props::Fill>,
	},
	AddRect {
		path: Vec<LayerId>,
		insert_index: isize,
		x0: f64,
		y0: f64,
		x1: f64,
		y1: f64,
		stroke: Option<layer_props::Stroke>,
		fill: Option<layer_props::Fill>,
	},
	AddLine {
		path: Vec<LayerId>,
		insert_index: isize,
		x0: f64,
		y0: f64,
		x1: f64,
		y1: f64,
		stroke: Option<layer_props::Stroke>,
	},
	AddShape {
		path: Vec<LayerId>,
		insert_index: isize,
		x0: f64,
		y0: f64,
		x1: f64,
		y1: f64,
		sides: u8,
		stroke: Option<layer_props::Stroke>,
		fill: Option<layer_props::Fill>,
	},
	DeleteLayer {
		path: Vec<LayerId>,
	},
	AddFolder {
		path: Vec<LayerId>,
	},
}
