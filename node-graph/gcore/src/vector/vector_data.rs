use super::style::{PathStyle, Stroke};
use crate::{uuid::ManipulatorGroupId, Color};

use dyn_any::{DynAny, StaticType};
use glam::DAffine2;

/// [VectorData] is passed between nodes.
/// It contains a list of subpaths (that may be open or closed), a transform and some style information.
#[derive(Clone, Debug, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VectorData {
	pub subpaths: Vec<bezier_rs::Subpath<ManipulatorGroupId>>,
	pub transform: DAffine2,
	pub style: PathStyle,
}

impl VectorData {
	pub const fn empty() -> Self {
		Self {
			subpaths: Vec::new(),
			transform: DAffine2::IDENTITY,
			style: PathStyle::new(Some(Stroke::new(Color::BLACK, 0.)), super::style::Fill::Solid(Color::BLACK)),
		}
	}

	pub fn from_subpath(subpath: bezier_rs::Subpath<ManipulatorGroupId>) -> Self {
		super::VectorData {
			subpaths: vec![subpath],
			transform: DAffine2::IDENTITY,
			style: PathStyle::default(),
		}
	}
}
