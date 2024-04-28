mod attributes;
mod modification;

use core::borrow::Borrow;

use super::style::{PathStyle, Stroke};
use crate::{AlphaBlending, Color};
pub use attributes::*;
pub use modification::*;

use bezier_rs::ManipulatorGroup;
use dyn_any::{DynAny, StaticType};

use glam::{DAffine2, DVec2};

/// [VectorData] is passed between nodes.
/// It contains a list of subpaths (that may be open or closed), a transform, and some style information.
#[derive(Clone, Debug, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VectorData {
	pub transform: DAffine2,
	pub style: PathStyle,
	pub alpha_blending: AlphaBlending,
	/// A list of all manipulator groups (referenced in `subpaths`) that have colinear handles (where they're locked at 180° angles from one another).
	/// This gets read in `graph_operation_message_handler.rs` by calling `inputs.as_mut_slice()` (search for the string `"Shape does not have both `subpath` and `colinear_manipulators` inputs"` to find it).
	pub colinear_manipulators: Vec<PointId>,

	pub point_domain: PointDomain,
	pub segment_domain: SegmentDomain,
	pub region_domain: RegionDomain,
}

impl core::hash::Hash for VectorData {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.point_domain.hash(state);
		self.segment_domain.hash(state);
		self.region_domain.hash(state);
		self.transform.to_cols_array().iter().for_each(|x| x.to_bits().hash(state));
		self.style.hash(state);
		self.alpha_blending.hash(state);
		self.colinear_manipulators.hash(state);
	}
}

impl VectorData {
	/// An empty subpath with no data, an identity transform, and a black fill.
	pub const fn empty() -> Self {
		Self {
			transform: DAffine2::IDENTITY,
			style: PathStyle::new(Some(Stroke::new(Some(Color::BLACK), 0.)), super::style::Fill::None),
			alpha_blending: AlphaBlending::new(),
			colinear_manipulators: Vec::new(),
			point_domain: PointDomain::new(),
			segment_domain: SegmentDomain::new(),
			region_domain: RegionDomain::new(),
		}
	}

	/// Construct some new vector data from a single subpath with an identity transform and black fill.
	pub fn from_subpath(subpath: impl Borrow<bezier_rs::Subpath<PointId>>) -> Self {
		Self::from_subpaths([subpath])
	}

	/// Push a subpath to the vector data
	pub fn append_subpath(&mut self, subpath: impl Borrow<bezier_rs::Subpath<PointId>>) {
		let subpath = subpath.borrow();
		let stroke_id = StrokeId::ZERO;
		for point in subpath.manipulator_groups() {
			let id = if self.point_domain.ids().contains(&point.id) { self.point_domain.next_id() } else { point.id };
			self.point_domain.push(id, point.anchor, Vec::new());
		}

		let handles = |a: &ManipulatorGroup<_>, b: &ManipulatorGroup<_>| match (a.out_handle, b.in_handle) {
			(None, None) => bezier_rs::BezierHandles::Linear,
			(Some(handle), None) | (None, Some(handle)) => bezier_rs::BezierHandles::Quadratic { handle },
			(Some(handle_start), Some(handle_end)) => bezier_rs::BezierHandles::Cubic { handle_start, handle_end },
		};
		let [mut first_seg, mut last_seg] = [None, None];
		let mut id = self.segment_domain.next_id();
		for pair in subpath.manipulator_groups().windows(2) {
			let id = id.next_id();
			first_seg = Some(first_seg.unwrap_or(id));
			last_seg = Some(id);
			self.segment_domain.push(id, pair[0].id, pair[1].id, handles(&pair[0], &pair[1]), stroke_id);
		}

		let fill_id = FillId::ZERO;

		if subpath.closed() {
			if let (Some(last), Some(first)) = (subpath.manipulator_groups().last(), subpath.manipulator_groups().first()) {
				let id = id.next_id();
				first_seg = Some(first_seg.unwrap_or(id));
				last_seg = Some(id);
				self.segment_domain.push(id, last.id, first.id, handles(last, first), stroke_id);
			}

			if let [Some(first_seg), Some(last_seg)] = [first_seg, last_seg] {
				self.region_domain.push(RegionId::generate(), first_seg..=last_seg, fill_id);
			}
		}
	}

	/// Construct some new vector data from subpaths with an identity transform and black fill.
	pub fn from_subpaths(subpaths: impl IntoIterator<Item = impl Borrow<bezier_rs::Subpath<PointId>>>) -> Self {
		let mut vector_data = Self::empty();

		for subpath in subpaths.into_iter() {
			vector_data.append_subpath(subpath);
		}

		vector_data
	}

	/// Compute the bounding boxes of the subpaths without any transform
	pub fn bounding_box(&self) -> Option<[DVec2; 2]> {
		self.bounding_box_with_transform(DAffine2::IDENTITY)
	}

