use std::hash::{Hash, Hasher};

use crate::{
	color::Color,
	layers::{style, BlendMode, Layer},
	LayerId,
};

use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Debug, Clone, Deserialize, Serialize)]
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
	AddBoundingBox {
		path: Vec<LayerId>,
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
		sides: u8,
		style: style::PathStyle,
	},
	DeleteLayer {
		path: Vec<LayerId>,
	},
	DuplicateLayer {
		path: Vec<LayerId>,
	},
	RenameLayer {
		path: Vec<LayerId>,
		name: String,
	},
	PasteLayer {
		layer: Layer,
		path: Vec<LayerId>,
		insert_index: isize,
	},
	AddFolder {
		path: Vec<LayerId>,
	},
	TransformLayer {
		path: Vec<LayerId>,
		transform: [f64; 6],
	},
	TransformLayerInViewport {
		path: Vec<LayerId>,
		transform: [f64; 6],
	},
	SetLayerTransformInViewport {
		path: Vec<LayerId>,
		transform: [f64; 6],
	},
	TransformLayerInScope {
		path: Vec<LayerId>,
		transform: [f64; 6],
		scope: [f64; 6],
	},
	SetLayerTransformInScope {
		path: Vec<LayerId>,
		transform: [f64; 6],
		scope: [f64; 6],
	},
	SetLayerTransform {
		path: Vec<LayerId>,
		transform: [f64; 6],
	},
	ToggleVisibility {
		path: Vec<LayerId>,
	},
	SetLayerBlendMode {
		path: Vec<LayerId>,
		blend_mode: BlendMode,
	},
	SetLayerOpacity {
		path: Vec<LayerId>,
		opacity: f64,
	},
	FillLayer {
		path: Vec<LayerId>,
		color: Color,
	},
}

impl Hash for Operation {
	fn hash<H>(&self, state: &mut H)
	where
		H: Hasher,
	{
		unsafe { std::mem::transmute::<&Operation, &[u8; std::mem::size_of::<Operation>()]>(self) }.hash(state);
	}
}

impl PartialEq for Operation {
	fn eq(&self, other: &Operation) -> bool {
		// TODO: Replace with let [s, o] = [self, other].map(|x| unsafe { std::mem::transmute::<&Operation, &[u8; std::mem::size_of::<Operation>()]>(x) });
		let vals: Vec<_> = [self, other]
			.iter()
			.map(|x| unsafe { std::mem::transmute::<&Operation, &[u8; std::mem::size_of::<Operation>()]>(x) })
			.collect();
		vals[0] == vals[1]
	}
}
