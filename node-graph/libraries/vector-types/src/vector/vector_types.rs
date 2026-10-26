use super::addressing;
use super::algorithms::consts::CLOSE_POINT_TOLERANCE;
use super::connectivity::Connectivity;
use super::contour::{Contour, contour_bezpaths, contours};
use super::geometry_cache::{GeometryCache, SetVectorPaths, set_paths_at};
use super::misc::{HandleId, ManipulatorPointId, dvec2_to_point};
use super::path::{
	CurveType, Path, append_named_bezpaths, curve_type_at, curve_types_of, no_paths, paths_from_item_attributes, paths_from_list_row_attributes, push_path, shared_paths_from_item_attributes,
};
use super::style::{Stroke, StrokeAlign, StrokeCap, StrokeJoin};
use super::vector_attributes::{PointId, SegmentId};
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::list::{ATTR_GEOMETRY_CACHE, ATTR_ID, ATTR_PATHS, ATTR_SMOOTH, Item, List};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use kurbo::{Affine, BezPath, PathSeg, Rect, Shape};

/// Vector graphics: a pool of control points, anchors and handles alike, that paths weave into curves and meshes.
/// The weave rides the holder as a `List<Path>` under `ATTR_PATHS`, and per-point data such as the stable `ATTR_ID` rides this pool's own attributes.
#[derive(Clone, Debug, Default, PartialEq, graphene_hash::CacheHash, DynAny)]
pub struct Vector(pub List<DVec2>);

impl core_types::ops::FromAnchorPosition for Vector {
	fn from_anchor_position(position: DVec2) -> Self {
		let mut pool = List::new_from_element(position);
		// A lone anchor is named zero, the ID stored document edits address it by
		pool.set_attribute(ATTR_ID, 0, PointId::ZERO);

		Self(pool)
	}
}

// Lets a position wire feed a ranked vector connector through the input adapter's element conversion
impl From<DVec2> for Vector {
	fn from(position: DVec2) -> Self {
		<Self as core_types::ops::FromAnchorPosition>::from_anchor_position(position)
	}
}

// Handles are pool points, so transforming the pool carries the whole geometry
impl core_types::transform::BakeTransform for Vector {
	fn bake_transform(&mut self, transform: &DAffine2) {
		self.transform(*transform);
	}
}

impl Vector {
	/// The number of points in the pool, counting anchors and handles alike.
	pub fn point_count(&self) -> usize {
		self.0.len()
	}

	/// The pool position at the given index.
	pub fn position(&self, index: usize) -> Option<DVec2> {
		self.0.element(index).copied()
	}

	/// Every pool position in index order.
	pub fn positions(&self) -> impl Iterator<Item = DVec2> + '_ {
		self.0.iter_element_values().copied()
	}

	/// The stored ID of the pool point at the given index, `PointId::UNNAMED` for a handle.
	pub fn point_id_at(&self, index: u32) -> Option<PointId> {
		addressing::point_id_at(&self.0, index)
	}

	/// The pool index a point ID names, by scanning the ID attribute in O(points).
	pub fn index_of_point_id(&self, point: PointId) -> Option<u32> {
		if point == PointId::UNNAMED {
			return None;
		}

		addressing::index_of_point_id(&self.0, point)
	}

	/// Applies an affine transform to every pool point.
	pub fn transform(&mut self, transform: DAffine2) {
		for position in self.0.iter_element_values_mut() {
			*position = transform.transform_point2(*position);
		}
	}

	/// The reverse index answering which paths meet at each pool point, built in O(points + references).
	pub fn connectivity(&self, paths: &List<Path>) -> Connectivity {
		Connectivity::build(self.0.len(), paths)
	}
}

/// The largest ID in the pool's point ID sequence, which a pool has once it holds point IDs at all.
pub(crate) fn largest_point_id(pool: &List<DVec2>) -> Option<PointId> {
	let ids = pool.iter_attribute_values::<PointId>(ATTR_ID)?;

	// An ID attribute that names nothing yet counts as a sequence at zero, which documents saved with edits to such geometry rely on
	Some(ids.copied().filter(|id| *id != PointId::UNNAMED).max().unwrap_or_default())
}

/// The largest ID in the paths' segment ID sequence.
fn largest_segment_id(paths: &List<Path>) -> Option<SegmentId> {
	let ids = paths.iter_element_values().filter_map(|path| path.0.iter_attribute_values::<SegmentId>(ATTR_ID));

	ids.flatten().copied().max()
}

impl Vector {
	/// Weaves a bezpath's contours onto this pool and the given paths, naming the new anchors and spans
	/// by continuing the ID sequences. Handles take `PointId::UNNAMED`.
	pub fn append_bezpath(&mut self, paths: &mut List<Path>, bezpath: &BezPath) {
		self.append_bezpaths(paths, [bezpath]);
	}

	/// Weaves each bezpath in turn as [`Self::append_bezpath`] would, in O(existing + appended) however many there are
	/// since the IDs to continue from are found once.
	pub fn append_bezpaths(&mut self, paths: &mut List<Path>, bezpaths: impl IntoIterator<Item = impl std::borrow::Borrow<BezPath>>) {
		self.append_bezpaths_counting_spans(paths, bezpaths);
	}

	/// Weaves bezpaths as [`Self::append_bezpaths`] does, returning how many spans each of them added.
	pub fn append_bezpaths_counting_spans(&mut self, paths: &mut List<Path>, bezpaths: impl IntoIterator<Item = impl std::borrow::Borrow<BezPath>>) -> Vec<usize> {
		let (last_point, last_segment) = (largest_point_id(&self.0), largest_segment_id(paths));
		append_named_bezpaths(&mut self.0, paths, bezpaths, last_point, last_segment, false)
	}

	/// Builds a fresh pool and paths from a single bezpath.
	pub fn from_bezpath(bezpath: &BezPath) -> (Self, List<Path>) {
		let (mut vector, mut paths) = (Self::default(), List::new());
		vector.append_bezpath(&mut paths, bezpath);

		(vector, paths)
	}

	/// Builds a fresh pool and paths from a single bezpath like [`Self::from_bezpath`], but with anchors at exactly the
	/// same position woven as one point, welding the contours that meet there.
	pub fn from_bezpath_welded(bezpath: &BezPath) -> (Self, List<Path>) {
		let (mut vector, mut paths) = (Self::default(), List::new());
		append_named_bezpaths(&mut vector.0, &mut paths, [bezpath], None, None, true);

		(vector, paths)
	}

	/// Builds a holder item from a single bezpath, its paths riding the item's attributes.
	pub fn item_from_bezpath(bezpath: &BezPath) -> Item<Self> {
		let (vector, paths) = Self::from_bezpath(bezpath);

		Item::new_from_element(vector).with_paths(paths)
	}
}

impl Vector {
	/// Every segment in span order as `(id, segment, start point ID, end point ID)`.
	pub fn segment_iter<'a>(&'a self, paths: &'a List<Path>) -> impl Iterator<Item = (SegmentId, PathSeg, PointId, PointId)> + 'a {
		let point_ids = self.0.iter_attribute_values::<PointId>(ATTR_ID).map(|ids| ids.as_slice()).unwrap_or_default();
		let point_id = move |point: u32| point_ids.get(point as usize).copied().unwrap_or_default();

		addressing::resolved_spans(paths).filter_map(move |resolved| {
			let segment = resolved.path.segment(&self.0, resolved.curve_type, resolved.span.index as usize)?;
			let (start, end) = resolved.endpoints;

			Some((resolved.id.unwrap_or_default(), segment, point_id(start), point_id(end)))
		})
	}

