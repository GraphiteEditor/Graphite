use super::misc::dvec2_to_point;
use super::style::{Stroke, StrokeAlign, StrokeCap, StrokeJoin};
use super::vector_attributes::*;
use crate::vector::misc::{BezierHandles, ManipulatorGroup};
use crate::vector::misc::{HandleId, ManipulatorPointId};
use crate::vector::vector_modification::VectorExt;
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::render_complexity::RenderComplexity;
use dyn_any::StaticType;
use glam::{DAffine2, DVec2};
use kurbo::{Affine, BezPath, Rect, Shape};
use std::collections::HashMap;

/// Represents vector graphics data, composed of Bézier curves in a path or mesh arrangement.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vector {
	/// A list of all manipulator groups (referenced in `subpaths`) that have colinear handles (where they're locked at 180° angles from one another).
	/// This gets read in `graph_operation_message_handler.rs` by calling `inputs.as_mut_slice()` (search for the string `"Shape does not have both `subpath` and `colinear_manipulators` inputs"` to find it).
	pub colinear_manipulators: Vec<[HandleId; 2]>,

	pub point_domain: PointDomain,
	pub segment_domain: SegmentDomain,
}
unsafe impl StaticType for Vector {
	type Static = Self;
}

impl Default for Vector {
	fn default() -> Self {
		Self {
			colinear_manipulators: Vec::new(),
			point_domain: PointDomain::new(),
			segment_domain: SegmentDomain::new(),
		}
	}
}

impl graphene_hash::CacheHash for Vector {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.point_domain.cache_hash(state);
		self.segment_domain.cache_hash(state);
		self.colinear_manipulators.cache_hash(state);
	}
}

impl core_types::ops::FromAnchorPosition for Vector {
	fn from_anchor_position(position: DVec2) -> Self {
		let mut point_domain = PointDomain::new();
		point_domain.push(PointId::ZERO, position);

		Self { point_domain, ..Default::default() }
	}
}

// Lets a position wire feed a ranked vector connector through the input adapter's element conversion
impl From<DVec2> for Vector {
	fn from(position: DVec2) -> Self {
		<Self as core_types::ops::FromAnchorPosition>::from_anchor_position(position)
	}
}

impl core_types::transform::BakeTransform for Vector {
	fn bake_transform(&mut self, transform: &glam::DAffine2) {
		for (_, point) in self.point_domain.positions_mut() {
			*point = transform.transform_point2(*point);
		}
		self.segment_domain.transform(*transform);
	}
}

impl Vector {
	/// Add a path of manipulator groups to this vector path.
	pub fn append_manipulator_groups(&mut self, manipulator_groups: &[ManipulatorGroup], closed: bool, preserve_id: bool) {
		let mut point_id = self.point_domain.next_id();

		let handles = |a: &ManipulatorGroup, b: &ManipulatorGroup| match (a.out_handle, b.in_handle) {
			(None, None) => BezierHandles::Linear,
			(Some(handle), None) | (None, Some(handle)) => BezierHandles::Quadratic { handle },
			(Some(handle_start), Some(handle_end)) => BezierHandles::Cubic { handle_start, handle_end },
		};
		let mut segment_id = self.segment_domain.next_id();
		let mut last_point = None;
		let mut first_point = None;

		// Construct a bezier segment from the two manipulators on the subpath.
		for pair in manipulator_groups.windows(2) {
			let start = last_point.unwrap_or_else(|| {
				let id = if preserve_id && !self.point_domain.ids().contains(&pair[0].id) {
					pair[0].id
				} else {
					point_id.next_id()
				};
				self.point_domain.push(id, pair[0].anchor);
				self.point_domain.ids().len() - 1
			});
			first_point = Some(first_point.unwrap_or(start));
			let end = if preserve_id && !self.point_domain.ids().contains(&pair[1].id) {
				pair[1].id
			} else {
				point_id.next_id()
			};
			let end_index = self.point_domain.ids().len();
			self.point_domain.push(end, pair[1].anchor);

			let id = segment_id.next_id();
			self.segment_domain.push(id, start, end_index, handles(&pair[0], &pair[1]));

			last_point = Some(end_index);
		}

		if closed && let (Some(last), Some(first), Some(first_id), Some(last_id)) = (manipulator_groups.last(), manipulator_groups.first(), first_point, last_point) {
			let id = segment_id.next_id();
			self.segment_domain.push(id, last_id, first_id, handles(last, first));
		}
	}

