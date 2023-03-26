use crate::uuid::ManipulatorGroupId;
use crate::vector::VectorData;
use crate::Node;

use bezier_rs::Subpath;
use glam::DVec2;

pub struct UnitCircleGenerator;

#[node_macro::node_fn(UnitCircleGenerator)]
fn unit_circle(_input: ()) -> VectorData {
	super::VectorData::from_subpath(Subpath::new_ellipse(DVec2::ZERO, DVec2::ONE))
}

#[derive(Debug, Clone, Copy)]
pub struct UnitSquareGenerator;

#[node_macro::node_fn(UnitSquareGenerator)]
fn unit_square(_input: ()) -> VectorData {
	super::VectorData::from_subpaths(vec![Subpath::new_ellipse(DVec2::ZERO, DVec2::ONE)])
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
