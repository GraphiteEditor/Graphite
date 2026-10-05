use super::algorithms::consts::CLOSE_POINT_TOLERANCE;
use super::misc::{dvec2_to_point, point_to_dvec2};
use super::vector_attributes::SegmentId;
use core_types::list::{ATTR_CURVE_TYPE, ATTR_ID, ATTR_PATHS, Item, List, Shared};
use glam::DVec2;
use graphene_hash::CacheHash;
use kurbo::{BezPath, CubicBez, Line, PathEl, PathSeg, Point, QuadBez};

/// The curve family a path's point references weave in, fixing the stride of its reference pattern.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, CacheHash)]
pub enum CurveType {
	/// Straight segments between consecutive anchors, with every reference an anchor.
	#[default]
	Linear,
	/// Quadratic Bézier segments, with one handle reference between each pair of anchors.
	Quadratic,
	/// Cubic Bézier segments, with two handle references between each pair of anchors.
	Cubic,
}

impl CurveType {
	/// Returns the number of references stepping from one anchor to the next: the segment's handle count plus one.
	pub fn stride(&self) -> usize {
		match self {
			Self::Linear => 1,
			Self::Quadratic => 2,
			Self::Cubic => 3,
		}
	}
}

impl std::fmt::Display for CurveType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Linear => write!(f, "Linear"),
			Self::Quadratic => write!(f, "Quadratic"),
			Self::Cubic => write!(f, "Cubic"),
		}
	}
}

/// One connected stroke of geometry: an ordered list of references (indices into the owning `Vector`'s point pool)
/// woven in its curve type's stride order. Welding is two paths, or two ends of one path, referencing the same anchor.
/// Per-reference data such as `ATTR_SMOOTH` rides this list's attributes.
///
/// References are stored as `i64`, the integer type the data model standardizes on, and read back as `u32` pool indices.
#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub struct Path(pub List<i64>);

impl Path {
	/// Builds a path from pool indices, with no per-reference attributes.
	pub fn from_references(references: impl IntoIterator<Item = u32>) -> Self {
		Self(List::new_from_elements(references.into_iter().map(i64::from).collect()))
	}

	/// The pool index named by the reference at `index`, or `None` if out of bounds.
	pub fn reference(&self, index: usize) -> Option<u32> {
		self.0.element(index).map(|&reference| reference as u32)
	}

	/// The pool indices named by each reference, in weave order.
	pub fn references(&self) -> impl ExactSizeIterator<Item = u32> + '_ {
		self.0.iter_element_values().map(|&reference| reference as u32)
	}

	/// Whether this path is closed, indicated by its first and last references naming the same pool point.
	pub fn is_closed(&self) -> bool {
		let len = self.0.len();
		len > 1 && self.0.element(0) == self.0.element(len - 1)
	}

	/// The number of segments this path draws, one fewer than its anchor count when open and equal to it when closed.
	pub fn segment_count(&self, curve_type: CurveType) -> usize {
		self.0.len().saturating_sub(1) / curve_type.stride()
	}

	/// The pool position named by the reference at `index`, or `None` if either lookup falls out of bounds.
	pub fn position(&self, pool: &List<DVec2>, index: usize) -> Option<DVec2> {
		pool.element(self.reference(index)? as usize).copied()
	}

	/// The segment at `index`, resolved against the point pool.
	pub fn segment(&self, pool: &List<DVec2>, curve_type: CurveType, index: usize) -> Option<PathSeg> {
		let start = index * curve_type.stride();
		let point = |offset: usize| self.position(pool, start + offset).map(dvec2_to_point);

		Some(match curve_type {
			CurveType::Linear => PathSeg::Line(Line::new(point(0)?, point(1)?)),
			CurveType::Quadratic => PathSeg::Quad(QuadBez::new(point(0)?, point(1)?, point(2)?)),
			CurveType::Cubic => PathSeg::Cubic(CubicBez::new(point(0)?, point(1)?, point(2)?, point(3)?)),
		})
	}

	/// A copy of the spans in `start_span..end_span`, keeping both boundary anchors so the pieces stay welded.
	pub fn clone_span_range(&self, curve_type: CurveType, start_span: usize, end_span: usize) -> Path {
		let stride = curve_type.stride();
		let first_reference = start_span * stride;
		let last_reference = (end_span * stride).min(self.0.len().saturating_sub(1));

		let mut piece = Path((first_reference..=last_reference).filter_map(|index| self.0.clone_item(index)).collect());

		// The final anchor's cell may hold the ID of a span outside the piece, which its own piece keeps
		let last_index = piece.0.len().saturating_sub(1);
		if piece.0.attribute::<SegmentId>(ATTR_ID, last_index).is_some() {
			piece.0.set_attribute(ATTR_ID, last_index, SegmentId::default());
		}

		piece
	}

	/// Reverses the given spans where they sit, so contours cross them in the opposite direction.
	/// Per-reference attributes travel with their references, and each span's ID moves to the anchor it now departs from.
	pub fn reverse_spans(&mut self, curve_type: CurveType, spans: std::ops::Range<usize>) {
		let stride = curve_type.stride();
		let spans = spans.start..spans.end.min(self.segment_count(curve_type));
		if spans.is_empty() {
			return;
		}
		let (first, last) = (spans.start * stride, spans.end * stride);

		// The cell on the last anchor belongs to whichever span follows these, so it stays where it is
		let id_at = |path: &Self, index: usize| path.0.attribute::<SegmentId>(ATTR_ID, index).copied();
		let ids: Vec<Option<SegmentId>> = spans.clone().map(|span| id_at(self, span * stride)).collect();
		let following_id = id_at(self, last);

		let mirrored = |index: usize| if (first..=last).contains(&index) { first + last - index } else { index };
		self.0 = (0..self.0.len()).filter_map(|index| self.0.clone_item(mirrored(index))).collect();

		// Each reversed span walks the old span mirrored across the range backwards, so it takes over that span's ID
		for (place, id) in ids.into_iter().rev().chain([following_id]).enumerate() {
			if let Some(id) = id {
				self.0.set_attribute(ATTR_ID, first + place * stride, id);
			}
		}
	}
}

