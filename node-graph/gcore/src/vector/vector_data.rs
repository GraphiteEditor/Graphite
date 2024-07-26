mod attributes;
mod modification;
pub use attributes::*;
pub use modification::*;

use super::style::{PathStyle, Stroke};
use crate::{AlphaBlending, Color};

use bezier_rs::ManipulatorGroup;
use dyn_any::{DynAny, StaticType};

use core::borrow::Borrow;
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
	pub colinear_manipulators: Vec<[HandleId; 2]>,

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
		Self::from_subpaths([subpath], false)
	}

	/// Push a subpath to the vector data
	pub fn append_subpath(&mut self, subpath: impl Borrow<bezier_rs::Subpath<PointId>>, preserve_id: bool) {
		let subpath: &bezier_rs::Subpath<PointId> = subpath.borrow();
		let stroke_id = StrokeId::ZERO;
		let mut point_id = self.point_domain.next_id();

		let handles = |a: &ManipulatorGroup<_>, b: &ManipulatorGroup<_>| match (a.out_handle, b.in_handle) {
			(None, None) => bezier_rs::BezierHandles::Linear,
			(Some(handle), None) | (None, Some(handle)) => bezier_rs::BezierHandles::Quadratic { handle },
			(Some(handle_start), Some(handle_end)) => bezier_rs::BezierHandles::Cubic { handle_start, handle_end },
		};
		let [mut first_seg, mut last_seg] = [None, None];
		let mut segment_id = self.segment_domain.next_id();
		let mut last_point = None;
		let mut first_point = None;
		for pair in subpath.manipulator_groups().windows(2) {
			let start = last_point.unwrap_or_else(|| {
				let id = if preserve_id && !self.point_domain.ids().contains(&pair[0].id) {
					pair[0].id
				} else {
					point_id.next_id()
				};
				self.point_domain.push(id, pair[0].anchor);
				id
			});
			first_point = Some(first_point.unwrap_or(start));
			let end = if preserve_id && !self.point_domain.ids().contains(&pair[1].id) {
				pair[1].id
			} else {
				point_id.next_id()
			};
			self.point_domain.push(end, pair[1].anchor);

			let id = segment_id.next_id();
			first_seg = Some(first_seg.unwrap_or(id));
			last_seg = Some(id);
			self.segment_domain.push(id, start, end, handles(&pair[0], &pair[1]), stroke_id);

			last_point = Some(end);
		}

		let fill_id = FillId::ZERO;

		if subpath.closed() {
			if let (Some(last), Some(first), Some(first_id), Some(last_id)) = (subpath.manipulator_groups().last(), subpath.manipulator_groups().first(), first_point, last_point) {
				let id = segment_id.next_id();
				first_seg = Some(first_seg.unwrap_or(id));
				last_seg = Some(id);
				self.segment_domain.push(id, last_id, first_id, handles(last, first), stroke_id);
			}

			if let [Some(first_seg), Some(last_seg)] = [first_seg, last_seg] {
				self.region_domain.push(self.region_domain.next_id(), first_seg..=last_seg, fill_id);
			}
		}
	}

	/// Construct some new vector data from subpaths with an identity transform and black fill.
	pub fn from_subpaths(subpaths: impl IntoIterator<Item = impl Borrow<bezier_rs::Subpath<PointId>>>, preserve_id: bool) -> Self {
		let mut vector_data = Self::empty();

		for subpath in subpaths.into_iter() {
			vector_data.append_subpath(subpath, preserve_id);
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
		self.point_domain.ids().iter().copied().filter(|&point| self.segment_domain.connected_count(point) == 1)
	}

	/// Computes if all the connected handles are colinear for an anchor, or if that handle is colinear for a handle.
	pub fn colinear(&self, point: ManipulatorPointId) -> bool {
		let has_handle = |target| self.colinear_manipulators.iter().flatten().any(|&handle| handle == target);
		match point {
			ManipulatorPointId::Anchor(id) => {
				self.segment_domain.start_connected(id).all(|segment| has_handle(HandleId::primary(segment))) && self.segment_domain.end_connected(id).all(|segment| has_handle(HandleId::end(segment)))
			}
			ManipulatorPointId::PrimaryHandle(segment) => has_handle(HandleId::primary(segment)),
			ManipulatorPointId::EndHandle(segment) => has_handle(HandleId::end(segment)),
		}
	}

	pub fn other_colinear_handle(&self, handle: HandleId) -> Option<HandleId> {
		let pair = self.colinear_manipulators.iter().find(|pair| pair.iter().any(|&val| val == handle))?;
		let other = pair.iter().copied().find(|&val| val != handle)?;
		if handle.to_manipulator_point().get_anchor(self) == other.to_manipulator_point().get_anchor(self) {
			Some(other)
		} else {
			None
		}
	}
}

