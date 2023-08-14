use crate::uuid::ManipulatorGroupId;
use crate::vector::VectorData;
use crate::Node;

use bezier_rs::Subpath;

use glam::DVec2;

#[derive(Debug, Clone, Copy)]
pub struct UnitCircleGenerator<Radius> {
	radius: Radius,
}

#[node_macro::node_fn(UnitCircleGenerator)]
fn unit_circle(_input: (), radius: f32) -> VectorData {
	let radius = radius.into();
	super::VectorData::from_subpath(Subpath::new_ellipse(DVec2::splat(radius * -1.), DVec2::splat(radius)))
}

#[derive(Debug, Clone, Copy)]
pub struct UnitRectangleGenerator<SizeX, SizeY> {
	size_x: SizeX,
	size_y: SizeY,
}

#[node_macro::node_fn(UnitRectangleGenerator)]
fn unit_square(_input: (), size_x: f32, size_y: f32) -> VectorData {
	let size_x: f64 = size_x.into();
	let size_y: f64 = size_y.into();
	let corner1 = DVec2::new(size_x * -1., size_y * -1.);
	let corner2 = DVec2::new(size_x, size_y);

	super::VectorData::from_subpaths(vec![Subpath::new_rect(corner1, corner2)])
}

#[derive(Debug, Clone, Copy)]
pub struct UnitPolygonGenerator<Points, Radius> {
	points: Points,
	radius: Radius,
}

#[node_macro::node_fn(UnitPolygonGenerator)]
fn unit_polygon(_input: (), points: u32, radius: f32) -> VectorData {
	let points = points.into();
	let radius = radius.into();
	super::VectorData::from_subpath(Subpath::new_regular_polygon(DVec2::splat(radius * -1.), points, radius))
}

#[derive(Debug, Clone, Copy)]
pub struct UnitStarGenerator<Points, Radius, InnerRadius> {
	points: Points,
	radius: Radius,
	inner_radius: InnerRadius,
}

#[node_macro::node_fn(UnitStarGenerator)]
fn unit_star(_input: (), points: u32, radius: f32, inner_radius: f32) -> VectorData {
	let points: u64 = points.into();
	let radius: f64 = radius.into();
	let inner_radius: f64 = inner_radius.into();

	super::VectorData::from_subpath(Subpath::new_regular_star_polygon(DVec2::splat(radius * -1.), points, radius, inner_radius))
}

#[derive(Debug, Clone, Copy)]
pub struct UnitLineGenerator<Pos1, Pos2> {
	pos_1: Pos1,
	pos_2: Pos2,
}

#[node_macro::node_fn(UnitLineGenerator)]
fn unit_line(_input: (), pos_1: DVec2, pos_2: DVec2) -> VectorData {
	super::VectorData::from_subpaths(vec![Subpath::new_line(pos_1, pos_2)])
}

#[derive(Debug, Clone, Copy)]
pub struct UnitSplineGenerator<Pos1, Pos2, Pos3> {
	pos_1: Pos1,
	pos_2: Pos2,
	pos_3: Pos3,
}

#[node_macro::node_fn(UnitSplineGenerator)]
fn unit_spline(_input: (), pos_1: DVec2, pos_2: DVec2, pos_3: DVec2) -> VectorData {
	super::VectorData::from_subpaths(vec![Subpath::new_cubic_spline(vec![pos_1, pos_2, pos_3])])
}

// TODO(TrueDoctor): I removed the Arc requirement we should think about when it makes sense to use it vs making a generic value node
#[derive(Debug, Clone)]
pub struct PathGenerator<Mirror> {
	mirror: Mirror,
}

#[node_macro::node_fn(PathGenerator)]
fn generate_path(path_data: Vec<Subpath<ManipulatorGroupId>>, mirror: Vec<ManipulatorGroupId>) -> super::VectorData {
	let mut vector_data = super::VectorData::from_subpaths(path_data);
	vector_data.mirror_angle = mirror;
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