/// Weaves the given references into a path in the curve type's stride, naming its spans with the given IDs in order.
fn woven_path(curve_type: CurveType, references: impl IntoIterator<Item = u32>, ids: impl IntoIterator<Item = SegmentId>) -> Path {
	let mut path = Path::from_references(references);

	let stride = curve_type.stride();
	let mut ids = ids.into_iter().take(path.segment_count(curve_type)).peekable();
	if ids.peek().is_some() {
		let mut span_ids = vec![SegmentId::default(); path.0.len()];
		for (span, id) in ids.enumerate() {
			span_ids[span * stride] = id;
		}
		path.0.set_attribute_values(ATTR_ID, span_ids);
	}

	path
}

/// Appends one path weaving the given references in the curve type's stride, naming its spans with the given IDs in order.
pub fn push_path(paths: &mut List<Path>, curve_type: CurveType, references: impl IntoIterator<Item = u32>, ids: impl IntoIterator<Item = SegmentId>) {
	let path = woven_path(curve_type, references, ids);
	paths.push(Item::new_from_element(path).with_attribute(ATTR_CURVE_TYPE, curve_type));
}

/// Appends a path for each run of references, as [`push_path`] would one at a time.
pub(crate) fn push_paths(paths: &mut List<Path>, runs: impl IntoIterator<Item = (CurveType, Vec<u32>, Vec<SegmentId>)>) {
	let woven = runs.into_iter().map(|(curve_type, references, ids)| (curve_type, woven_path(curve_type, references, ids)));
	let (curve_types, woven) = woven.unzip();

	extend_with_paths(paths, curve_types, woven);
}

/// Appends paths with their curve types as one piece, which spares each path a pass through an item of its own.
fn extend_with_paths(paths: &mut List<Path>, curve_types: Vec<CurveType>, woven: Vec<Path>) {
	if woven.is_empty() {
		return;
	}

	let mut appended = List::new_from_elements(woven);
	appended.set_attribute_values(ATTR_CURVE_TYPE, curve_types);
	paths.extend(appended);
}

/// Appends a segment to a bezpath already positioned at that segment's start point.
pub fn append_segment(bezpath: &mut BezPath, segment: PathSeg) {
	match segment {
		PathSeg::Line(line) => bezpath.line_to(line.p1),
		PathSeg::Quad(quad) => bezpath.quad_to(quad.p1, quad.p2),
		PathSeg::Cubic(cubic) => bezpath.curve_to(cubic.p1, cubic.p2, cubic.p3),
	}
}