	/// The segment a segment ID names, resolved against the pool.
	pub fn segment_from_id(&self, paths: &List<Path>, id: SegmentId) -> Option<PathSeg> {
		let span = addressing::span_of_segment_id(paths, id)?;
		paths.element(span.path as usize)?.segment(&self.0, curve_type_at(paths, span.path as usize), span.index as usize)
	}

	/// The endpoints and geometry of the segment a segment ID names.
	pub fn segment_points_from_id(&self, paths: &List<Path>, id: SegmentId) -> Option<(PointId, PointId, PathSeg)> {
		let span = addressing::span_of_segment_id(paths, id)?;
		let (start, end) = addressing::span_endpoints(paths, span)?;
		let segment = paths.element(span.path as usize)?.segment(&self.0, curve_type_at(paths, span.path as usize), span.index as usize)?;

		Some((self.point_id_at(start).unwrap_or_default(), self.point_id_at(end).unwrap_or_default(), segment))
	}

	/// The segment a segment ID names, paired with the pool indices of its endpoints.
	pub fn segment_with_pool_endpoints(&self, paths: &List<Path>, id: SegmentId) -> Option<(PathSeg, usize, usize)> {
		let span = addressing::span_of_segment_id(paths, id)?;
		let (start, end) = addressing::span_endpoints(paths, span)?;
		let segment = paths.element(span.path as usize)?.segment(&self.0, curve_type_at(paths, span.path as usize), span.index as usize)?;

		Some((segment, start as usize, end as usize))
	}

	/// The ID of the point the segment starts at.
	pub fn segment_start_from_id(&self, paths: &List<Path>, segment: SegmentId) -> Option<PointId> {
		let span = addressing::span_of_segment_id(paths, segment)?;
		self.point_id_at(addressing::span_endpoints(paths, span)?.0)
	}

	/// The ID of the point the segment ends at.
	pub fn segment_end_from_id(&self, paths: &List<Path>, segment: SegmentId) -> Option<PointId> {
		let span = addressing::span_of_segment_id(paths, segment)?;
		self.point_id_at(addressing::span_endpoints(paths, span)?.1)
	}

	/// Each segment's start point ID, in span order.
	pub fn start_point<'a>(&'a self, paths: &'a List<Path>) -> impl Iterator<Item = PointId> + 'a {
		addressing::spans(paths).filter_map(|span| self.point_id_at(addressing::span_endpoints(paths, span)?.0))
	}

	/// Each segment's end point ID, in span order.
	pub fn end_point<'a>(&'a self, paths: &'a List<Path>) -> impl Iterator<Item = PointId> + 'a {
		addressing::spans(paths).filter_map(|span| self.point_id_at(addressing::span_endpoints(paths, span)?.1))
	}

	/// The point at the segment's other end from the given one.
	pub fn other_point(&self, paths: &List<Path>, segment: SegmentId, current: PointId) -> Option<PointId> {
		let (start, end, _) = self.segment_points_from_id(paths, segment)?;

		match current {
			_ if start == end => None,
			point if point == start => Some(end),
			point if point == end => Some(start),
			_ => None,
		}
	}

	/// Emits the stitched contours, one bezpath each, in the order the paths are stored.
	pub fn stroke_bezpath_iter(&self, paths: &List<Path>) -> impl Iterator<Item = BezPath> {
		contour_bezpaths(&self.0, paths).into_iter()
	}
}

impl Vector {
	/// Compute the bounding box of the contours without any transform.
	pub fn bounding_box(&self, paths: &List<Path>) -> Option<[DVec2; 2]> {
		self.bounding_box_with_transform(paths, DAffine2::IDENTITY)
	}

	/// Compute the bounding box of the contours without any transform.
	pub fn bounding_box_rect(&self, paths: &List<Path>) -> Option<Rect> {
		self.bounding_box_with_transform_rect(paths, DAffine2::IDENTITY)
	}

	/// Compute the bounding box of the contours with the specified transform.
	pub fn bounding_box_with_transform(&self, paths: &List<Path>, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.bounding_box_with_transform_rect(paths, transform)
			.map(|rect| [DVec2::new(rect.x0, rect.y0), DVec2::new(rect.x1, rect.y1)])
	}

	fn bounding_box_with_transform_rect(&self, paths: &List<Path>, transform: DAffine2) -> Option<Rect> {
		contours_bounding_rect(&contour_bezpaths(&self.0, paths), transform)
	}

	/// Tight bounding box in the supplied transform space, including the actual rendered stroke geometry
	/// (caps, joins, miter extensions, dashes). Runs `kurbo::stroke` on each contour and unions the
	/// resulting fills' bounding boxes.
	///
	/// Kurbo doesn't natively support stroke alignment, a renderer-side compositing trick that
	/// only applies to closed paths. For closed paths with non-Center align, we use the `effective_width`
	/// identity (`Inside` = 0, `Outside` = 2×weight): the renderer masks half of a centered double-width
	/// stroke, so its AABB matches the unmasked centered stroke's. For open paths the renderer always
	/// draws a centered `weight`-wide stroke regardless of the align attribute, so we mirror that here.
	pub fn stroke_inclusive_bounding_box_with_transform(&self, paths: &List<Path>, transform: DAffine2, stroke: Option<&Stroke>) -> Option<[DVec2; 2]> {
		contours_stroke_inclusive_bounding_box(&contour_bezpaths(&self.0, paths), transform, stroke)
	}

	/// Calculate the corners of the bounding box but with a nonzero size.
	///
	/// If the layer bounds are `0` in either axis then they are changed to be `1`.
	pub fn nonzero_bounding_box(&self, paths: &List<Path>) -> [DVec2; 2] {
		let [bounds_min, mut bounds_max] = self.bounding_box(paths).unwrap_or_default();

		let bounds_size = bounds_max - bounds_min;
		if bounds_size.x < 1e-10 {
			bounds_max.x = bounds_min.x + 1.;
		}
		if bounds_size.y < 1e-10 {
			bounds_max.y = bounds_min.y + 1.;
		}

		[bounds_min, bounds_max]
	}

