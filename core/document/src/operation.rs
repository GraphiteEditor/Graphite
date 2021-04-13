use crate::LayerId;

pub enum Operation {
	AddCircle {
		path: Vec<LayerId>,
		insert_index: isize,
		cx: f64,
		cy: f64,
		r: f64,
	},
	AddRect {
		path: Vec<LayerId>,
		insert_index: isize,
		x0: f64,
		y0: f64,
		x1: f64,
		y1: f64,
	},
	AddLine {
		path: Vec<LayerId>,
		insert_index: isize,
		x0: f64,
		y0: f64,
		x1: f64,
		y1: f64,
	},
	DeleteLayer {
		path: Vec<LayerId>,
	},
	AddFolder {
		path: Vec<LayerId>,
	},
}