	/// Construct some new vector path from a single [`BezPath`] with an identity transform and black fill.
	pub fn from_bezpath(bezpath: BezPath) -> Self {
		let mut vector = Self::default();
		vector.append_bezpath(bezpath);
		vector
	}

	/// Compute the bounding boxes of the bezpaths without any transform
	pub fn bounding_box_rect(&self) -> Option<Rect> {
		self.bounding_box_with_transform_rect(DAffine2::IDENTITY)
	}

	pub fn close_subpaths(&mut self) {
		let segments_to_add: Vec<_> = self
			.build_stroke_path_iter()
			.filter(|(_, closed)| !closed)
			.filter_map(|(manipulator_groups, _)| {
				let (first, last) = manipulator_groups.first().zip(manipulator_groups.last())?;
				let (start, end) = self.point_domain.resolve_id(first.id).zip(self.point_domain.resolve_id(last.id))?;
				Some((start, end))
			})
			.collect();

		for (start, end) in segments_to_add {
			let segment_id = self.segment_domain.next_id().next_id();
			self.segment_domain.push(segment_id, start, end, BezierHandles::Linear);
		}
	}

	/// Compute the bounding boxes of the subpaths without any transform
	pub fn bounding_box(&self) -> Option<[DVec2; 2]> {
		self.bounding_box_with_transform_rect(DAffine2::IDENTITY)
			.map(|rect| [DVec2::new(rect.x0, rect.y0), DVec2::new(rect.x1, rect.y1)])
	}

	/// Compute the bounding boxes of the subpaths with the specified transform
	pub fn bounding_box_with_transform(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.bounding_box_with_transform_rect(transform)
			.map(|rect| [DVec2::new(rect.x0, rect.y0), DVec2::new(rect.x1, rect.y1)])
	}

	/// Compute the bounding boxes of the bezpaths with the specified transform
	fn bounding_box_with_transform_rect(&self, transform: DAffine2) -> Option<Rect> {
		let combine = |r1: Rect, r2: Rect| r1.union(r2);
		self.stroke_bezpath_iter()
			.map(|mut bezpath| {
				bezpath.apply_affine(Affine::new(transform.to_cols_array()));
				bezpath.bounding_box()
			})
			.reduce(combine)
	}

