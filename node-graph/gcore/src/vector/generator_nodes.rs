use crate::{Node};
use glam::{DAffine2, DVec2};

use super::subpath::Subpath;

type VectorData = Subpath;

pub struct UnitCircleGenerator;

#[node_macro::node_fn(UnitCircleGenerator)]
fn unit_circle(_input: ()) -> VectorData {
	Subpath::new_ellipse(DVec2::ZERO, DVec2::ONE)
}

#[derive(Debug, Clone, Copy)]
pub struct UnitSquareGenerator;

#[node_macro::node_fn(UnitSquareGenerator)]
fn unit_square(_input: ()) -> VectorData {
	Subpath::new_rect(DVec2::ZERO, DVec2::ONE)
}

// TODO: I removed the Arc requirement we shouuld think about when it makes sense to use its
// vs making a generic value node
#[derive(Debug, Clone)]
pub struct PathGenerator<P> {
	path_data: P,
}

#[node_macro::node_fn(PathGenerator)]
fn generate_path(_input: (), path_data: Subpath) -> VectorData {
	path_data
}

use crate::raster::Image;

#[derive(Debug, Clone, Copy)]
pub struct BlitSubpath<P> {
	path_data: P,
}

#[node_macro::node_fn(BlitSubpath)]
fn bilt_subpath(base_image: Image, path_data: Subpath) -> Image {
	log::info!("Blitting subpath {path_data:#?}");
	// TODO: Get forma to compile
	/*use forma::prelude::*;
	let composition = Composition::new();
	let mut renderer = cpu::Renderer::new();
	let mut path_builder = PathBuilder::new();
	for path_segment in path_data.bezier_iter() {
		let points = path_segment.internal.get_points().collect::<Vec<_>>();
		match points.len() {
			2 => path_builder.line_to(points[1].into()),
			3 => path_builder.quad_to(points[1].into(), points[2].into()),
			4 => path_builder.cubic_to(points[1].into(), points[2].into(), points[3].into()),
		}
	}*/

	base_image
}

#[derive(Debug, Clone, Copy)]
pub struct TransformSubpathNode<Translation, Rotation, Scale, Shear> {
	translate: Translation,
	rotate: Rotation,
	scale: Scale,
	shear: Shear,
}

#[node_macro::node_fn(TransformSubpathNode)]
fn transform_subpath(subpath: Subpath, translate: DVec2, rotate: f64, scale: DVec2, shear: DVec2) -> VectorData {
	let (sin, cos) = rotate.sin_cos();

	let mut subpath = subpath;
	subpath.apply_affine(DAffine2::from_cols_array(&[scale.x + cos, shear.y + sin, shear.x - sin, scale.y + cos, translate.x, translate.y]));
	subpath
}