/// The paths an item carries beside a vector element, absent when the pool is a bare point cloud.
pub fn paths_from_item_attributes<T>(item: &Item<T>) -> Option<&List<Path>> {
	item.attribute(ATTR_PATHS)
}

/// The paths an item carries beside a vector element, held without copying them and empty when it carries none.
/// The handle goes on reading the paths whatever then happens to the item.
pub fn shared_paths_from_item_attributes<T>(item: &Item<T>) -> Shared<List<Path>> {
	static EMPTY: std::sync::LazyLock<std::sync::Arc<List<Path>>> = std::sync::LazyLock::new(Default::default);

	item.attribute_shared(ATTR_PATHS).unwrap_or_else(|| EMPTY.clone().into())
}

/// The paths a list carries at `index` beside a vector element, absent when the pool is a bare point cloud.
pub fn paths_from_list_row_attributes<T>(list: &List<T>, index: usize) -> Option<&List<Path>> {
	list.attribute(ATTR_PATHS, index)
}

/// The empty weave, standing in for a pool with a holder that carries no paths.
pub fn no_paths() -> &'static List<Path> {
	static EMPTY: std::sync::LazyLock<List<Path>> = std::sync::LazyLock::new(List::new);
	&EMPTY
}

/// The curve type the path at `index` weaves in.
pub fn curve_type_at(paths: &List<Path>, index: usize) -> CurveType {
	paths.attribute_cloned_or_default(ATTR_CURVE_TYPE, index)
}

/// Whether the path at `index` only carries on the one stored before it in another curve type, setting out from
/// that path's last anchor. Such a path begins nothing of its own.
pub fn continues_previous_path(paths: &List<Path>, index: usize) -> bool {
	let Some(previous_index) = index.checked_sub(1) else { return false };
	let (Some(path), Some(previous)) = (paths.element(index), paths.element(previous_index)) else {
		return false;
	};
	let previous_last = previous.0.len().saturating_sub(1);

	previous_last > 0 && path.0.element(0) == previous.0.element(previous_last) && curve_type_at(paths, index) != curve_type_at(paths, previous_index)
}

/// Weaves bezpath contours into pool points and paths, starting a new path at each verb change so every
/// path keeps one stride. The runs of a split subpath share the anchor they meet at, welding them.
struct PathWeaver {
	positions: Vec<DVec2>,
	anchors: Vec<bool>,
	offset: u32,
	woven: Vec<(CurveType, Vec<u32>)>,
	references: Vec<u32>,
	curve_type: Option<CurveType>,
	subpath_start: Option<u32>,
}

impl PathWeaver {
	fn new(offset: u32) -> Self {
		Self {
			positions: Vec::new(),
			anchors: Vec::new(),
			offset,
			woven: Vec::new(),
			references: Vec::new(),
			curve_type: None,
			subpath_start: None,
		}
	}

	/// Adds an anchor or handle point to the pool being built and returns the reference naming it.
	fn push_point(&mut self, point: Point, anchor: bool) -> u32 {
		self.positions.push(point_to_dvec2(point));
		self.anchors.push(anchor);
		self.offset + self.positions.len() as u32 - 1
	}

	fn position(&self, reference: u32) -> Option<DVec2> {
		self.positions.get(reference.checked_sub(self.offset)? as usize).copied()
	}

	/// Banks the references woven so far as a finished path, dropping a lone anchor that drew nothing.
	fn flush(&mut self) {
		if self.references.len() >= 2 {
			let curve_type = self.curve_type.unwrap_or_default();
			self.woven.push((curve_type, std::mem::take(&mut self.references)));
		}

		self.references.clear();
		self.curve_type = None;
	}

	/// Prepares to weave a segment of the given curve type, splitting at the shared anchor when the verb changes.
	/// Returns whether an anchor exists to draw from.
	fn begin_segment(&mut self, verb: CurveType) -> bool {
		// After a close the pen rests on the subpath's first point, so a segment drawn with no new MoveTo departs from there
		let weld = match self.references.last() {
			Some(&weld) => weld,
			None => {
				let Some(start) = self.subpath_start else { return false };
				self.references.push(start);
				start
			}
		};

		if let Some(current) = self.curve_type
			&& current != verb
		{
			self.flush();
			self.references.push(weld);
		}
		self.curve_type = Some(verb);

		true
	}