	/// Tight bounding box in the supplied transform space, including the actual rendered stroke geometry
	/// (caps, joins, miter extensions, dashes). Runs `kurbo::stroke` on each subpath and unions the
	/// resulting fills' bounding boxes.
	///
	/// Kurbo doesn't natively support stroke alignment — that's a renderer-side compositing trick that
	/// only applies to closed paths. For closed paths with non-Center align, we use the `effective_width`
	/// identity (`Inside` = 0, `Outside` = 2×weight): the renderer masks half of a centered double-width
	/// stroke, so its AABB matches the unmasked centered stroke's. For open paths the renderer always
	/// draws a centered `weight`-wide stroke regardless of the align attribute, so we mirror that here.
	pub fn stroke_inclusive_bounding_box_with_transform(&self, transform: DAffine2, stroke: Option<&Stroke>) -> Option<[DVec2; 2]> {
		let path_bounds = self.bounding_box_with_transform(transform);

		let Some(stroke) = stroke else { return path_bounds };
		// Stroke alignment is only honored by the renderer when every subpath is closed; open paths fall
		// back to drawing a Center-aligned `weight`-wide stroke. Match that behavior to keep bounds in sync.
		let aligned_renders = stroke.align != StrokeAlign::Center && self.stroke_bezpath_iter().all(|path| matches!(path.elements().last(), Some(kurbo::PathEl::ClosePath)));
		let kurbo_width = if aligned_renders { stroke.effective_width() } else { stroke.weight };
		// `Inside`-aligned strokes never expand beyond the path bounds; a zero-weight stroke is invisible
		if kurbo_width <= 0. {
			return path_bounds;
		}

		let join = match stroke.join {
			StrokeJoin::Miter => kurbo::Join::Miter,
			StrokeJoin::Bevel => kurbo::Join::Bevel,
			StrokeJoin::Round => kurbo::Join::Round,
		};
		let cap = match stroke.cap {
			StrokeCap::Butt => kurbo::Cap::Butt,
			StrokeCap::Round => kurbo::Cap::Round,
			StrokeCap::Square => kurbo::Cap::Square,
		};

		let stroke_style = kurbo::Stroke::new(kurbo_width)
			.with_caps(cap)
			.with_join(join)
			.with_dashes(stroke.dash_offset, stroke.dash_lengths.clone())
			.with_miter_limit(stroke.join_miter_limit);
		let stroke_options = kurbo::StrokeOpts::default();
		// Same tolerance as `solidify_stroke` — balanced between performance and curve accuracy
		const STROKE_TOLERANCE: f64 = 0.25;

		let stroke_local = Affine::new(stroke.transform.to_cols_array());
		let stroke_local_inverse = (stroke.transform.matrix2.determinant() != 0.).then(|| Affine::new(stroke.transform.inverse().to_cols_array()));
		let final_transform = Affine::new(transform.to_cols_array());

		let stroke_bounds = self
			.stroke_bezpath_iter()
			.map(|mut bezpath| {
				// Match `solidify_stroke`'s pre/post-stroke transform pair so non-uniform `stroke.transform`
				// (e.g. resisting layer scale) is applied while kurbo strokes, then undone before placing the
				// stroked geometry back in the vector's local space
				bezpath.apply_affine(stroke_local);
				let mut stroked = kurbo::stroke(bezpath, &stroke_style, &stroke_options, STROKE_TOLERANCE);
				if let Some(inverse) = stroke_local_inverse {
					stroked.apply_affine(inverse);
				}
				stroked.apply_affine(final_transform);
				stroked.bounding_box()
			})
			.reduce(|a, b| a.union(b))
			.map(|rect| [DVec2::new(rect.x0, rect.y0), DVec2::new(rect.x1, rect.y1)]);

		// Union with the path bounds in case a degenerate subpath (e.g. a single point) produced no stroke geometry
		match (path_bounds, stroke_bounds) {
			(Some([min_a, max_a]), Some([min_b, max_b])) => Some([min_a.min(min_b), max_a.max(max_b)]),
			(Some(b), None) | (None, Some(b)) => Some(b),
			(None, None) => None,
		}
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

	pub fn start_point(&self) -> impl Iterator<Item = PointId> + '_ {
		self.segment_domain.start_point().iter().map(|&index| self.point_domain.ids()[index])
	}