impl Default for VectorData {
	fn default() -> Self {
		Self::empty()
	}
}

/// A selectable part of a curve, either an anchor (start or end of a bézier) or a handle (doesn't necessarily go through the bézier but influences curviture).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ManipulatorPointId {
	/// A control anchor - the start or end point of a bézier.
	Anchor(PointId),
	/// The handle for a bézier - the first handle on a cubic and the only handle on a quadratic.
	PrimaryHandle(SegmentId),
	/// The end handle on a cubic bézier.
	EndHandle(SegmentId),
}

impl ManipulatorPointId {
	/// Attempt to retrieve the manipulator position in layer space (no transformation applied).
	#[must_use]
	pub fn get_position(&self, vector_data: &VectorData) -> Option<DVec2> {
		match self {
			ManipulatorPointId::Anchor(id) => vector_data.point_domain.position_from_id(*id),
			ManipulatorPointId::PrimaryHandle(id) => vector_data.segment_from_id(*id).and_then(|bezier| bezier.handle_start()),
			ManipulatorPointId::EndHandle(id) => vector_data.segment_from_id(*id).and_then(|bezier| bezier.handle_end()),
		}
	}

	/// Attempt to get a pair of handles. For an anchor this is the first to handles connected. For a handle it is self and the first opposing handle.
	#[must_use]
	pub fn get_handle_pair(self, vector_data: &VectorData) -> Option<[HandleId; 2]> {
		match self {
			ManipulatorPointId::Anchor(point) => vector_data.segment_domain.all_connected(point).take(2).collect::<Vec<_>>().try_into().ok(),
			ManipulatorPointId::PrimaryHandle(segment) => {
				let point = vector_data.segment_domain.segment_start_from_id(segment)?;
				let current = HandleId::primary(segment);
				let other = vector_data.segment_domain.all_connected(point).find(|&value| value != current);
				other.map(|other| [current, other])
			}
			ManipulatorPointId::EndHandle(segment) => {
				let point = vector_data.segment_domain.segment_end_from_id(segment)?;
				let current = HandleId::end(segment);
				let other = vector_data.segment_domain.all_connected(point).find(|&value| value != current);
				other.map(|other| [current, other])
			}
		}
	}

	/// Attempt to find the closest anchor. If self is already an anchor then it is just self. If it is a start or end handle, then the start or end point is chosen.
	#[must_use]
	pub fn get_anchor(self, vector_data: &VectorData) -> Option<PointId> {
		match self {
			ManipulatorPointId::Anchor(point) => Some(point),
			ManipulatorPointId::PrimaryHandle(segment) => vector_data.segment_domain.segment_start_from_id(segment),
			ManipulatorPointId::EndHandle(segment) => vector_data.segment_domain.segment_end_from_id(segment),
		}
	}

	/// Attempt to convert self to a [`HandleId`], returning none for an anchor.
	#[must_use]
	pub fn as_handle(self) -> Option<HandleId> {
		match self {
			ManipulatorPointId::PrimaryHandle(segment) => Some(HandleId::primary(segment)),
			ManipulatorPointId::EndHandle(segment) => Some(HandleId::end(segment)),
			ManipulatorPointId::Anchor(_) => None,
		}
	}

	/// Attempt to convert self to an anchor, returning None for a handle.
	#[must_use]
	pub fn as_anchor(self) -> Option<PointId> {
		match self {
			ManipulatorPointId::Anchor(point) => Some(point),
			_ => None,
		}
	}
}

/// The type of handle found on a bézier curve.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HandleType {
	/// The first handle on a cubic bézier or the only handle on a quadratic bézier.
	Primary,
	/// The second handle on a cubic bézier.
	End,
}