	fn push_element(&mut self, element: PathEl) {
		match element {
			PathEl::MoveTo(point) => {
				self.flush();
				let reference = self.push_point(point, true);
				self.subpath_start = Some(reference);
				self.references.push(reference);
			}
			PathEl::LineTo(anchor) => self.push_segment(CurveType::Linear, &[anchor]),
			PathEl::QuadTo(handle, anchor) => self.push_segment(CurveType::Quadratic, &[handle, anchor]),
			PathEl::CurveTo(start_handle, end_handle, anchor) => self.push_segment(CurveType::Cubic, &[start_handle, end_handle, anchor]),
			PathEl::ClosePath => self.close(),
		}
	}

	fn push_segment(&mut self, verb: CurveType, points: &[Point]) {
		if !self.begin_segment(verb) {
			return;
		}

		// Every point but the last is a handle leading to the anchor that ends the segment
		for (index, &point) in points.iter().enumerate() {
			let reference = self.push_point(point, index + 1 == points.len());
			self.references.push(reference);
		}
	}

	/// Closes the subpath by naming its first point again, drawing a straight line home when the pen sits elsewhere.
	fn close(&mut self) {
		let (Some(start), Some(&last)) = (self.subpath_start, self.references.last()) else { return };

		if last == start {
			self.flush();
			return;
		}

		// A final anchor resting on the subpath's first point is that same point, so name it instead of duplicating it
		let coincident = matches!((self.position(last), self.position(start)), (Some(last), Some(start)) if last.distance_squared(start) <= CLOSE_POINT_TOLERANCE * CLOSE_POINT_TOLERANCE);
		if coincident {
			self.references.pop();
			self.references.push(start);
			if last as usize + 1 == self.offset as usize + self.positions.len() {
				self.positions.pop();
				self.anchors.pop();
			}
		} else if self.begin_segment(CurveType::Linear) {
			self.references.push(start);
		}

		self.flush();
	}

	/// Appends the woven points and paths, returning whether each appended point is an anchor rather than a handle.
	fn finish(mut self, pool: &mut List<DVec2>, paths: &mut List<Path>) -> Vec<bool> {
		self.flush();

		pool.extend(List::new_from_elements(self.positions));

		let woven = self.woven.into_iter().map(|(curve_type, references)| (curve_type, Path::from_references(references)));
		let (curve_types, woven) = woven.unzip();
		extend_with_paths(paths, curve_types, woven);

		self.anchors
	}
}

/// Appends a bezpath's contours to a point pool and its paths.
pub fn append_bezpath(pool: &mut List<DVec2>, paths: &mut List<Path>, bezpath: &BezPath) {
	weave_bezpath(pool, paths, bezpath);
}

/// Appends a bezpath's contours to a point pool and its paths, returning whether each appended point is an anchor.
pub(crate) fn weave_bezpath(pool: &mut List<DVec2>, paths: &mut List<Path>, bezpath: &BezPath) -> Vec<bool> {
	let mut weaver = PathWeaver::new(pool.len() as u32);

	for &element in bezpath.elements() {
		weaver.push_element(element);
	}

	weaver.finish(pool, paths)
}

/// Builds a fresh point pool and its paths from a bezpath.
pub fn pool_and_paths_from_bezpath(bezpath: &BezPath) -> (List<DVec2>, List<Path>) {
	let (mut pool, mut paths) = (List::new(), List::new());
	append_bezpath(&mut pool, &mut paths, bezpath);

	(pool, paths)
}

#[cfg(test)]
mod tests {
	use super::*;
	use kurbo::{Rect, Shape};

	fn pool(positions: &[(f64, f64)]) -> List<DVec2> {
		positions.iter().map(|&(x, y)| Item::new_from_element(DVec2::new(x, y))).collect()
	}

	fn path(references: &[u32]) -> Path {
		Path::from_references(references.iter().copied())
	}

	#[test]
	fn open_linear_path() {
		let pool = pool(&[(0., 0.), (10., 0.), (10., 10.)]);
		let path = path(&[0, 1, 2]);

		assert!(!path.is_closed());
		assert_eq!(path.segment_count(CurveType::Linear), 2);
		assert_eq!(path.segment(&pool, CurveType::Linear, 1), Some(PathSeg::Line(Line::new((10., 0.), (10., 10.)))));
		assert_eq!(path.segment(&pool, CurveType::Linear, 2), None);
	}

