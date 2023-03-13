use super::style::{PathStyle, Stroke};
use crate::{uuid::ManipulatorGroupId, Color};

use dyn_any::{DynAny, StaticType};
use glam::{DAffine2, DVec2};

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
	/// An empty subpath with no data, an identity transform and a black fill.
	pub const fn empty() -> Self {
		Self {
			subpaths: Vec::new(),
			transform: DAffine2::IDENTITY,
			style: PathStyle::new(Some(Stroke::new(Color::BLACK, 0.)), super::style::Fill::None),
		}
	}

	/// Construct some new vector data from a single subpath with an identy transform and black fill.
	pub fn from_subpath(subpath: bezier_rs::Subpath<ManipulatorGroupId>) -> Self {
		Self::from_subpaths(vec![subpath])
	}

	/// Construct some new vector data from subpaths with an identy transform and black fill.
	pub fn from_subpaths(subpaths: Vec<bezier_rs::Subpath<ManipulatorGroupId>>) -> Self {
		super::VectorData { subpaths, ..Self::empty() }
	}

	/// Compute the bounding boxes of the subpaths without any transform
	pub fn bounding_box(&self) -> Option<[DVec2; 2]> {
		self.bounding_box_with_transform(DAffine2::IDENTITY)
	}

	/// Compute the bounding boxes of the subpaths with the specified transform
	pub fn bounding_box_with_transform(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.subpaths
			.iter()
			.filter_map(|subpath| subpath.bounding_box_with_transform(transform))
			.reduce(|b1, b2| [b1[0].min(b2[0]), b1[1].max(b2[1])])
	}

	/// Calculate the corners of the bounding box but with a nonzero size.
	///
	/// If the layer bounds are `0` in either axis then they are changed to be `1`.
	pub fn nonzero_bounding_box(&self) -> [DVec2; 2] {
		let [bounds_min, mut bounds_max] = self.bounding_box().unwrap_or_default();

		let bounds_size = bounds_max - bounds_min;
		if bounds_size.x < 1e-10 {
			bounds_max.x = bounds_min.x + 1.;
		}
		if bounds_size.y < 1e-10 {
			bounds_max.y = bounds_min.y + 1.;
		}

		[bounds_min, bounds_max]
	}

	/// Compute the pivot of the layer in layerspace (the coordinates of the subpaths)
	pub fn layerspace_pivot(&self, normalised_pivot: DVec2) -> DVec2 {
		let [bounds_min, bounds_max] = self.nonzero_bounding_box();
		let bounds_size = bounds_max - bounds_min;
		bounds_min + bounds_size * normalised_pivot
	}

	/// Compute the pivot in local space with the current transform applied
	pub fn local_pivot(&self, normalised_pivot: DVec2) -> DVec2 {
		self.transform.transform_point2(self.layerspace_pivot(normalised_pivot))
	}
}

impl Default for VectorData {
	fn default() -> Self {
		Self::empty()
	}
}
