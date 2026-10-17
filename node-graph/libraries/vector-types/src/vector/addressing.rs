use super::connectivity::Connectivity;
use super::misc::HandleId;
use super::path::{CurveType, Path, curve_type_at};
use super::vector_attributes::{PointId, SegmentId};
use core_types::list::{ATTR_ID, List};
use glam::DVec2;

/// A segment's place in the weave: the span of one path between two consecutive anchors.
/// Segments are never stored, so this is how one is named.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
	pub path: u32,
	pub index: u32,
}

impl Span {
	/// The reference index where this span's starting anchor sits.
	pub fn start_reference(&self, curve_type: CurveType) -> usize {
		self.index as usize * curve_type.stride()
	}

	/// The reference index where this span's ending anchor sits.
	pub fn end_reference(&self, curve_type: CurveType) -> usize {
		self.start_reference(curve_type) + curve_type.stride()
	}
}

/// Every span of every path, in path order.
pub fn spans(paths: &List<Path>) -> impl Iterator<Item = Span> + '_ {
	paths.iter_element_values().enumerate().flat_map(|(path_index, path)| {
		let curve_type = curve_type_at(paths, path_index);
		(0..path.segment_count(curve_type) as u32).map(move |index| Span { path: path_index as u32, index })
	})
}

/// The stable ID naming a span, stored on the anchor reference the span departs from. A reference cell left unnamed
/// holds `SegmentId::ZERO`, which sequential IDs never take.
pub fn segment_id_of(paths: &List<Path>, span: Span) -> Option<SegmentId> {
	let path = paths.element(span.path as usize)?;
	let reference = span.start_reference(curve_type_at(paths, span.path as usize));

	path.0.attribute::<SegmentId>(ATTR_ID, reference).copied()
}

/// The span a segment ID names, found by scanning every span in O(references).
pub fn span_of_segment_id(paths: &List<Path>, segment: SegmentId) -> Option<Span> {
	spans(paths).find(|&span| segment_id_of(paths, span) == Some(segment))
}

/// The pool points a span runs between.
pub fn span_endpoints(paths: &List<Path>, span: Span) -> Option<(u32, u32)> {
	let path = paths.element(span.path as usize)?;
	let curve_type = curve_type_at(paths, span.path as usize);
	let start = path.reference(span.start_reference(curve_type))?;
	let end = path.reference(span.end_reference(curve_type))?;

	Some((start, end))
}

/// The pool point a handle of the given span occupies, absent on a linear span that has no handles.
pub fn handle_point(paths: &List<Path>, span: Span, primary: bool) -> Option<u32> {
	let path = paths.element(span.path as usize)?;
	let curve_type = curve_type_at(paths, span.path as usize);
	if curve_type == CurveType::Linear {
		return None;
	}

	// A quadratic span's single handle answers to either side
	let offset = if primary || curve_type == CurveType::Quadratic { 1 } else { 2 };

	path.reference(span.start_reference(curve_type) + offset)
}

/// The stable ID of a pool point.
pub fn point_id_at(pool: &List<DVec2>, index: u32) -> Option<PointId> {
	pool.attribute::<PointId>(ATTR_ID, index as usize).copied()
}

/// The pool index a point ID names.
pub fn index_of_point_id(pool: &List<DVec2>, point: PointId) -> Option<u32> {
	pool.iter_attribute_values::<PointId>(ATTR_ID)?.position(|&id| id == point).map(|index| index as u32)
}

/// The spans meeting at a pool point in stored span order, each paired with whether the point is where that span ends.
pub fn spans_at(pool_index: u32, paths: &List<Path>, connectivity: &Connectivity) -> Vec<(Span, bool)> {
	let mut found = Vec::new();

	for reference in connectivity.references(pool_index) {
		let Some(path) = paths.element(reference.path as usize) else { continue };
		if !reference.is_anchor() {
			continue;
		}
		let curve_type = reference.curve_type;

		// An anchor arrives on the span before it and departs the one starting at it
		let anchor = reference.index as usize / curve_type.stride();
		if let Some(previous) = anchor.checked_sub(1) {
			found.push((
				Span {
					path: reference.path,
					index: previous as u32,
				},
				true,
			));
		}
		if anchor < path.segment_count(curve_type) {
			found.push((
				Span {
					path: reference.path,
					index: anchor as u32,
				},
				false,
			));
		}
	}

	found
}

/// The segments departing the given pool point.
pub fn start_connected(pool_index: u32, paths: &List<Path>, connectivity: &Connectivity) -> Vec<SegmentId> {
	spans_at(pool_index, paths, connectivity)
		.into_iter()
		.filter(|&(_, arriving)| !arriving)
		.filter_map(|(span, _)| segment_id_of(paths, span))
		.collect()
}