	/// Compute the bounding boxes of the subpaths with the specified transform
	pub fn bounding_box_with_transform(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.segment_bezier_iter()
			.map(|(_, bezier, _, _)| bezier.apply_transformation(|point| transform.transform_point2(point)).bounding_box())
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
	pub fn layerspace_pivot(&self, normalized_pivot: DVec2) -> DVec2 {
		let [bounds_min, bounds_max] = self.nonzero_bounding_box();
		let bounds_size = bounds_max - bounds_min;
		bounds_min + bounds_size * normalized_pivot
	}

	/// Compute the pivot in local space with the current transform applied
	pub fn local_pivot(&self, normalized_pivot: DVec2) -> DVec2 {
		self.transform.transform_point2(self.layerspace_pivot(normalized_pivot))
	}

	/// Points connected to a single segment
	pub fn single_connected_points(&self) -> impl Iterator<Item = PointId> + '_ {
		self.point_domain.ids().iter().copied().filter(|&point| self.segment_domain.connected(point) == 1)
	}
}

impl Default for VectorData {
	fn default() -> Self {
		Self::empty()
	}
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ManipulatorPointId {
	Anchor(PointId),
	PrimaryHandle(SegmentId),
	EndHandle(SegmentId),
}
impl ManipulatorPointId {
	pub fn get_position(&self, vector_data: &VectorData) -> Option<DVec2> {
		match self {
			ManipulatorPointId::Anchor(id) => vector_data.point_domain.pos_from_id(*id),
			ManipulatorPointId::PrimaryHandle(id) => vector_data.segment_from_id(*id).and_then(|bezier| bezier.handle_start()),
			ManipulatorPointId::EndHandle(id) => vector_data.segment_from_id(*id).and_then(|bezier| bezier.handle_end()),
		}
	}
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SelectedType {
	Anchor = 1 << 0,
	InHandle = 1 << 1,
	OutHandle = 1 << 2,
}
impl SelectedType {
	/// Get the location of the [SelectedType] in the [ManipulatorGroup]
	pub fn get_position(&self, manipulator_group: &ManipulatorGroup<PointId>) -> Option<DVec2> {
		match self {
			Self::Anchor => Some(manipulator_group.anchor),
			Self::InHandle => manipulator_group.in_handle,
			Self::OutHandle => manipulator_group.out_handle,
		}
	}

	/// Get the closest [SelectedType] in the [ManipulatorGroup].
	pub fn closest_widget(manipulator_group: &ManipulatorGroup<PointId>, transform_space: DAffine2, target: DVec2, hide_handle_distance: f64) -> (Self, f64) {
		let anchor = transform_space.transform_point2(manipulator_group.anchor);
		// Skip handles under the anchor
		let not_under_anchor = |&(selected_type, position): &(SelectedType, DVec2)| selected_type == Self::Anchor || position.distance_squared(anchor) > hide_handle_distance.powi(2);
		let compute_distance = |selected_type: Self| {
			selected_type.get_position(manipulator_group).and_then(|position| {
				Some((selected_type, transform_space.transform_point2(position)))
					.filter(not_under_anchor)
					.map(|(selected_type, pos)| (selected_type, pos.distance_squared(target)))
			})
		};
		[Self::Anchor, Self::InHandle, Self::OutHandle]
			.into_iter()
			.filter_map(compute_distance)
			.min_by(|a, b| a.1.total_cmp(&b.1))
			.unwrap_or((Self::Anchor, manipulator_group.anchor.distance_squared(target)))
	}

	/// Opposite handle
	pub fn opposite(&self) -> Self {
		match self {
			SelectedType::Anchor => SelectedType::Anchor,
			SelectedType::InHandle => SelectedType::OutHandle,
			SelectedType::OutHandle => SelectedType::InHandle,
		}
	}

	/// Check if handle
	pub fn is_handle(self) -> bool {
		self != SelectedType::Anchor
	}
}

#[test]
fn construct_closed_subpath() {
	let circle = bezier_rs::Subpath::new_ellipse(DVec2::NEG_ONE, DVec2::ONE);
	let vector_data = VectorData::from_subpath(&circle);
	assert_eq!(vector_data.point_domain.ids().len(), 4);
	let bézier_paths = vector_data.segment_bezier_iter().map(|(_, bézier, _, _)| bézier).collect::<Vec<_>>();
	assert_eq!(bézier_paths.len(), 4);
	assert!(bézier_paths.iter().all(|bézier| circle.iter().find(|original_bézier| original_bézier == bézier).is_some()));

	let generated = vector_data.stroke_bezier_paths().collect::<Vec<_>>();
	assert_eq!(generated, vec![circle]);
}

#[test]
fn construct_open_subpath() {
	let bézier = bezier_rs::Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::NEG_ONE, DVec2::ONE, DVec2::X);
	let subpath = bezier_rs::Subpath::from_bezier(&bézier);
	let vector_data = VectorData::from_subpath(&subpath);
	assert_eq!(vector_data.point_domain.ids().len(), 2);
	let bézier_paths = vector_data.segment_bezier_iter().map(|(_, bézier, _, _)| bézier).collect::<Vec<_>>();
	assert_eq!(bézier_paths, vec![bézier]);

	let generated = vector_data.stroke_bezier_paths().collect::<Vec<_>>();
	assert_eq!(generated, vec![subpath]);
}
