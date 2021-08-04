use std::{
	collections::hash_map::DefaultHasher,
	hash::{Hash, Hasher},
};

use crate::{
	color::Color,
	layers::{style, BlendMode, Layer},
	LayerId,
};

use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
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

impl Operation {
	unsafe fn as_slice(&self) -> &[u8] {
		core::slice::from_raw_parts(self as *const Operation as *const u8, std::mem::size_of::<Operation>())
	}
	pub fn pseudo_hash(&self) -> u64 {
		let mut s = DefaultHasher::new();
		unsafe { self.as_slice() }.hash(&mut s);
		s.finish()
	}
}
