use super::HandleId;
use crate::vector::{PointId, VectorData};
use crate::Node;
use bezier_rs::Subpath;
use glam::DVec2;

#[derive(Debug, Clone, Copy)]
pub struct CircleGenerator<Radius> {
	radius: Radius,
}

#[node_macro::node_fn(CircleGenerator)]
fn circle_generator(_input: (), radius: f64) -> VectorData {
	let radius: f64 = radius;
	super::VectorData::from_subpath(Subpath::new_ellipse(DVec2::splat(-radius), DVec2::splat(radius)))
}

#[derive(Debug, Clone, Copy)]
pub struct EllipseGenerator<RadiusX, RadiusY> {
	radius_x: RadiusX,
	radius_y: RadiusY,
}

#[node_macro::node_fn(EllipseGenerator)]
fn ellipse_generator(_input: (), radius_x: f64, radius_y: f64) -> VectorData {
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

#[derive(Debug, Clone, Copy)]
pub struct RectangleGenerator<SizeX, SizeY, IsIndividual, CornerRadius, Clamped> {
	size_x: SizeX,
	size_y: SizeY,
	is_individual: IsIndividual,
	corner_radius: CornerRadius,
	clamped: Clamped,
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

#[node_macro::node_fn(RectangleGenerator)]
fn square_generator<T: CornerRadius>(_input: (), size_x: f64, size_y: f64, is_individual: bool, corner_radius: T, clamped: bool) -> VectorData {
	corner_radius.generate(DVec2::new(size_x, size_y), clamped)
}

#[derive(Debug, Clone, Copy)]
pub struct RegularPolygonGenerator<Points, Radius> {
	points: Points,
	radius: Radius,
}

#[node_macro::node_fn(RegularPolygonGenerator)]
fn regular_polygon_generator(_input: (), points: u32, radius: f64) -> VectorData {
	let points = points.into();
	let radius: f64 = radius * 2.;
	super::VectorData::from_subpath(Subpath::new_regular_polygon(DVec2::splat(-radius), points, radius))
}

#[derive(Debug, Clone, Copy)]
pub struct StarGenerator<Points, Radius, InnerRadius> {
	points: Points,
	radius: Radius,
	inner_radius: InnerRadius,
}

#[node_macro::node_fn(StarGenerator)]
fn star_generator(_input: (), points: u32, radius: f64, inner_radius: f64) -> VectorData {
	let points = points.into();
	let diameter: f64 = radius * 2.;
	let inner_diameter = inner_radius * 2.;

	super::VectorData::from_subpath(Subpath::new_star_polygon(DVec2::splat(-diameter), points, diameter, inner_diameter))
}

#[derive(Debug, Clone, Copy)]
pub struct LineGenerator<Pos1, Pos2> {
	pos_1: Pos1,
	pos_2: Pos2,
}

#[node_macro::node_fn(LineGenerator)]
fn line_generator(_input: (), pos_1: DVec2, pos_2: DVec2) -> VectorData {
	super::VectorData::from_subpath(Subpath::new_line(pos_1, pos_2))
}

#[derive(Debug, Clone, Copy)]
pub struct SplineGenerator<Positions> {
	positions: Positions,
}

#[node_macro::node_fn(SplineGenerator)]
fn spline_generator(_input: (), positions: Vec<DVec2>) -> VectorData {
	let mut spline = super::VectorData::from_subpath(Subpath::new_cubic_spline(positions));
	for pair in spline.segment_domain.ids().windows(2) {
		spline.colinear_manipulators.push([HandleId::end(pair[0]), HandleId::primary(pair[1])]);
	}
	spline
}

// TODO(TrueDoctor): I removed the Arc requirement we should think about when it makes sense to use it vs making a generic value node
#[derive(Debug, Clone)]
pub struct PathGenerator<ColinearManipulators> {
	colinear_manipulators: ColinearManipulators,
}

#[node_macro::node_fn(PathGenerator)]
fn generate_path(path_data: Vec<Subpath<PointId>>, colinear_manipulators: Vec<[HandleId; 2]>) -> super::VectorData {
	let mut vector_data = super::VectorData::from_subpaths(path_data, false);
	vector_data.colinear_manipulators = colinear_manipulators;
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