/// The segments arriving at the given pool point.
pub fn end_connected(pool_index: u32, paths: &List<Path>, connectivity: &Connectivity) -> Vec<SegmentId> {
	spans_at(pool_index, paths, connectivity)
		.into_iter()
		.filter(|&(_, arriving)| arriving)
		.filter_map(|(span, _)| segment_id_of(paths, span))
		.collect()
}

/// The handles of every segment meeting the given pool point, naming the side that faces it, with departing segments first.
pub fn all_connected(pool_index: u32, paths: &List<Path>, connectivity: &Connectivity) -> Vec<HandleId> {
	let (arriving, departing): (Vec<_>, Vec<_>) = spans_at(pool_index, paths, connectivity).into_iter().partition(|&(_, arriving)| arriving);

	departing
		.into_iter()
		.chain(arriving)
		.filter_map(|(span, arriving)| {
			let segment = segment_id_of(paths, span)?;
			Some(if arriving { HandleId::end(segment) } else { HandleId::primary(segment) })
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::list::{ATTR_CURVE_TYPE, Item};

	fn pool(count: u32) -> List<DVec2> {
		(0..count)
			.map(|index| Item::new_from_element(DVec2::new(index as f64, 0.)).with_attribute(ATTR_ID, PointId::generate()))
			.collect()
	}

	fn paths_of(specs: &[(CurveType, &[u32])]) -> List<Path> {
		let mut paths = List::new();

		for &(curve_type, references) in specs {
			let mut path = Path::from_references(references.iter().copied());

			// Name each span on the anchor it departs from
			for span in 0..path.segment_count(curve_type) {
				path.0.set_attribute(ATTR_ID, span * curve_type.stride(), SegmentId::generate());
			}

			paths.push(Item::new_from_element(path).with_attribute(ATTR_CURVE_TYPE, curve_type));
		}

		paths
	}

	#[test]
	fn spans_name_every_segment() {
		let paths = paths_of(&[(CurveType::Linear, &[0, 1, 2]), (CurveType::Cubic, &[2, 3, 4, 5])]);

		let all: Vec<_> = spans(&paths).collect();
		assert_eq!(all.len(), 3);
		assert_eq!(all[2], Span { path: 1, index: 0 });

		for span in all {
			let id = segment_id_of(&paths, span).expect("every span was named");
			assert_eq!(span_of_segment_id(&paths, id), Some(span));
		}
	}

	#[test]
	fn spans_resolve_their_endpoints_and_handles() {
		let paths = paths_of(&[(CurveType::Cubic, &[0, 1, 2, 3])]);
		let span = Span { path: 0, index: 0 };

		assert_eq!(span_endpoints(&paths, span), Some((0, 3)));
		assert_eq!(handle_point(&paths, span, true), Some(1));
		assert_eq!(handle_point(&paths, span, false), Some(2));

		let linear = paths_of(&[(CurveType::Linear, &[0, 1])]);
		assert_eq!(handle_point(&linear, Span { path: 0, index: 0 }, true), None);
	}

	// A welded anchor reports the segments on both sides, each from the path that owns it
	#[test]
	fn connectivity_by_segment_id() {
		let paths = paths_of(&[(CurveType::Linear, &[0, 1]), (CurveType::Linear, &[1, 2])]);
		let connectivity = Connectivity::build(3, &paths);

		assert_eq!(start_connected(0, &paths, &connectivity).len(), 1);
		assert!(end_connected(0, &paths, &connectivity).is_empty());

		assert_eq!(start_connected(1, &paths, &connectivity).len(), 1);
		assert_eq!(end_connected(1, &paths, &connectivity).len(), 1);
		assert_eq!(all_connected(1, &paths, &connectivity).len(), 2);
	}

	// The tools take the first connected handle, so it must not depend on whether the anchor welds two paths or sits along one
	#[test]
	fn connected_handles_lead_with_departing_segments() {
		let paths = paths_of(&[(CurveType::Linear, &[0, 1]), (CurveType::Cubic, &[1, 2, 3, 4, 5, 6, 7])]);
		let connectivity = Connectivity::build(8, &paths);
		let segment = |path, index| segment_id_of(&paths, Span { path, index }).expect("every span was named");

		let welded = all_connected(1, &paths, &connectivity);
		assert_eq!(welded, [HandleId::primary(segment(1, 0)), HandleId::end(segment(0, 0))]);

		let along_path = all_connected(4, &paths, &connectivity);
		assert_eq!(along_path, [HandleId::primary(segment(1, 1)), HandleId::end(segment(1, 0))]);

		// The spans themselves stay in stored order
		let spans = spans_at(4, &paths, &connectivity);
		assert_eq!(spans, [(Span { path: 1, index: 0 }, true), (Span { path: 1, index: 1 }, false)]);
	}

	#[test]
	fn point_ids_resolve_both_ways() {
		let pool = pool(4);

		for index in 0..4 {
			let id = point_id_at(&pool, index).expect("every point was named");
			assert_eq!(index_of_point_id(&pool, id), Some(index));
		}
	}
}