	pub fn end_point(&self) -> impl Iterator<Item = PointId> + '_ {
		self.segment_domain.end_point().iter().map(|&index| self.point_domain.ids()[index])
	}

	pub fn push(&mut self, id: SegmentId, start: PointId, end: PointId, handles: (Option<DVec2>, Option<DVec2>)) {
		let [Some(start), Some(end)] = [start, end].map(|id| self.point_domain.resolve_id(id)) else {
			return;
		};
		let handles = match handles {
			(None, None) => BezierHandles::Linear,
			(None, Some(handle)) | (Some(handle), None) => BezierHandles::Quadratic { handle },
			(Some(handle_start), Some(handle_end)) => BezierHandles::Cubic { handle_start, handle_end },
		};
		self.segment_domain.push(id, start, end, handles)
	}

	pub fn handles_mut(&mut self) -> impl Iterator<Item = (SegmentId, &mut BezierHandles, PointId, PointId)> {
		self.segment_domain
			.handles_mut()
			.map(|(id, handles, start, end)| (id, handles, self.point_domain.ids()[start], self.point_domain.ids()[end]))
	}

	pub fn segment_start_from_id(&self, segment: SegmentId) -> Option<PointId> {
		self.segment_domain.segment_start_from_id(segment).map(|index| self.point_domain.ids()[index])
	}

	pub fn segment_end_from_id(&self, segment: SegmentId) -> Option<PointId> {
		self.segment_domain.segment_end_from_id(segment).map(|index| self.point_domain.ids()[index])
	}

	/// Attempts to find another point in the segment that is not the one passed in.
	pub fn other_point(&self, segment: SegmentId, current: PointId) -> Option<PointId> {
		let index = self.point_domain.resolve_id(current);
		index.and_then(|index| self.segment_domain.other_point(segment, index)).map(|index| self.point_domain.ids()[index])
	}

	/// Gets all points connected to the current one but not including the current one.
	pub fn connected_points(&self, current: PointId) -> impl Iterator<Item = PointId> + '_ {
		let index = [self.point_domain.resolve_id(current)].into_iter().flatten();
		index.flat_map(|index| self.segment_domain.connected_points(index).map(|index| self.point_domain.ids()[index]))
	}

	/// Returns the number of linear segments connected to the given point.
	pub fn connected_linear_segments(&self, point_id: PointId) -> usize {
		self.segment_iter()
			.filter(|(_, segment, start, end)| (*start == point_id || *end == point_id) && matches!(segment, kurbo::PathSeg::Line(_)))
			.count()
	}

	/// Get an array slice of all segment IDs.
	pub fn segment_ids(&self) -> &[SegmentId] {
		self.segment_domain.ids()
	}

	/// Enumerate all segments that start at the point.
	pub fn start_connected(&self, point: PointId) -> impl Iterator<Item = SegmentId> + '_ {
		let index = [self.point_domain.resolve_id(point)].into_iter().flatten();
		index.flat_map(|index| self.segment_domain.start_connected(index))
	}

	/// Enumerate all segments that end at the point.
	pub fn end_connected(&self, point: PointId) -> impl Iterator<Item = SegmentId> + '_ {
		let index = [self.point_domain.resolve_id(point)].into_iter().flatten();
		index.flat_map(|index| self.segment_domain.end_connected(index))
	}

	/// Enumerate all segments that start or end at a point, converting them to [`HandleId`s]. Note that the handles may not exist e.g. for a linear segment.
	pub fn all_connected(&self, point: PointId) -> impl Iterator<Item = HandleId> + '_ {
		let index = [self.point_domain.resolve_id(point)].into_iter().flatten();
		index.flat_map(|index| self.segment_domain.all_connected(index))
	}

	/// Enumerate the number of segments connected to a point. If a segment starts and ends at a point then it is counted twice.
	pub fn connected_count(&self, point: PointId) -> usize {
		self.point_domain.resolve_id(point).map_or(0, |point| self.segment_domain.connected_count(point))
	}

	/// Enumerate the number of segments connected to a point. If a segment starts and ends at a point then it is counted twice.
	pub fn any_connected(&self, point: PointId) -> bool {
		self.point_domain.resolve_id(point).is_some_and(|point| self.segment_domain.any_connected(point))
	}

	pub fn check_point_inside_shape(&self, transform: DAffine2, point: DVec2) -> bool {
		let number = self
			.stroke_bezpath_iter()
			.map(|mut bezpath| {
				// TODO: apply transform to points instead of modifying the paths
				bezpath.apply_affine(Affine::new(transform.to_cols_array()));
				bezpath.close_path();
				let bbox = bezpath.bounding_box();
				(bezpath, bbox)
			})
			.filter(|(_, bbox)| bbox.contains(dvec2_to_point(point)))
			.map(|(bezpath, _)| bezpath.winding(dvec2_to_point(point)))
			.sum::<i32>();

		// Non-zero fill rule
		number != 0
	}

	/// Iterator over all anchor points.
	pub fn anchor_points(&self) -> impl Iterator<Item = PointId> + '_ {
		self.point_domain.ids().iter().copied()
	}

	/// Anchor points at the ends of open subpaths. These are points with exactly one connection by a segment to another anchor.
	pub fn anchor_endpoints(&self) -> impl Iterator<Item = PointId> + '_ {
		// O(points + segments): tally every point's connections in a single pass
		let mut connected_counts = vec![0_usize; self.point_domain.ids().len()];
		for &point_index in self.segment_domain.start_point().iter().chain(self.segment_domain.end_point()) {
			if let Some(count) = connected_counts.get_mut(point_index) {
				*count += 1;
			}
		}

		self.anchor_points().zip(connected_counts).filter(|&(_, count)| count == 1).map(|(id, _)| id)
	}

	/// Computes if all the connected handles are colinear for an anchor, or if that handle is colinear for a handle.
	pub fn colinear(&self, point: ManipulatorPointId) -> bool {
		let has_handle = |target| self.colinear_manipulators.iter().flatten().any(|&handle| handle == target);
		match point {
			ManipulatorPointId::Anchor(id) => {
				self.start_connected(id).all(|segment| has_handle(HandleId::primary(segment))) && self.end_connected(id).all(|segment| has_handle(HandleId::end(segment)))
			}
			ManipulatorPointId::PrimaryHandle(segment) => has_handle(HandleId::primary(segment)),
			ManipulatorPointId::EndHandle(segment) => has_handle(HandleId::end(segment)),
		}
	}

	pub fn other_colinear_handle(&self, handle: HandleId) -> Option<HandleId> {
		let pair = self.colinear_manipulators.iter().find(|pair| pair.contains(&handle))?;
		let other = pair.iter().copied().find(|&val| val != handle)?;
		if handle.to_manipulator_point().get_anchor(self) == other.to_manipulator_point().get_anchor(self) {
			Some(other)
		} else {
			None
		}
	}

	pub fn adjacent_segment(&self, manipulator_id: &ManipulatorPointId) -> Option<(PointId, SegmentId)> {
		match manipulator_id {
			ManipulatorPointId::PrimaryHandle(segment_id) => {
				// For start handle, find segments ending at our start point
				let (start_point_id, _, _) = self.segment_points_from_id(*segment_id)?;
				let start_index = self.point_domain.resolve_id(start_point_id)?;

				self.segment_domain.end_connected(start_index).find(|&id| id != *segment_id).map(|id| (start_point_id, id)).or(self
					.segment_domain
					.start_connected(start_index)
					.find(|&id| id != *segment_id)
					.map(|id| (start_point_id, id)))
			}
			ManipulatorPointId::EndHandle(segment_id) => {
				// For end handle, find segments starting at our end point
				let (_, end_point_id, _) = self.segment_points_from_id(*segment_id)?;
				let end_index = self.point_domain.resolve_id(end_point_id)?;

				self.segment_domain.start_connected(end_index).find(|&id| id != *segment_id).map(|id| (end_point_id, id)).or(self
					.segment_domain
					.end_connected(end_index)
					.find(|&id| id != *segment_id)
					.map(|id| (end_point_id, id)))
			}
			ManipulatorPointId::Anchor(_) => None,
		}
	}

	pub fn concat(&mut self, additional: &Self, transform_of_additional: DAffine2, collision_hash_seed: u64) {
		let point_map = additional
			.point_domain
			.ids()
			.iter()
			.filter(|id| self.point_domain.ids().contains(id))
			.map(|&old| (old, old.generate_from_hash(collision_hash_seed)))
			.collect::<HashMap<_, _>>();

		let segment_map = additional
			.segment_domain
			.ids()
			.iter()
			.filter(|id| self.segment_domain.ids().contains(id))
			.map(|&old| (old, old.generate_from_hash(collision_hash_seed)))
			.collect::<HashMap<_, _>>();

		let id_map = IdMap {
			point_offset: self.point_domain.ids().len(),
			point_map,
			segment_map,
		};

		self.point_domain.concat(&additional.point_domain, transform_of_additional, &id_map);
		self.segment_domain.concat(&additional.segment_domain, transform_of_additional, &id_map);

		self.colinear_manipulators.extend(additional.colinear_manipulators.iter().copied());
	}
}