	// A closed path repeats its first reference last, so anchors and segments count alike
	#[test]
	fn closed_cubic_path() {
		let pool = pool(&[(0., 0.), (5., 0.), (10., 5.), (10., 10.), (10., 15.), (5., 20.)]);
		let path = path(&[0, 1, 2, 3, 4, 5, 0]);

		assert!(path.is_closed());
		assert_eq!(path.segment_count(CurveType::Cubic), 2);
		assert_eq!(
			path.segment(&pool, CurveType::Cubic, 1),
			Some(PathSeg::Cubic(CubicBez::new((10., 10.), (10., 15.), (5., 20.), (0., 0.))))
		);
	}

	// Turning around the middle of a path keeps every span's ID on the anchor it sets out from
	#[test]
	fn reversing_spans_where_they_sit() {
		let mut lasso = path(&[0, 1, 2, 3, 4, 1, 5]);
		let mut segment_id = SegmentId::ZERO;
		let ids: Vec<SegmentId> = (0..6).map(|_| segment_id.next_id()).collect();
		for (span, &id) in ids.iter().enumerate() {
			lasso.0.set_attribute(ATTR_ID, span, id);
		}

		lasso.reverse_spans(CurveType::Linear, 1..5);

		assert_eq!(lasso.0.iter_element_values().copied().collect::<Vec<_>>(), vec![0, 1, 4, 3, 2, 1, 5]);
		let span_ids: Vec<SegmentId> = (0..6).map(|span| lasso.0.attribute_cloned_or_default(ATTR_ID, span)).collect();
		assert_eq!(span_ids, [ids[0], ids[4], ids[3], ids[2], ids[1], ids[5]]);
	}

	// A partial segment trailing a path is left where it is, so anchors and handles keep their roles
	#[test]
	fn reversing_leaves_a_partial_tail_in_place() {
		let mut ragged = path(&[4, 10, 11, 0, 12, 13, 2, 14]);

		ragged.reverse_spans(CurveType::Cubic, 0..2);

		assert_eq!(ragged.0.iter_element_values().copied().collect::<Vec<_>>(), vec![2, 13, 12, 0, 11, 10, 4, 14]);
	}

	// A reference naming a point the pool does not have drops its segment instead of panicking
	#[test]
	fn dangling_reference_skips_its_segment() {
		let pool = pool(&[(0., 0.), (10., 0.)]);
		let path = path(&[0, 1, 99]);

		assert_eq!(path.segment_count(CurveType::Linear), 2);
		assert!(path.segment(&pool, CurveType::Linear, 0).is_some());
		assert_eq!(path.segment(&pool, CurveType::Linear, 1), None);
	}

	// Four corners draw four sides from five references, the last repeating the first to close the shape
	#[test]
	fn closed_rectangle() {
		let pool = pool(&[(-36.25, -17.5), (36.25, -17.5), (36.25, 17.5), (-36.25, 17.5)]);
		let mut paths = List::new();
		paths.push(Item::new_from_element(path(&[0, 1, 2, 3, 0])));
		paths.set_attribute(ATTR_CURVE_TYPE, 0, CurveType::Linear);

		let rectangle = paths.element(0).unwrap();
		assert!(rectangle.is_closed());
		assert_eq!(rectangle.segment_count(CurveType::Linear), 4);

		let closing_side = rectangle.segment(&pool, curve_type_at(&paths, 0), 3);
		assert_eq!(closing_side, Some(PathSeg::Line(Line::new((-36.25, 17.5), (-36.25, -17.5)))));
	}

	// Connectivity rides the holder, so a pool with a holder that declares no paths is a point cloud
	#[test]
	fn paths_ride_the_holder() {
		let mut item = Item::new_from_element(pool(&[(0., 0.), (10., 0.)]));
		assert!(paths_from_item_attributes(&item).is_none());

		let mut paths = List::new();
		paths.push(Item::new_from_element(path(&[0, 1])));
		item.set_attribute(ATTR_PATHS, paths);

		assert_eq!(paths_from_item_attributes(&item).map(List::len), Some(1));
		assert_eq!(curve_type_at(paths_from_item_attributes(&item).unwrap(), 0), CurveType::Linear);
	}

