use core_types::list::{ATTR_APPEARANCE, Item, List};
use graphic_types::{Appearance, Cover, Vector};
use kurbo::{BezPath, PathEl, Shape};
use linesweeper::topology::{OutputSegIdx, Topology};

pub fn nonzero_vectors(vector: Item<Vector>, geometry: &BezPath) -> List<Vector> {
	let appearance = vector.attribute::<Appearance>(ATTR_APPEARANCE).cloned().unwrap_or_default();
	let Some(region) = appearance.has_cover(Cover::Fill).then(|| even_odd_region(geometry)).flatten() else {
		return List::new_from_item(vector);
	};

	let only = |item: Item<Vector>, cover| {
		let mut appearance = appearance.clone();
		appearance.retain_cover(cover);
		item.with_attribute(ATTR_APPEARANCE, appearance)
	};
	let fill = only(Vector::item_from_bezpath(&region), Cover::Fill);
	if !appearance.has_cover(Cover::Stroke) {
		return List::new_from_item(fill);
	}
	let stroke = only(vector, Cover::Stroke);
	let paint_order = if appearance.fill_and_stroke().stroke_below { [stroke, fill] } else { [fill, stroke] };
	paint_order.into_iter().collect()
}

fn even_odd_region(geometry: &BezPath) -> Option<BezPath> {
	let bounds = geometry.bounding_box();
	let magnitude = [bounds.x0, bounds.y0, bounds.x1, bounds.y1].into_iter().map(f64::abs).fold(0., f64::max);
	let topology = Topology::from_path(&closed_subpaths(geometry), (magnitude * f64::EPSILON * 64.).max(1e-6)).ok()?;

	let rules_differ = (topology.segment_indices().map(OutputSegIdx::first_half))
		.flat_map(|half| [*topology.winding_clockwise(half), *topology.winding_counter_clockwise(half)])
		.any(|winding| winding != 0 && winding % 2 == 0);
	rules_differ.then(|| topology.contours(|winding| winding % 2 != 0).into_contours().flat_map(|contour| contour.path).collect())
}

fn closed_subpaths(geometry: &BezPath) -> BezPath {
	let ends_closed = |path: &BezPath| matches!(path.elements().last(), None | Some(PathEl::ClosePath));
	let mut closed = BezPath::new();
	for element in geometry {
		if matches!(element, PathEl::MoveTo(_)) && !ends_closed(&closed) {
			closed.close_path();
		}
		closed.push(element);
	}
	if !ends_closed(&closed) {
		closed.close_path();
	}
	closed
}