// The element sees only geometry; stroke inflation is applied at the row level by `vector_list_bounding_box`
// in graphic-types, which can read the appearance attribute the stroke parameters live on.
impl BoundingBox for Vector {
	fn bounding_box(&self, transform: DAffine2, _include_stroke: bool) -> RenderBoundingBox {
		match self.bounding_box_with_transform(transform) {
			Some(bounds) => RenderBoundingBox::Rectangle(bounds),
			None => RenderBoundingBox::None,
		}
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		BoundingBox::bounding_box(self, transform, include_stroke)
	}
}

impl RenderComplexity for Vector {
	fn render_complexity(&self) -> usize {
		self.segment_domain.ids().len()
	}
}

// Note: BoundingBox for List<Vector> is handled by blanket impl in gcore

#[cfg(test)]
mod tests {
	use crate::vector::algorithms::shapes::ellipse_bezpath;
	use kurbo::{CubicBez, PathSeg, Point};

	use super::*;

	fn open_curve_bezpath() -> BezPath {
		let mut bezpath = BezPath::new();
		bezpath.move_to(Point::ZERO);
		bezpath.curve_to(Point::new(-1., -1.), Point::new(1., 1.), Point::new(1., 0.));
		bezpath
	}

	#[test]
	fn construct_closed_path() {
		let circle = ellipse_bezpath(DVec2::NEG_ONE, DVec2::ONE);
		let vector = Vector::from_bezpath(circle.clone());
		assert_eq!(vector.point_domain.ids().len(), 4);

		let segments = vector.segment_iter().map(|(_, bezier, _, _)| bezier).collect::<Vec<_>>();
		assert_eq!(segments.len(), 4);
		assert!(segments.iter().all(|&segment| circle.segments().any(|original| original == segment)));

		let generated = vector.stroke_bezpath_iter().collect::<Vec<_>>();
		assert_eq!(generated.len(), 1);
		assert_eq!(generated[0].elements(), circle.elements());
	}