	// Two merged rectangles keep their own paths on their own items
	#[test]
	fn each_list_item_carries_its_own_paths() {
		let mut vectors = List::new();
		for _ in 0..2 {
			let mut paths = List::new();
			paths.push(Item::new_from_element(path(&[0, 1, 2, 3, 0])));
			vectors.push(Item::new_from_element(pool(&[(0., 0.), (1., 0.), (1., 1.), (0., 1.)])).with_attribute(ATTR_PATHS, paths));
		}

		assert_eq!(vectors.len(), 2);
		for index in 0..vectors.len() {
			let paths = paths_from_list_row_attributes(&vectors, index).unwrap();
			assert_eq!(paths.element(0).unwrap().segment_count(CurveType::Linear), 4);
		}
	}

	// A rectangle weaves to four pool points and five references, the fifth naming the first again
	#[test]
	fn weave_rectangle() {
		let (pool, paths) = pool_and_paths_from_bezpath(&Rect::new(-36.25, -17.5, 36.25, 17.5).to_path(0.));

		assert_eq!(pool.len(), 4);
		assert_eq!(paths.len(), 1);
		assert_eq!(curve_type_at(&paths, 0), CurveType::Linear);

		let rectangle = paths.element(0).unwrap();
		assert_eq!(rectangle.0.iter_element_values().copied().collect::<Vec<_>>(), vec![0, 1, 2, 3, 0]);
		assert!(rectangle.is_closed());
		assert_eq!(rectangle.segment_count(CurveType::Linear), 4);
	}

	// A closed contour landing back on its first point names that point rather than duplicating it
	#[test]
	fn weave_closed_cubic_contour() {
		let mut bezpath = BezPath::new();
		bezpath.move_to((0., 0.));
		bezpath.curve_to((5., 0.), (10., 5.), (10., 10.));
		bezpath.curve_to((5., 15.), (0., 5.), (0., 0.));
		bezpath.close_path();

		let (pool, paths) = pool_and_paths_from_bezpath(&bezpath);

		assert_eq!(pool.len(), 6);
		let contour = paths.element(0).unwrap();
		assert_eq!(contour.0.iter_element_values().copied().collect::<Vec<_>>(), vec![0, 1, 2, 3, 4, 5, 0]);
		assert!(contour.is_closed());
		assert_eq!(contour.segment_count(CurveType::Cubic), 2);
	}

	// Mixing verbs splits the subpath into one path per stride, welded where they meet
	#[test]
	fn weave_splits_at_verb_changes() {
		let mut bezpath = BezPath::new();
		bezpath.move_to((0., 0.));
		bezpath.line_to((10., 0.));
		bezpath.curve_to((15., 5.), (15., 10.), (10., 15.));
		bezpath.close_path();

		let (_, paths) = pool_and_paths_from_bezpath(&bezpath);

		assert_eq!(paths.len(), 3);
		assert_eq!(curve_type_at(&paths, 0), CurveType::Linear);
		assert_eq!(curve_type_at(&paths, 1), CurveType::Cubic);
		assert_eq!(curve_type_at(&paths, 2), CurveType::Linear);

		let references = |index: usize| paths.element(index).unwrap().0.iter_element_values().copied().collect::<Vec<_>>();
		assert_eq!(references(0), vec![0, 1]);
		assert_eq!(references(1), vec![1, 2, 3, 4]);
		assert_eq!(references(2), vec![4, 0]);
	}

	// A lone move leaves a free point in the pool and draws no path
	#[test]
	fn weave_lone_move_is_a_free_point() {
		let mut bezpath = BezPath::new();
		bezpath.move_to((5., 5.));

		let (pool, paths) = pool_and_paths_from_bezpath(&bezpath);

		assert_eq!(pool.len(), 1);
		assert!(paths.is_empty());
	}

	// A segment drawn straight after a close departs from the closed subpath's first point
	#[test]
	fn weave_continues_from_subpath_start_after_close() {
		let mut bezpath = BezPath::new();
		bezpath.move_to((0., 0.));
		bezpath.line_to((10., 0.));
		bezpath.close_path();
		bezpath.line_to((5., 5.));

		let (pool, paths) = pool_and_paths_from_bezpath(&bezpath);

		assert_eq!(pool.len(), 3);
		assert_eq!(paths.len(), 2);
		let references = |index: usize| paths.element(index).unwrap().0.iter_element_values().copied().collect::<Vec<_>>();
		assert_eq!(references(0), vec![0, 1, 0]);
		assert_eq!(references(1), vec![0, 2]);
	}
}
