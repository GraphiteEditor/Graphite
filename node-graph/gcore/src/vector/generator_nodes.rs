use super::HandleId;
use crate::vector::{PointId, VectorData};

use bezier_rs::Subpath;

use glam::DVec2;

#[node_macro::node(category("Vector: Generator"))]
fn circle(_: (), _input: (), #[default(50.)] radius: f64) -> VectorData {
	let radius: f64 = radius;
	super::VectorData::from_subpath(Subpath::new_ellipse(DVec2::splat(-radius), DVec2::splat(radius)))
}

#[node_macro::node(category("Vector: Generator"))]
fn ellipse(_: (), _input: (), #[default(50)] radius_x: f64, #[default(25)] radius_y: f64) -> VectorData {
	let radius = DVec2::new(radius_x, radius_y);
	let corner1 = -radius;
	let corner2 = radius;
	let mut ellipse = super::VectorData::from_subpath(Subpath::new_ellipse(corner1, corner2));
	let len = ellipse.segment_domain.ids().len();
	for i in 0..len {
		ellipse
			.colinear_manipulators
			.push([HandleId::end(ellipse.segment_domain.ids()[i]), HandleId::primary(ellipse.segment_domain.ids()[(i + 1) % len])]);
	}
	ellipse
}

trait CornerRadius {
	fn generate(self, size: DVec2, clamped: bool) -> super::VectorData;
}
impl CornerRadius for f64 {
	fn generate(self, size: DVec2, clamped: bool) -> super::VectorData {
		let clamped_radius = if clamped { self.clamp(0., size.x.min(size.y).max(0.) / 2.) } else { self };
		super::VectorData::from_subpath(Subpath::new_rounded_rect(size / -2., size / 2., [clamped_radius; 4]))
	}
}
impl CornerRadius for [f64; 4] {
	fn generate(self, size: DVec2, clamped: bool) -> super::VectorData {
		let clamped_radius = if clamped {
			// Algorithm follows the CSS spec: <https://drafts.csswg.org/css-backgrounds/#corner-overlap>

			let mut scale_factor: f64 = 1.;
			for i in 0..4 {
				let side_length = if i % 2 == 0 { size.x } else { size.y };
				let adjacent_corner_radius_sum = self[i] + self[(i + 1) % 4];
				if side_length < adjacent_corner_radius_sum {
					scale_factor = scale_factor.min(side_length / adjacent_corner_radius_sum);
				}
			}
			self.map(|x| x * scale_factor)
		} else {
			self
		};
		super::VectorData::from_subpath(Subpath::new_rounded_rect(size / -2., size / 2., clamped_radius))
	}
}

#[node_macro::node(category("Vector: Generator"))]
fn rectangle<T: CornerRadius>(
	_: (),
	_input: (),
	#[default(100)] size_x: f64,
	#[default(100)] size_y: f64,
	_is_individual: bool,
	#[implementations(f64, [f64; 4])] corner_radius: T,
	#[default(true)] clamped: bool,
) -> VectorData {
	corner_radius.generate(DVec2::new(size_x, size_y), clamped)
}

#[node_macro::node(category("Vector: Generator"))]
fn regular_polygon(
	_: (),
	_input: (),
	#[default(6)]
	#[min(3.)]
	sides: u32,
	#[default(50)] radius: f64,
) -> VectorData {
	let points = sides.into();
	let radius: f64 = radius * 2.;
	super::VectorData::from_subpath(Subpath::new_regular_polygon(DVec2::splat(-radius), points, radius))
}

#[node_macro::node(category("Vector: Generator"))]
fn star(
	_: (),
	_input: (),
	#[default(5)]
	#[min(2.)]
	sides: u32,
	#[default(50)] radius: f64,
	#[default(25)] inner_radius: f64,
) -> VectorData {
	let points = sides.into();
	let diameter: f64 = radius * 2.;
	let inner_diameter = inner_radius * 2.;

	super::VectorData::from_subpath(Subpath::new_star_polygon(DVec2::splat(-diameter), points, diameter, inner_diameter))
}

#[node_macro::node(category("Vector: Generator"))]
fn line(_: (), _input: (), #[default((0., -50.))] start: DVec2, #[default((0., 50.))] end: DVec2) -> VectorData {
	super::VectorData::from_subpath(Subpath::new_line(start, end))
}

#[node_macro::node(category("Vector: Generator"))]
fn spline(_: (), _input: (), positions: Vec<DVec2>) -> VectorData {
	let mut spline = super::VectorData::from_subpath(Subpath::new_cubic_spline(positions));
	for pair in spline.segment_domain.ids().windows(2) {
		spline.colinear_manipulators.push([HandleId::end(pair[0]), HandleId::primary(pair[1])]);
	}
	spline
}

// TODO(TrueDoctor): I removed the Arc requirement we should think about when it makes sense to use it vs making a generic value node

#[node_macro::node(category("Vector: Generator"))]
fn path(_: (), path_data: Vec<Subpath<PointId>>, colinear_manipulators: Vec<PointId>) -> super::VectorData {
	let mut vector_data = super::VectorData::from_subpaths(path_data, false);
	vector_data.colinear_manipulators = colinear_manipulators
		.iter()
		.filter_map(|&point| super::ManipulatorPointId::Anchor(point).get_handle_pair(&vector_data))
		.collect();
	vector_data
}

// #[derive(Debug, Clone, Copy)]
// pub struct BlitSubpath<P> {
// 	path_data: P,
// }

// #[node_macro::node_fn(BlitSubpath)]
// fn blit_subpath(base_image: Image, path_data: VectorData) -> Image {
// 	// TODO: Get forma to compile
// 	use forma::prelude::*;
// 	let composition = Composition::new();
// 	let mut renderer = cpu::Renderer::new();
// 	let mut path_builder = PathBuilder::new();
// 	for path_segment in path_data.bezier_iter() {
// 		let points = path_segment.internal.get_points().collect::<Vec<_>>();
// 		match points.len() {
// 			2 => path_builder.line_to(points[1].into()),
// 			3 => path_builder.quad_to(points[1].into(), points[2].into()),
// 			4 => path_builder.cubic_to(points[1].into(), points[2].into(), points[3].into()),
// 		}
// 	}

// 	base_image
// }