	#[test]
	fn construct_open_path() {
		let curve = open_curve_bezpath();
		let vector = Vector::from_bezpath(curve.clone());
		assert_eq!(vector.point_domain.ids().len(), 2);

		let segments = vector.segment_iter().map(|(_, bezier, _, _)| bezier).collect::<Vec<_>>();
		assert_eq!(segments, vec![PathSeg::Cubic(CubicBez::new(Point::ZERO, Point::new(-1., -1.), Point::new(1., 1.), Point::new(1., 0.)))]);

		let generated = vector.stroke_manipulator_groups().collect::<Vec<_>>();
		assert_eq!(generated.len(), 1);
		let (groups, closed) = &generated[0];
		assert!(!closed);
		assert_eq!(groups.len(), 2);
		assert_eq!((groups[0].anchor, groups[0].in_handle, groups[0].out_handle), (DVec2::ZERO, None, Some(DVec2::new(-1., -1.))));
		assert_eq!((groups[1].anchor, groups[1].in_handle, groups[1].out_handle), (DVec2::new(1., 0.), Some(DVec2::new(1., 1.)), None));
	}

	#[test]
	fn construct_many_paths() {
		let curve = open_curve_bezpath();
		let circle = ellipse_bezpath(DVec2::NEG_ONE, DVec2::ONE);

		let mut vector = Vector::from_bezpath(curve.clone());
		vector.append_bezpath(circle.clone());
		assert_eq!(vector.point_domain.ids().len(), 6);

		let segments = vector.segment_iter().map(|(_, bezier, _, _)| bezier).collect::<Vec<_>>();
		assert_eq!(segments.len(), 5);
		assert!(segments.iter().all(|&segment| circle.segments().chain(curve.segments()).any(|original| original == segment)));

		let generated = vector.stroke_bezpath_iter().collect::<Vec<_>>();
		assert_eq!(generated.len(), 2);
		assert_eq!(generated[0].elements(), curve.elements());
		assert_eq!(generated[1].elements(), circle.elements());
	}

	// Verifies the `DVec2 -> List<Vector>` conversion that replaced the former "Vec2 to Point" node yields a path
	// with exactly one anchor point at the given position and no segments
	#[test]
	fn anchor_position_builds_single_point_path() {
		use core_types::ops::FromAnchorPosition;

		let vector = Vector::from_anchor_position(DVec2::new(3., 4.));

		assert_eq!(vector.point_domain.positions(), [DVec2::new(3., 4.)]);
		assert_eq!(vector.point_domain.ids(), [PointId::ZERO], "expected the anchor position to be assigned the default PointId::ZERO",);
		assert!(vector.segment_domain.ids().is_empty());
	}
}