/// Represents a primary or end handle found in a particular segment.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HandleId {
	pub ty: HandleType,
	pub segment: SegmentId,
}

impl HandleId {
	/// Construct a handle for the first handle on a cubic bézier or the only handle on a quadratic bézier.
	#[must_use]
	pub const fn primary(segment: SegmentId) -> Self {
		Self { ty: HandleType::Primary, segment }
	}

	/// Construct a handle for the end handle on a cubic bézier.
	#[must_use]
	pub const fn end(segment: SegmentId) -> Self {
		Self { ty: HandleType::End, segment }
	}

	/// Convert to [`ManipulatorPointId`].
	#[must_use]
	pub fn to_manipulator_point(self) -> ManipulatorPointId {
		match self.ty {
			HandleType::Primary => ManipulatorPointId::PrimaryHandle(self.segment),
			HandleType::End => ManipulatorPointId::EndHandle(self.segment),
		}
	}

	/// Set the handle's position relative to the anchor which is the start anchor for the primary handle and end anchor for the end handle.
	#[must_use]
	pub fn set_relative_position(self, relative_position: DVec2) -> VectorModificationType {
		let Self { ty, segment } = self;
		match ty {
			HandleType::Primary => VectorModificationType::SetPrimaryHandle { segment, relative_position },
			HandleType::End => VectorModificationType::SetEndHandle { segment, relative_position },
		}
	}

	/// Convert an end handle to the primary handle and a primary handle to an end handle. Note that the new handle may not exist (e.g. for a quadratic bézier).
	#[must_use]
	pub fn opposite(self) -> Self {
		match self.ty {
			HandleType::Primary => Self::end(self.segment),
			HandleType::End => Self::primary(self.segment),
		}
	}
}

#[cfg(test)]
fn assert_subpath_eq(generated: &[bezier_rs::Subpath<PointId>], expected: &[bezier_rs::Subpath<PointId>]) {
	assert_eq!(generated.len(), expected.len());
	for (generated, expected) in generated.iter().zip(expected) {
		assert_eq!(generated.manipulator_groups().len(), expected.manipulator_groups().len());
		assert_eq!(generated.closed(), expected.closed());
		for (generated, expected) in generated.manipulator_groups().iter().zip(expected.manipulator_groups()) {
			assert_eq!(generated.in_handle, expected.in_handle);
			assert_eq!(generated.out_handle, expected.out_handle);
			assert_eq!(generated.anchor, expected.anchor);
		}
	}
}

#[test]
fn construct_closed_subpath() {
	let circle = bezier_rs::Subpath::new_ellipse(DVec2::NEG_ONE, DVec2::ONE);
	let vector_data = VectorData::from_subpath(&circle);
	assert_eq!(vector_data.point_domain.ids().len(), 4);
	let bézier_paths = vector_data.segment_bezier_iter().map(|(_, bézier, _, _)| bézier).collect::<Vec<_>>();
	assert_eq!(bézier_paths.len(), 4);
	assert!(bézier_paths.iter().all(|&bézier| circle.iter().any(|original_bézier| original_bézier == bézier)));

	let generated = vector_data.stroke_bezier_paths().collect::<Vec<_>>();
	assert_subpath_eq(&generated, &[circle]);
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
	assert_subpath_eq(&generated, &[subpath]);
}

#[test]
fn construct_many_subpath() {
	let curve = bezier_rs::Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::NEG_ONE, DVec2::ONE, DVec2::X);
	let curve = bezier_rs::Subpath::from_bezier(&curve);
	let circle = bezier_rs::Subpath::new_ellipse(DVec2::NEG_ONE, DVec2::ONE);

	let vector_data = VectorData::from_subpaths([&curve, &circle], false);
	assert_eq!(vector_data.point_domain.ids().len(), 6);

	let bézier_paths = vector_data.segment_bezier_iter().map(|(_, bézier, _, _)| bézier).collect::<Vec<_>>();
	assert_eq!(bézier_paths.len(), 5);
	assert!(bézier_paths.iter().all(|&bézier| circle.iter().chain(curve.iter()).any(|original_bézier| original_bézier == bézier)));

	let generated = vector_data.stroke_bezier_paths().collect::<Vec<_>>();
	assert_subpath_eq(&generated, &[curve, circle]);
}
