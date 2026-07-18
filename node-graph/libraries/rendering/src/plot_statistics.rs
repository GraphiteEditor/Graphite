use kurbo::{BezPath, ParamCurveArclen, PathEl, PathSeg, Point, Rect, Shape};

/// Pen movement totals for plotting an SVG document with a pen plotter, which draws every path as its outline.
/// Distances are in SVG user units; scale by the paper fit before converting to wall-clock time.
///
/// Pen-up travel between subpaths is deliberately not measured: the print server reorders paths (and rotates the
/// start points of closed ones) to minimize travel, so document-order travel distance is meaningless. Its cost is
/// captured as part of the constant time per pen lift instead.
pub struct PlotStatistics {
	/// Total distance drawn with the pen down, following every path's geometry.
	pub pen_down_distance: f64,
	/// Number of pen lift/reposition/lower cycles (one per subpath).
	pub pen_lift_count: usize,
	/// Width of the artwork's bounding box, which the print server scales to fit the paper (the document size is ignored).
	pub width: f64,
	/// Height of the artwork's bounding box.
	pub height: f64,
}

#[derive(Default)]
struct MeasureState {
	pen_down_distance: f64,
	pen_lift_count: usize,
	bounds: Option<Rect>,
}

/// Measures the pen plotter movement statistics of an SVG document by walking every path in document order.
/// Returns `None` if the SVG cannot be parsed or contains no path geometry.
pub fn svg_plot_statistics(svg: &str) -> Option<PlotStatistics> {
	let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;

	let mut state = MeasureState::default();
	accumulate_group(tree.root(), &mut state);

	let bounds = state.bounds?;
	Some(PlotStatistics {
		pen_down_distance: state.pen_down_distance,
		pen_lift_count: state.pen_lift_count,
		width: bounds.width(),
		height: bounds.height(),
	})
}

fn accumulate_group(group: &usvg::Group, state: &mut MeasureState) {
	for node in group.children() {
		match node {
			usvg::Node::Group(group) => accumulate_group(group, state),
			usvg::Node::Path(path) => accumulate_bezpath(&usvg_path_to_bezpath(path), state),
			_ => {}
		}
	}
}

fn accumulate_bezpath(bezpath: &BezPath, state: &mut MeasureState) {
	const ARC_LENGTH_ACCURACY: f64 = 0.1;

	if bezpath.elements().is_empty() {
		return;
	}

	for segment in bezpath.segments() {
		state.pen_down_distance += match segment {
			PathSeg::Line(line) => line.p0.distance(line.p1),
			segment => segment.arclen(ARC_LENGTH_ACCURACY),
		};
	}

	state.pen_lift_count += bezpath.elements().iter().filter(|element| matches!(element, PathEl::MoveTo(_))).count();

	let bounds = bezpath.bounding_box();
	if !bounds.is_nan() {
		state.bounds = Some(state.bounds.map_or(bounds, |existing| existing.union(bounds)));
	}
}

/// Converts a usvg path into a kurbo path with its absolute transform applied.
fn usvg_path_to_bezpath(path: &usvg::Path) -> BezPath {
	let transform = path.abs_transform();
	let to_point = |point: &usvg::tiny_skia_path::Point| {
		let (x, y) = (point.x as f64, point.y as f64);
		Point::new(
			transform.sx as f64 * x + transform.kx as f64 * y + transform.tx as f64,
			transform.ky as f64 * x + transform.sy as f64 * y + transform.ty as f64,
		)
	};

	let mut bezpath = BezPath::new();
	let mut points = path.data().points().iter();

	for verb in path.data().verbs() {
		match verb {
			usvg::tiny_skia_path::PathVerb::Move => {
				let Some(point) = points.next().map(to_point) else { continue };
				bezpath.move_to(point);
			}
			usvg::tiny_skia_path::PathVerb::Line => {
				let Some(end) = points.next().map(to_point) else { continue };
				bezpath.line_to(end);
			}
			usvg::tiny_skia_path::PathVerb::Quad => {
				let Some(handle) = points.next().map(to_point) else { continue };
				let Some(end) = points.next().map(to_point) else { continue };
				bezpath.quad_to(handle, end);
			}
			usvg::tiny_skia_path::PathVerb::Cubic => {
				let Some(first_handle) = points.next().map(to_point) else { continue };
				let Some(second_handle) = points.next().map(to_point) else { continue };
				let Some(end) = points.next().map(to_point) else { continue };
				bezpath.curve_to(first_handle, second_handle, end);
			}
			usvg::tiny_skia_path::PathVerb::Close => bezpath.close_path(),
		}
	}

	bezpath
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn measures_lines_squares_and_bounds() {
		// A 30x30 square (120 units drawn, 1 lift) followed by a vertical line (100 units drawn, 1 lift),
		// with an artwork bounding box spanning (10,10) to (40,150)
		let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 200" width="100" height="200">
			<path d="M10,10 L40,10 L40,40 L10,40 Z" fill="black" />
			<path d="M10,50 L10,150" stroke="black" fill="none" />
		</svg>"##;

		let statistics = svg_plot_statistics(svg).unwrap();

		assert_eq!(statistics.pen_lift_count, 2);
		assert!((statistics.pen_down_distance - 220.).abs() < 1e-6, "pen down was {}", statistics.pen_down_distance);
		assert!((statistics.width - 30.).abs() < 1e-6, "width was {}", statistics.width);
		assert!((statistics.height - 140.).abs() < 1e-6, "height was {}", statistics.height);
	}
}