	pub fn check_point_inside_shape(&self, paths: &List<Path>, transform: DAffine2, point: DVec2) -> bool {
		let number = self
			.stroke_bezpath_iter(paths)
			.map(|mut bezpath| {
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
}

/// The bounding box of already extracted contours, placed by `transform`.
pub fn contours_bounding_box(contours: &[BezPath], transform: DAffine2) -> Option<[DVec2; 2]> {
	contours_bounding_rect(contours, transform).map(|rect| [DVec2::new(rect.x0, rect.y0), DVec2::new(rect.x1, rect.y1)])
}

fn contours_bounding_rect(contours: &[BezPath], transform: DAffine2) -> Option<Rect> {
	// Transforming segment by segment bounds each contour without copying it
	let affine = Affine::new(transform.to_cols_array());
	let bounds = |bezpath: &BezPath| {
		if transform == DAffine2::IDENTITY {
			return bezpath.bounding_box();
		}
		bezpath.segments().map(|segment| (affine * segment).bounding_box()).reduce(|a, b| a.union(b)).unwrap_or_default()
	};
	contours.iter().map(bounds).reduce(|r1, r2| r1.union(r2))
}

/// Like [`Vector::stroke_inclusive_bounding_box_with_transform`], for already extracted contours.
pub fn contours_stroke_inclusive_bounding_box(contours: &[BezPath], transform: DAffine2, stroke: Option<&Stroke>) -> Option<[DVec2; 2]> {
	let path_bounds = contours_bounding_box(contours, transform);

	let Some(stroke) = stroke else { return path_bounds };
	// Stroke alignment is only honored by the renderer when every contour is closed; open paths fall
	// back to drawing a Center-aligned `weight`-wide stroke. Match that behavior to keep bounds in sync.
	let aligned_renders = stroke.align != StrokeAlign::Center && contours.iter().all(|path| matches!(path.elements().last(), Some(kurbo::PathEl::ClosePath)));
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
	// Same tolerance as `solidify_stroke`, balanced between performance and curve accuracy
	const STROKE_TOLERANCE: f64 = 0.25;

	let stroke_local = Affine::new(stroke.transform.to_cols_array());
	let stroke_local_inverse = (stroke.transform.matrix2.determinant() != 0.).then(|| Affine::new(stroke.transform.inverse().to_cols_array()));
	let final_transform = Affine::new(transform.to_cols_array());

	let stroke_bounds = contours
		.iter()
		.map(|bezpath| {
			// Match `solidify_stroke`'s pre/post-stroke transform pair so non-uniform `stroke.transform`
			// (e.g. resisting layer scale) is applied while kurbo strokes, then undone before placing the
			// stroked geometry back in the vector's local space
			let mut stroked = kurbo::stroke(stroke_local * bezpath, &stroke_style, &stroke_options, STROKE_TOLERANCE);
			if let Some(inverse) = stroke_local_inverse {
				stroked.apply_affine(inverse);
			}
			stroked.apply_affine(final_transform);
			stroked.bounding_box()
		})
		.reduce(|a, b| a.union(b))
		.map(|rect| [DVec2::new(rect.x0, rect.y0), DVec2::new(rect.x1, rect.y1)]);

	// Union with the path bounds in case a degenerate contour (e.g. a single point) produced no stroke geometry
	match (path_bounds, stroke_bounds) {
		(Some([min_a, max_a]), Some([min_b, max_b])) => Some([min_a.min(min_b), max_a.max(max_b)]),
		(Some(b), None) | (None, Some(b)) => Some(b),
		(None, None) => None,
	}
}

impl Vector {
	/// Marks each pool point that paths name only at handle slots, in O(points + references).
	pub(crate) fn handle_mask(&self, paths: &List<Path>) -> Vec<bool> {
		self.point_roles(paths).0
	}

	/// The handle mask, along with whether every handle is one segment's alone: no point fills two handle slots, and none
	/// is an anchor in one place and a handle in another.
	pub(crate) fn point_roles(&self, paths: &List<Path>) -> (Vec<bool>, bool) {
		// Each point is referenced by no path, only at a handle slot, or at an anchor slot somewhere
		const UNREFERENCED: u8 = 0;
		const HANDLE: u8 = 1;
		const ANCHOR: u8 = 2;

		let mut roles = vec![UNREFERENCED; self.0.len()];
		let mut handles_unshared = true;
		for (path, curve_type) in paths.iter_element_values().zip(curve_types_of(paths)) {
			let stride = curve_type.stride();
			for (reference_index, point) in path.references().enumerate() {
				let Some(role) = roles.get_mut(point as usize) else { continue };
				if reference_index.is_multiple_of(stride) {
					handles_unshared &= *role != HANDLE;
					*role = ANCHOR;
				} else {
					handles_unshared &= *role == UNREFERENCED;
					*role = (*role).max(HANDLE);
				}
			}
		}

		(roles.into_iter().map(|role| role == HANDLE).collect(), handles_unshared)
	}

	/// The pool indices of all anchor points and free points in pool order, skipping handles.
	pub fn anchor_indices(&self, paths: &List<Path>) -> Vec<usize> {
		self.handle_mask(paths).into_iter().enumerate().filter(|&(_, handle)| !handle).map(|(index, _)| index).collect()
	}

	/// Drops the paths and every point used only as a handle, leaving the anchors and free points as a bare cloud.
	/// A point serving as an anchor in any path is kept even where another path uses it as a handle.
	/// Returns the former pool index of each kept point, in their kept order.
	pub fn keep_only_anchors(&mut self, paths: &mut List<Path>) -> Vec<usize> {
		let anchors = self.anchor_indices(paths);
		let pool = anchors.iter().filter_map(|&index| self.0.clone_item(index)).collect();

		self.0 = pool;
		*paths = List::new();

		anchors
	}

	/// The positions of all anchor points and free points in pool order, skipping handles.
	pub fn anchor_positions(&self, paths: &List<Path>) -> Vec<DVec2> {
		self.anchor_indices(paths).into_iter().filter_map(|index| self.position(index)).collect()
	}

	/// Iterator over the IDs of all anchor points and free points, skipping handles.
	pub fn anchor_points(&self, paths: &List<Path>) -> impl Iterator<Item = PointId> {
		let ids = self.0.iter_attribute_values::<PointId>(ATTR_ID).map(|ids| ids.as_slice()).unwrap_or_default();
		let anchors: Vec<_> = self.handle_mask(paths).into_iter().zip(ids).filter(|&(handle, _)| !handle).map(|(_, &id)| id).collect();

		anchors.into_iter()
	}

	/// Anchor points at the ends of open chains, where exactly one segment end meets the point.
	pub fn anchor_endpoints(&self, paths: &List<Path>) -> impl Iterator<Item = PointId> {
		let connectivity = self.connectivity(paths);
		let endpoints: Vec<_> = (0..self.0.len() as u32)
			.filter(|&index| connectivity.spokes(index) == 1)
			.filter_map(|index| self.point_id_at(index))
			.collect();

		endpoints.into_iter()
	}

	// Each per-point query below builds its own connectivity index, O(points + references) with three allocations.
	// Callers looping over many points should hoist `connectivity()` and use `addressing` directly.

	/// Enumerate all segments that start at the point.
	pub fn start_connected(&self, paths: &List<Path>, point: PointId) -> impl Iterator<Item = SegmentId> {
		let connected = self
			.index_of_point_id(point)
			.map(|index| addressing::start_connected(index, paths, &self.connectivity(paths)))
			.unwrap_or_default();

		connected.into_iter()
	}

	/// Enumerate all segments that end at the point.
	pub fn end_connected(&self, paths: &List<Path>, point: PointId) -> impl Iterator<Item = SegmentId> {
		let connected = self
			.index_of_point_id(point)
			.map(|index| addressing::end_connected(index, paths, &self.connectivity(paths)))
			.unwrap_or_default();

		connected.into_iter()
	}

	/// Enumerate all segments that start or end at a point, converting them to [`HandleId`]s. Note that the handles may not exist e.g. for a linear segment.
	pub fn all_connected(&self, paths: &List<Path>, point: PointId) -> impl Iterator<Item = HandleId> {
		let connected = self
			.index_of_point_id(point)
			.map(|index| addressing::all_connected(index, paths, &self.connectivity(paths)))
			.unwrap_or_default();

		connected.into_iter()
	}

	/// The number of segment ends meeting at a point. A segment that starts and ends there is counted twice.
	pub fn connected_count(&self, paths: &List<Path>, point: PointId) -> usize {
		let Some(index) = self.index_of_point_id(point) else { return 0 };

		self.connectivity(paths).spokes(index)
	}

	/// Whether any segment starts or ends at the point.
	pub fn any_connected(&self, paths: &List<Path>, point: PointId) -> bool {
		self.connected_count(paths, point) > 0
	}

	/// Gets all points connected to the current one but not including the current one.
	pub fn connected_points(&self, paths: &List<Path>, current: PointId) -> impl Iterator<Item = PointId> {
		let mut found = Vec::new();
		if let Some(index) = self.index_of_point_id(current) {
			for (span, arriving) in addressing::spans_at(index, paths, &self.connectivity(paths)) {
				let Some((start, end)) = addressing::span_endpoints(paths, span) else { continue };
				if start == end {
					continue;
				}

				let other = if arriving { start } else { end };
				if let Some(id) = self.point_id_at(other) {
					found.push(id);
				}
			}
		}

		found.into_iter()
	}

	/// Returns the number of linear segments connected to the given point.
	pub fn connected_linear_segments(&self, paths: &List<Path>, point_id: PointId) -> usize {
		self.segment_iter(paths)
			.filter(|(_, segment, start, end)| (*start == point_id || *end == point_id) && matches!(segment, PathSeg::Line(_)))
			.count()
	}
}

impl Vector {
	/// Whether the junction at the given anchor keeps its tangent pair smooth, read from any of its occurrences.
	fn smooth_at_anchor_index(&self, paths: &List<Path>, connectivity: &Connectivity, index: u32) -> bool {
		connectivity.references(index).iter().filter(|reference| reference.is_anchor()).any(|reference| {
			let Some(path) = paths.element(reference.path as usize) else { return false };
			path.0.attribute::<bool>(ATTR_SMOOTH, reference.index as usize).copied().unwrap_or_default()
		})
	}

	/// Whether the anchor at the given pool index forms a smooth tangent pair, which needs exactly two
	/// segment ends meeting there. A junction has no unambiguous pair to keep smooth.
	fn pairs_smoothly(&self, paths: &List<Path>, connectivity: &Connectivity, index: u32) -> bool {
		self.smooth_at_anchor_index(paths, connectivity, index) && connectivity.spokes(index) == 2
	}

	/// Whether the anchor (or the handle's anchor) keeps a smooth tangent pair, which needs exactly two segment ends meeting there.
	pub fn colinear(&self, paths: &List<Path>, point: ManipulatorPointId) -> bool {
		let connectivity = self.connectivity(paths);
		let handle_anchor_smooth = |segment: SegmentId, at_start: bool| {
			let Some(span) = addressing::span_of_segment_id(paths, segment) else { return false };
			let Some((start, end)) = addressing::span_endpoints(paths, span) else { return false };

			self.pairs_smoothly(paths, &connectivity, if at_start { start } else { end })
		};

		match point {
			ManipulatorPointId::Anchor(id) => match self.index_of_point_id(id) {
				Some(index) => self.pairs_smoothly(paths, &connectivity, index),
				None => false,
			},
			ManipulatorPointId::PrimaryHandle(segment) => handle_anchor_smooth(segment, true),
			ManipulatorPointId::EndHandle(segment) => handle_anchor_smooth(segment, false),
		}
	}

	/// Every G1 pair: the two facing handles at each smooth anchor where exactly two segment ends meet.
	pub fn colinear_pairs(&self, paths: &List<Path>) -> Vec<[HandleId; 2]> {
		self.colinear_pairs_with(paths, &self.connectivity(paths))
	}

	/// The G1 pairs, read through a connectivity the caller already built.
	pub(crate) fn colinear_pairs_with(&self, paths: &List<Path>, connectivity: &Connectivity) -> Vec<[HandleId; 2]> {
		let mut pairs = Vec::new();

		for index in 0..self.point_count() as u32 {
			if connectivity.spokes(index) != 2 || !self.smooth_at_anchor_index(paths, connectivity, index) {
				continue;
			}

			let handles: Vec<HandleId> = addressing::spans_at(index, paths, connectivity)
				.into_iter()
				.filter_map(|(span, arriving)| {
					let segment = addressing::segment_id_of(paths, span)?;
					Some(if arriving { HandleId::end(segment) } else { HandleId::primary(segment) })
				})
				.collect();
			if let [first, second] = handles.as_slice() {
				pairs.push([*first, *second]);
			}
		}

		pairs
	}

	/// The G1 pair the given handle belongs to, ordered with that handle first.
	pub fn colinear_pair_containing(&self, paths: &List<Path>, handle: HandleId) -> Option<[HandleId; 2]> {
		self.other_colinear_handle(paths, handle).map(|other| [handle, other])
	}

	/// The handle paired with the given one across their shared smooth anchor, where exactly two segment ends meet.
	pub fn other_colinear_handle(&self, paths: &List<Path>, handle: HandleId) -> Option<HandleId> {
		let span = addressing::span_of_segment_id(paths, handle.segment)?;
		let (start, end) = addressing::span_endpoints(paths, span)?;
		let anchor = if handle.ty == super::misc::HandleType::Primary { start } else { end };

		let connectivity = self.connectivity(paths);
		if !self.pairs_smoothly(paths, &connectivity, anchor) {
			return None;
		}

		addressing::spans_at(anchor, paths, &connectivity)
			.into_iter()
			.filter(|&(other, arriving)| (other, arriving) != (span, handle.ty != super::misc::HandleType::Primary))
			.find_map(|(other, arriving)| {
				let segment = addressing::segment_id_of(paths, other)?;
				Some(if arriving { HandleId::end(segment) } else { HandleId::primary(segment) })
			})
	}

	/// Sets whether the junction at the given anchor keeps its tangent pair smooth, written to every occurrence.
	pub fn set_smooth(&mut self, paths: &mut List<Path>, point: PointId, smooth: bool) {
		let Some(index) = self.index_of_point_id(point) else { return };

		self.set_smooth_at_indices(paths, [index], smooth);
	}

	/// Sets the smooth flag on many anchors at once, finding them in one pass over the pool's IDs.
	pub fn set_smooth_points(&mut self, paths: &mut List<Path>, points: impl IntoIterator<Item = PointId>, smooth: bool) {
		let points: std::collections::HashSet<PointId> = points.into_iter().collect();
		let indices: Vec<u32> = (0..self.0.len() as u32).filter(|&index| self.point_id_at(index).is_some_and(|id| points.contains(&id))).collect();

		self.set_smooth_at_indices(paths, indices, smooth);
	}

	/// Sets the smooth flag on many anchors at once, reading one connectivity rather than one per anchor.
	pub fn set_smooth_at_indices(&mut self, paths: &mut List<Path>, indices: impl IntoIterator<Item = u32>, smooth: bool) {
		let mut indices = indices.into_iter().peekable();
		if indices.peek().is_none() {
			return;
		}

		let connectivity = self.connectivity(paths);
		for index in indices {
			for reference in connectivity.references(index).iter().filter(|reference| reference.is_anchor()) {
				let Some(path) = paths.element_mut(reference.path as usize) else { continue };
				path.0.set_attribute(ATTR_SMOOTH, reference.index as usize, smooth);
			}
		}
	}

	/// The neighboring segment sharing the anchor the given handle sits on.
	pub fn adjacent_segment(&self, paths: &List<Path>, manipulator_id: &ManipulatorPointId) -> Option<(PointId, SegmentId)> {
		let (segment_id, at_start) = match manipulator_id {
			ManipulatorPointId::PrimaryHandle(segment_id) => (*segment_id, true),
			ManipulatorPointId::EndHandle(segment_id) => (*segment_id, false),
			ManipulatorPointId::Anchor(_) => return None,
		};

		let span = addressing::span_of_segment_id(paths, segment_id)?;
		let (start, end) = addressing::span_endpoints(paths, span)?;
		let anchor = if at_start { start } else { end };
		let anchor_id = self.point_id_at(anchor)?;

		// Prefer the segment meeting this one head-on, since a smooth tangent pairs across the anchor
		let occurrences = addressing::spans_at(anchor, paths, &self.connectivity(paths));
		let other = |wanted_arriving: bool| {
			occurrences.iter().find_map(|&(other, arriving)| {
				let id = addressing::segment_id_of(paths, other)?;
				(id != segment_id && arriving == wanted_arriving).then_some(id)
			})
		};

		other(at_start).or_else(|| other(!at_start)).map(|id| (anchor_id, id))
	}
}

impl Vector {
	/// Appends a lone segment between two existing points, its handles becoming new unnamed pool points.
	pub fn push(&mut self, paths: &mut List<Path>, id: SegmentId, start: PointId, end: PointId, handles: (Option<DVec2>, Option<DVec2>)) {
		let [Some(start), Some(end)] = [start, end].map(|point| self.index_of_point_id(point)) else {
			return;
		};

		let (curve_type, handle_positions) = match handles {
			(None, None) => (CurveType::Linear, Vec::new()),
			(None, Some(handle)) | (Some(handle), None) => (CurveType::Quadratic, vec![handle]),
			(Some(handle_start), Some(handle_end)) => (CurveType::Cubic, vec![handle_start, handle_end]),
		};

		let mut references = vec![start];
		for position in handle_positions {
			references.push(self.0.len() as u32);
			self.0.push(Item::new_from_element(position).with_attribute(ATTR_ID, PointId::UNNAMED));
		}
		references.push(end);

		push_path(paths, curve_type, references, [id]);
	}

	/// Welds every open contour shut with a linear segment from its last point back to its first, stored where the contour runs on into it.
	/// A contour already ending on its start, within floating point noise, merges that end into the start instead.
	pub fn close_subpaths(&mut self, paths: &mut List<Path>) {
		let mut open: Vec<Contour> = contours(self.point_count(), paths).into_iter().filter(|contour| !contour.closed).collect();

		// An end resting on its own start is that point, so it merges in rather than drawing a closing segment of no length
		let positions = self.0.iter_element_values().as_slice();
		let rests_on_start = |contour: &&Contour| matches!((positions.get(contour.entry as usize), positions.get(contour.exit as usize)), (Some(entry), Some(exit)) if entry.distance_squared(*exit) <= CLOSE_POINT_TOLERANCE * CLOSE_POINT_TOLERANCE);
		let resting: Vec<(u32, u32)> = open.iter().filter(rests_on_start).map(|contour| (contour.exit, contour.entry)).collect();
		if !resting.is_empty() {
			self.merge_points(paths, &resting);
			open = contours(self.point_count(), paths).into_iter().filter(|contour| !contour.closed).collect();
		}

		// A closing segment carries on the contour's last stored path from its exit, even at a junction. A linear path takes
		// it in as one more span, and any other is followed by a linear path that carries on its path flow.
		let curve_types: Vec<CurveType> = curve_types_of(paths).collect();
		let mut latest = largest_segment_id(paths);
		let mut closing_paths = Vec::new();
		for contour in open {
			let Some(&last) = contour.parts.last() else { continue };

			// Each closing segment's ID skips one after the latest, as the IDs of a newly appended bezpath do
			let id = latest.map_or(SegmentId::ZERO, |mut latest| latest.next_id()).next_id();
			latest = Some(id);

			if curve_types.get(last as usize) == Some(&CurveType::Linear)
				&& let Some(path) = paths.element_mut(last as usize)
			{
				path.0.set_attribute(ATTR_ID, path.0.len().saturating_sub(1), id);
				path.0.push(Item::new_from_element(i64::from(contour.entry)));
			} else {
				let mut single = List::new();
				push_path(&mut single, CurveType::Linear, [contour.exit, contour.entry], [id]);
				closing_paths.extend(single.into_iter().map(|path| (last as usize + 1, path)));
			}
		}
		if closing_paths.is_empty() {
			return;
		}

		// Contours come out in the order of their first stored paths, so their last ones can be out of order
		closing_paths.sort_by_key(|&(after, _)| after);

		// Rebuild the paths with each closing path after the one it carries on, keeping every other path in its stored order
		let mut closing_paths = closing_paths.into_iter().peekable();
		let mut rebuilt = List::new();
		for (index, path) in std::mem::take(paths).into_iter().enumerate() {
			while let Some((_, closing)) = closing_paths.next_if(|&(after, _)| after == index) {
				rebuilt.push(closing);
			}
			rebuilt.push(path);
		}
		for (_, closing) in closing_paths {
			rebuilt.push(closing);
		}

		*paths = rebuilt;
	}

	/// Merges the first point of each pair into the second, renaming every reference to it and dropping it from the pool, in O(points + references).
	fn merge_points(&mut self, paths: &mut List<Path>, merges: &[(u32, u32)]) {
		let point_count = self.0.len();

		// Each point's representative is found by following merges to a point merged into nothing, which settles chains and cycles alike
		let mut merged_into: Vec<usize> = (0..point_count).collect();
		let representative = |merged_into: &mut Vec<usize>, mut point: usize| {
			while merged_into[point] != point {
				merged_into[point] = merged_into[merged_into[point]];
				point = merged_into[point];
			}
			point
		};
		for &(from, to) in merges {
			let (from, to) = (from as usize, to as usize);
			if from >= point_count || to >= point_count {
				continue;
			}

			let (from, to) = (representative(&mut merged_into, from), representative(&mut merged_into, to));
			merged_into[from] = to;
		}
		let representatives: Vec<usize> = (0..point_count).map(|point| representative(&mut merged_into, point)).collect();

		// Kept points are renumbered past those dropped, and every reference names its point's representative
		let mut renumbered = Vec::with_capacity(point_count);
		let mut kept_count = 0;
		for (point, &representative) in representatives.iter().enumerate() {
			renumbered.push(kept_count);
			kept_count += usize::from(representative == point);
		}
		for path in paths.iter_element_values_mut() {
			for reference in path.0.iter_element_values_mut() {
				if let Some(&representative) = usize::try_from(*reference).ok().and_then(|point| representatives.get(point)) {
					*reference = renumbered[representative] as i64;
				}
			}
		}

		let kept = representatives.iter().enumerate().map(|(point, &representative)| representative == point);
		self.0 = std::mem::take(&mut self.0).into_iter().zip(kept).filter_map(|(point, kept)| kept.then_some(point)).collect();
	}

	/// Appends another vector's points and paths, renaming the IDs this vector already uses, and records the appended IDs in `taken_ids`.
	/// `taken_ids` must hold this vector's IDs, as [`IdCollisionSet::of`] gives them.
	pub fn concat(&mut self, paths: &mut List<Path>, additional: &Self, additional_paths: &List<Path>, transform_of_additional: DAffine2, collision_hash_seed: u64, taken_ids: &mut IdCollisionSet) {
		let offset = self.0.len() as u32;

		// Remap colliding stable IDs so both sides stay addressable after the merge.
		// Appended IDs are only checked against the IDs taken before this call.
		let mut appended_point_ids = Vec::new();
		for index in 0..additional.0.len() {
			let Some(&position) = additional.0.element(index) else { continue };
			let mut item = Item::new_from_element(transform_of_additional.transform_point2(position));

			if let Some(&id) = additional.0.attribute::<PointId>(ATTR_ID, index) {
				let id = if id != PointId::UNNAMED && taken_ids.points.contains(&id) {
					id.generate_from_hash(collision_hash_seed)
				} else {
					id
				};
				item.set_attribute(ATTR_ID, id);
				appended_point_ids.push(id);
			}

			self.0.push(item);
		}

		let mut appended_segment_ids = Vec::new();
		for index in 0..additional_paths.len() {
			let Some(mut item) = additional_paths.clone_item(index) else { continue };

			let path = item.element_mut();
			for reference in path.0.iter_element_values_mut() {
				*reference += i64::from(offset);
			}
			if let Some(ids) = path.0.iter_attribute_values_mut::<SegmentId>(ATTR_ID) {
				for id in ids {
					if *id != SegmentId::ZERO && taken_ids.segments.contains(id) {
						*id = id.generate_from_hash(collision_hash_seed);
					}
					appended_segment_ids.push(*id);
				}
			}

			paths.push(item);
		}

		taken_ids.points.extend(appended_point_ids);
		taken_ids.segments.extend(appended_segment_ids);
	}

	/// Rewrites every stable ID deterministically from the given hash, so instanced copies stay distinct.
	pub fn vector_new_ids_from_hash(&mut self, paths: &mut List<Path>, node_id: u64) {
		if let Some(ids) = self.0.iter_attribute_values_mut::<PointId>(ATTR_ID) {
			for id in ids.filter(|id| **id != PointId::UNNAMED) {
				*id = id.generate_from_hash(node_id);
			}
		}

		for index in 0..paths.len() {
			let Some(path) = paths.element_mut(index) else { continue };
			if let Some(ids) = path.0.iter_attribute_values_mut::<SegmentId>(ATTR_ID) {
				for id in ids.filter(|id| **id != SegmentId::ZERO) {
					*id = id.generate_from_hash(node_id);
				}
			}
		}
	}
}

/// The stable IDs a vector uses, carried across [`Vector::concat`] calls so merging many vectors stays linear.
#[derive(Clone, Debug, Default)]
pub struct IdCollisionSet {
	points: rustc_hash::FxHashSet<PointId>,
	segments: rustc_hash::FxHashSet<SegmentId>,
}

impl IdCollisionSet {
	/// The IDs used by `vector`'s points and `paths`' segments.
	pub fn of(vector: &Vector, paths: &List<Path>) -> Self {
		let points = vector.0.iter_attribute_values::<PointId>(ATTR_ID).into_iter().flatten().copied().collect();
		let segments = paths
			.iter_element_values()
			.filter_map(|path| path.0.iter_attribute_values::<SegmentId>(ATTR_ID))
			.flatten()
			.copied()
			.collect();

		Self { points, segments }
	}
}

/// Edits a holder item's pool and paths together, taking the paths attribute out and putting it back where it was, or last if absent.
pub fn edit_item_geometry<R>(item: &mut Item<Vector>, edit: impl FnOnce(&mut Vector, &mut List<Path>) -> R) -> R {
	let mut paths = item.attribute_mut::<List<Path>>(ATTR_PATHS).map(std::mem::take).unwrap_or_default();
	let result = edit(item.element_mut(), &mut paths);
	*item.attribute_mut_or_insert_default::<List<Path>>(ATTR_PATHS) = paths;
	item.set_attribute(ATTR_GEOMETRY_CACHE, GeometryCache::default());

	result
}

/// Edits an item's pool while only reading its paths, which stay shared with the item's clones instead of being copied.
pub fn edit_item_points<R>(item: &mut Item<Vector>, edit: impl FnOnce(&mut Vector, &List<Path>) -> R) -> R {
	let paths = shared_paths_from_item_attributes(item);
	let result = edit(item.element_mut(), &paths);
	ensure_paths_attribute(item);
	item.set_attribute(ATTR_GEOMETRY_CACHE, GeometryCache::default());

	result
}

/// Gives an item without paths an empty paths attribute, placed last, as [`edit_item_geometry`] gives a point cloud.
pub fn ensure_paths_attribute(item: &mut Item<Vector>) {
	if item.attribute::<List<Path>>(ATTR_PATHS).is_none() {
		item.remove_attribute::<List<Path>>(ATTR_PATHS);
		item.set_attribute(ATTR_PATHS, List::<Path>::new());
	}
}

/// Edits one list row's pool and paths together, writing the paths attribute back afterwards.
pub fn edit_list_row_geometry<R>(list: &mut List<Vector>, index: usize, edit: impl FnOnce(&mut Vector, &mut List<Path>) -> R) -> Option<R> {
	let mut paths = list.attribute::<List<Path>>(ATTR_PATHS, index).cloned().unwrap_or_default();
	let result = edit(list.element_mut(index)?, &mut paths);
	set_paths_at(list, index, paths);

	Some(result)
}

// Alone, the element sees only its own pool, and the pool's control points always contain the drawn geometry.
impl BoundingBox for Vector {
	fn bounding_box_in_item(item: &Item<Self>, transform: DAffine2, _include_stroke: bool, _thumbnail: bool) -> RenderBoundingBox {
		let paths = paths_from_item_attributes(item).unwrap_or_else(|| no_paths());
		item.element()
			.bounding_box_with_transform(paths, transform)
			.map_or(RenderBoundingBox::None, RenderBoundingBox::Rectangle)
	}

	fn bounding_box_in_list(list: &List<Self>, index: usize, transform: DAffine2, _include_stroke: bool, _thumbnail: bool) -> RenderBoundingBox {
		let Some(vector) = list.element(index) else { return RenderBoundingBox::None };
		let paths = paths_from_list_row_attributes(list, index).unwrap_or_else(|| no_paths());
		vector.bounding_box_with_transform(paths, transform).map_or(RenderBoundingBox::None, RenderBoundingBox::Rectangle)
	}

	fn bounding_box(&self, transform: DAffine2, _include_stroke: bool) -> RenderBoundingBox {
		let mut bounds: Option<[DVec2; 2]> = None;
		for position in self.positions() {
			let position = transform.transform_point2(position);
			bounds = Some(match bounds {
				Some([min, max]) => [min.min(position), max.max(position)],
				None => [position, position],
			});
		}

		match bounds {
			Some(bounds) => RenderBoundingBox::Rectangle(bounds),
			None => RenderBoundingBox::None,
		}
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		BoundingBox::bounding_box(self, transform, include_stroke)
	}
}

// Note: BoundingBox for List<Vector> is handled by blanket impl in core-types

#[cfg(test)]
mod tests {
	use super::*;
	use crate::vector::algorithms::shapes::ellipse_bezpath;
	use kurbo::{CubicBez, PathEl, Point};

	fn open_curve_bezpath() -> BezPath {
		let mut bezpath = BezPath::new();
		bezpath.move_to(Point::ZERO);
		bezpath.curve_to(Point::new(-1., -1.), Point::new(1., 1.), Point::new(1., 0.));
		bezpath
	}

	fn lone_anchor(position: DVec2) -> Vector {
		<Vector as core_types::ops::FromAnchorPosition>::from_anchor_position(position)
	}

	// Bezpaths appended together come out as they do appended one at a time, IDs included, with the spans each one added counted
	#[test]
	fn bezpaths_appended_together_match_one_at_a_time() {
		let mut mixed = BezPath::new();
		mixed.move_to((0., 0.));
		mixed.line_to((4., 0.));
		mixed.quad_to((6., 2.), (4., 4.));
		mixed.close_path();
		let bezpaths = [ellipse_bezpath(DVec2::NEG_ONE, DVec2::ONE), BezPath::new(), mixed, open_curve_bezpath()];

		let (mut together, mut together_paths) = (lone_anchor(DVec2::splat(9.)), List::new());
		let span_counts = together.append_bezpaths_counting_spans(&mut together_paths, &bezpaths);
		assert_eq!(span_counts, [4, 0, 3, 1]);

		let (mut in_turn, mut in_turn_paths) = (lone_anchor(DVec2::splat(9.)), List::new());
		for bezpath in &bezpaths {
			in_turn.append_bezpath(&mut in_turn_paths, bezpath);
		}

		assert_eq!((together, together_paths), (in_turn, in_turn_paths));
	}

	// Each appended bezpath's IDs skip one after the largest before it, which documents saved with edits to such geometry rely on
	#[test]
	fn appended_bezpaths_continue_the_id_sequences_with_a_gap() {
		let mut triangle = BezPath::new();
		triangle.move_to((0., 0.));
		triangle.line_to((4., 0.));
		triangle.line_to((0., 4.));
		triangle.close_path();
		let named = |vector: &Vector, paths: &List<Path>| {
			let point_ids: Vec<u64> = vector.0.iter_attribute_values::<PointId>(ATTR_ID).into_iter().flatten().map(|id| id.inner()).collect();
			let segment_ids: Vec<u64> = vector.segment_iter(paths).map(|(id, ..)| id.inner()).collect();
			(point_ids, segment_ids)
		};

		let (mut vector, mut paths) = (Vector::default(), List::new());
		vector.append_bezpaths(&mut paths, [&triangle, &triangle]);
		assert_eq!(named(&vector, &paths), (vec![1, 2, 3, 5, 6, 7], vec![1, 2, 3, 5, 6, 7]));

		// A pool holding IDs that name nothing yet starts its sequence one further on
		let mut unnamed = Vector::default();
		unnamed.0.set_attribute_values(ATTR_ID, Vec::<PointId>::new());
		let mut unnamed_paths = List::new();
		unnamed.append_bezpath(&mut unnamed_paths, &triangle);
		assert_eq!(named(&unnamed, &unnamed_paths).0, [2, 3, 4]);
	}

	// A lone anchor is named zero and only the later collisions are renamed, so stored edits keyed on zero still land
	#[test]
	fn lone_anchor_is_named_zero_and_collisions_rename_from_it() {
		let (mut vector, mut paths) = (lone_anchor(DVec2::ZERO), List::new());
		assert_eq!(vector.point_id_at(0), Some(PointId::ZERO));

		let mut taken_ids = IdCollisionSet::of(&vector, &paths);
		vector.concat(&mut paths, &lone_anchor(DVec2::X), &List::new(), DAffine2::IDENTITY, 7, &mut taken_ids);
		vector.concat(&mut paths, &lone_anchor(DVec2::Y), &List::new(), DAffine2::IDENTITY, 8, &mut taken_ids);

		assert_eq!(vector.point_id_at(0), Some(PointId::ZERO));
		assert_eq!(vector.point_id_at(1), Some(PointId::ZERO.generate_from_hash(7)));
		assert_eq!(vector.point_id_at(2), Some(PointId::ZERO.generate_from_hash(8)));
		assert_eq!(vector.index_of_point_id(PointId::ZERO), Some(0));
	}

	// Handles carry the unnamed filler, so a zero-named anchor behind them is still the one found by ID
	#[test]
	fn handles_do_not_shadow_a_zero_named_anchor() {
		let (mut vector, mut paths) = Vector::from_bezpath(&open_curve_bezpath());
		assert_eq!(vector.point_id_at(1), Some(PointId::UNNAMED));
		assert_eq!(vector.point_id_at(2), Some(PointId::UNNAMED));

		let mut taken_ids = IdCollisionSet::of(&vector, &paths);
		vector.concat(&mut paths, &lone_anchor(DVec2::new(5., 5.)), &List::new(), DAffine2::IDENTITY, 1, &mut taken_ids);

		let index = vector.index_of_point_id(PointId::ZERO).unwrap();
		assert_eq!(vector.position(index as usize), Some(DVec2::new(5., 5.)));
		assert!(vector.anchor_indices(&paths).contains(&(index as usize)));
		assert_eq!(largest_point_id(&vector.0).map(PointId::inner), Some(2));
	}

	#[test]
	fn construct_closed_path() {
		let circle = ellipse_bezpath(DVec2::NEG_ONE, DVec2::ONE);
		let (vector, paths) = Vector::from_bezpath(&circle);
		assert_eq!(vector.anchor_points(&paths).count(), 4);

		let segments = vector.segment_iter(&paths).map(|(_, segment, _, _)| segment).collect::<Vec<_>>();
		assert_eq!(segments.len(), 4);
		assert!(segments.iter().all(|&segment| circle.segments().any(|original| original == segment)));

		let generated = vector.stroke_bezpath_iter(&paths).collect::<Vec<_>>();
		assert_eq!(generated.len(), 1);
		assert_eq!(generated[0].elements(), circle.elements());
	}

	#[test]
	fn construct_open_path() {
		let curve = open_curve_bezpath();
		let (vector, paths) = Vector::from_bezpath(&curve);
		assert_eq!(vector.anchor_points(&paths).count(), 2);

		let segments = vector.segment_iter(&paths).map(|(_, segment, _, _)| segment).collect::<Vec<_>>();
		assert_eq!(segments, vec![PathSeg::Cubic(CubicBez::new(Point::ZERO, Point::new(-1., -1.), Point::new(1., 1.), Point::new(1., 0.)))]);
	}

	#[test]
	fn construct_many_paths() {
		let curve = open_curve_bezpath();
		let circle = ellipse_bezpath(DVec2::NEG_ONE, DVec2::ONE);

		let (mut vector, mut paths) = Vector::from_bezpath(&curve);
		vector.append_bezpath(&mut paths, &circle);
		assert_eq!(vector.anchor_points(&paths).count(), 6);

		let segments = vector.segment_iter(&paths).map(|(_, segment, _, _)| segment).collect::<Vec<_>>();
		assert_eq!(segments.len(), 5);
		assert!(segments.iter().all(|&segment| circle.segments().chain(curve.segments()).any(|original| original == segment)));

		let generated = vector.stroke_bezpath_iter(&paths).collect::<Vec<_>>();
		assert_eq!(generated.len(), 2);
		assert_eq!(generated[0].elements(), curve.elements());
		assert_eq!(generated[1].elements(), circle.elements());
	}

	// Stored modifications name these exact IDs, so regeneration must reproduce them: anchors count up from one while handles stay unnamed
	#[test]
	fn appended_ids_stay_reproducible_for_stored_documents() {
		let curve = open_curve_bezpath();
		let (mut vector, mut paths) = Vector::from_bezpath(&curve);

		let point_id = |vector: &Vector, index| vector.point_id_at(index).map(|id| id.inner());
		assert_eq!(point_id(&vector, 0), Some(1));
		assert_eq!(vector.point_id_at(1), Some(PointId::UNNAMED), "handles are unnamed");
		assert_eq!(vector.point_id_at(2), Some(PointId::UNNAMED));
		assert_eq!(point_id(&vector, 3), Some(2));

		let segment_ids: Vec<_> = vector.segment_iter(&paths).map(|(id, ..)| id.inner()).collect();
		assert_eq!(segment_ids, vec![1]);

		// A second append rescans for the maximum and skips one value
		vector.append_bezpath(&mut paths, &open_curve_bezpath());
		assert_eq!(point_id(&vector, 4), Some(4));
		assert_eq!(point_id(&vector, 7), Some(5));
	}

	// A lone anchor position builds one named pool point
	#[test]
	fn anchor_position_builds_single_point_path() {
		use core_types::ops::FromAnchorPosition;

		let vector = Vector::from_anchor_position(DVec2::new(3., 4.));

		assert_eq!(vector.positions().collect::<Vec<_>>(), [DVec2::new(3., 4.)]);
		assert_eq!(vector.point_id_at(0), Some(PointId::ZERO), "the anchor keeps the zero ID stored edits address it by");
		assert_eq!(vector.point_count(), 1);
	}

	// An open subpath welds shut with one linear closing segment, and a second call leaves it alone
	#[test]
	fn close_subpaths_welds_open_subpaths() {
		let mut bezpath = BezPath::new();
		bezpath.move_to((0., 0.));
		bezpath.line_to((10., 0.));
		bezpath.line_to((10., 10.));

		let (mut vector, mut paths) = Vector::from_bezpath(&bezpath);
		assert_eq!(paths.len(), 1);

		vector.close_subpaths(&mut paths);
		assert_eq!(paths.len(), 1);
		assert!(vector.stroke_bezpath_iter(&paths).all(|contour| matches!(contour.elements().last(), Some(PathEl::ClosePath))));

		vector.close_subpaths(&mut paths);
		assert_eq!(paths.len(), 1);
	}

	// A crossbar between two junctions closes in one pass like the uprights it joins, and closing again adds nothing
	#[test]
	fn close_subpaths_closes_a_path_between_two_junctions() {
		let mut vector = Vector::default();
		for position in [(0., 0.), (0., 5.), (0., 10.), (10., 0.), (10., 5.), (10., 10.)] {
			vector.0.push(Item::new_from_element(DVec2::from(position)));
		}

		// Two uprights joined by a crossbar at their midpoints, like an H
		let mut paths = List::new();
		for references in [vec![0, 1, 2], vec![3, 4, 5], vec![1, 4]] {
			push_path(&mut paths, CurveType::Linear, references, []);
		}

		vector.close_subpaths(&mut paths);
		let contours: Vec<_> = vector.stroke_bezpath_iter(&paths).collect();
		assert_eq!(contours.len(), 3);
		assert!(contours.iter().all(|contour| matches!(contour.elements().last(), Some(PathEl::ClosePath))));

		vector.close_subpaths(&mut paths);
		assert_eq!(paths.len(), 3);
	}

	// A curved path ending at a junction closes with a linear path stored after it, which carries on its path flow through the junction
	#[test]
	fn close_subpaths_carries_on_a_curved_path_flow() {
		let mut vector = Vector::default();
		for position in [(0., 0.), (5., -5.), (15., -5.), (20., 0.), (20., 10.), (30., 0.)] {
			vector.0.push(Item::new_from_element(DVec2::from(position)));
		}
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Cubic, [0, 1, 2, 3], []);
		push_path(&mut paths, CurveType::Linear, [4, 3], []);
		push_path(&mut paths, CurveType::Linear, [5, 3], []);

		vector.close_subpaths(&mut paths);

		let references: Vec<Vec<u32>> = paths.iter_element_values().map(|path| path.references().collect()).collect();
		assert_eq!(references, [vec![0, 1, 2, 3], vec![3, 0], vec![4, 3, 4], vec![5, 3, 5]]);
		assert!(contours(vector.point_count(), &paths).iter().all(|contour| contour.closed));
	}

	// Paths ending at a junction close too, and closing them again adds nothing
	#[test]
	fn close_subpaths_closes_paths_ending_at_a_junction() {
		let mut vector = Vector::default();
		for index in 0..5 {
			vector.0.push(Item::new_from_element(DVec2::new(index as f64, (index * index) as f64)));
		}
		let mut paths = List::new();
		for references in [vec![0, 1, 2], vec![3, 2], vec![4, 2]] {
			push_path(&mut paths, CurveType::Linear, references, []);
		}

		vector.close_subpaths(&mut paths);
		assert_eq!(paths.len(), 3);
		assert!(vector.stroke_bezpath_iter(&paths).all(|contour| matches!(contour.elements().last(), Some(PathEl::ClosePath))));

		vector.close_subpaths(&mut paths);
		assert_eq!(paths.len(), 3);
	}

	// An end resting on its own start, within floating point noise, merges into it instead of adding a closing segment, while a wider gap still gets one
	#[test]
	fn close_subpaths_merges_an_end_resting_on_its_start() {
		for (gap, merges) in [(0., true), (1e-9, true), (1e-3, false)] {
			let mut vector = Vector::default();
			for position in [(0., 0.), (10., 0.), (10., 10.), (gap, 0.)] {
				vector.0.push(Item::new_from_element(DVec2::from(position)));
			}
			let mut paths = List::new();
			push_path(&mut paths, CurveType::Linear, [0, 1, 2, 3], []);

			vector.close_subpaths(&mut paths);

			let references: Vec<Vec<u32>> = paths.iter_element_values().map(|path| path.references().collect()).collect();
			if merges {
				assert_eq!(references, [vec![0, 1, 2, 0]], "gap {gap}");
				assert_eq!(vector.point_count(), 3, "gap {gap}");
			} else {
				assert_eq!(references, [vec![0, 1, 2, 3, 0]], "gap {gap}");
				assert_eq!(vector.point_count(), 4, "gap {gap}");
			}
			assert!(contours(vector.point_count(), &paths).iter().all(|contour| contour.closed), "gap {gap}");
		}
	}

	// Closing paths land after the curved paths they carry on even when contours end out of storage order, so one meeting junctions at both ends still closes
	#[test]
	fn close_subpaths_places_closing_paths_in_storage_order() {
		let mut vector = Vector::default();
		for index in 0..13 {
			vector.0.push(Item::new_from_element(DVec2::new(index as f64, (index * index) as f64)));
		}

		// The first contour welds on path 2, so it ends after the second, which meets junctions at points 5 and 6
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Linear, [0, 1], []);
		push_path(&mut paths, CurveType::Cubic, [5, 9, 10, 6], []);
		push_path(&mut paths, CurveType::Cubic, [1, 2, 3, 4], []);
		for references in [[7, 6], [8, 6], [11, 5], [12, 5]] {
			push_path(&mut paths, CurveType::Linear, references, []);
		}

		vector.close_subpaths(&mut paths);

		let references: Vec<Vec<u32>> = paths.iter_element_values().map(|path| path.references().collect()).collect();
		assert_eq!(references[..5], [vec![0, 1], vec![5, 9, 10, 6], vec![6, 5], vec![1, 2, 3, 4], vec![4, 0]]);
		assert!(contours(vector.point_count(), &paths).iter().all(|contour| contour.closed));
	}

	// Paths ending where a merged end was still meet the closed path there, at the start it merged into
	#[test]
	fn close_subpaths_keeps_paths_meeting_a_merged_end() {
		let mut vector = Vector::default();
		for position in [(0., 0.), (10., 0.), (10., 10.), (0., 0.), (-10., 0.), (0., -10.)] {
			vector.0.push(Item::new_from_element(DVec2::from(position)));
		}
		let mut paths = List::new();
		for references in [vec![0, 1, 2, 3], vec![4, 3], vec![5, 3]] {
			push_path(&mut paths, CurveType::Linear, references, []);
		}

		vector.close_subpaths(&mut paths);

		assert_eq!(vector.point_count(), 5);
		let references: Vec<Vec<u32>> = paths.iter_element_values().map(|path| path.references().collect()).collect();
		assert_eq!(references[0], [0, 1, 2, 0]);
		assert!(
			references.iter().filter(|path| path.contains(&3)).all(|path| path.contains(&0)),
			"the side paths end at the merged start"
		);
	}
}
