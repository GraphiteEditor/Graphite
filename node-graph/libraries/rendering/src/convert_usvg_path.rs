use kurbo::{BezPath, Point};

pub fn convert_usvg_path(path: &usvg::Path) -> BezPath {
	let mut bezpath = BezPath::new();

	let mut points = path.data().points().iter();
	let to_point = |p: &usvg::tiny_skia_path::Point| Point::new(p.x as f64, p.y as f64);

	for verb in path.data().verbs() {
		match verb {
			usvg::tiny_skia_path::PathVerb::Move => {
				let Some(start) = points.next().map(to_point) else { continue };
				bezpath.move_to(start);
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
