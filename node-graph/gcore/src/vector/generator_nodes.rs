use crate::Node;
use glam::{DAffine2, DVec2};

use super::subpath::Subpath;

type VectorData = Subpath;

pub struct UnitCircleGenerator;

impl Node<()> for UnitCircleGenerator {
	type Output = VectorData;
	fn eval(self, input: ()) -> Self::Output {
		(&self).eval(input)
	}
}

impl Node<()> for &UnitCircleGenerator {
	type Output = VectorData;
	fn eval(self, _input: ()) -> Self::Output {
		Subpath::new_ellipse(DVec2::ZERO, DVec2::ONE)
	}
}

#[derive(Debug, Clone, Copy)]
pub struct UnitSquareGenerator;

impl Node<()> for UnitSquareGenerator {
	type Output = VectorData;
	fn eval(self, input: ()) -> Self::Output {
		(&self).eval(input)
	}
}

impl Node<()> for &UnitSquareGenerator {
	type Output = VectorData;
	fn eval(self, _input: ()) -> Self::Output {
		Subpath::new_rect(DVec2::ZERO, DVec2::ONE)
	}
}

// TODO: I removed the Arc requirement we shouuld think about when it makes sense to use its
// vs making a generic value node
#[derive(Debug, Clone)]
pub struct PathGenerator(Subpath);

impl Node<()> for PathGenerator {
	type Output = VectorData;
	fn eval(self, input: ()) -> Self::Output {
		(&self).eval(input)
	}
}

impl Node<()> for &PathGenerator {
	type Output = VectorData;
	fn eval(self, _input: ()) -> Self::Output {
		(self.0).clone()
	}
}
use crate::raster::Image;

#[derive(Debug, Clone, Copy)]
pub struct BlitSubpath<N: Node<(), Output = Subpath>>(N);

impl<N: Node<(), Output = Subpath>> Node<Image> for BlitSubpath<N> {
	type Output = Image;
	fn eval(self, input: Image) -> Self::Output {
		let subpath = self.0.eval(());
		log::info!("Blitting subpath {subpath:?}");
		input
	}
}

impl<N: Node<(), Output = Subpath> + Copy> Node<Image> for &BlitSubpath<N> {
	type Output = Image;
	fn eval(self, input: Image) -> Self::Output {
		let subpath = self.0.eval(());
		log::info!("Blitting subpath {subpath:?}");
		input
	}
}

impl<N: Node<(), Output = Subpath>> BlitSubpath<N> {
	pub fn new(node: N) -> Self {
		Self(node)
	}
}

#[derive(Debug, Clone, Copy)]
pub struct TransformSubpathNode<Translation, Rotation, Scale, Shear>
where
	Translation: Node<(), Output = DVec2>,
	Rotation: Node<(), Output = f64>,
	Scale: Node<(), Output = DVec2>,
	Shear: Node<(), Output = DVec2>,
{
	translate_node: Translation,
	rotate_node: Rotation,
	scale_node: Scale,
	shear_node: Shear,
}

impl<Translation, Rotation, Scale, Shear> TransformSubpathNode<Translation, Rotation, Scale, Shear>
where
	Translation: Node<(), Output = DVec2>,
	Rotation: Node<(), Output = f64>,
	Scale: Node<(), Output = DVec2>,
	Shear: Node<(), Output = DVec2>,
{
	pub fn new(translate_node: Translation, rotate_node: Rotation, scale_node: Scale, shear_node: Shear) -> Self {
		Self {
			translate_node,
			rotate_node,
			scale_node,
			shear_node,
		}
	}
}

impl<Translation, Rotation, Scale, Shear> Node<Subpath> for TransformSubpathNode<Translation, Rotation, Scale, Shear>
where
	Translation: Node<(), Output = DVec2>,
	Rotation: Node<(), Output = f64>,
	Scale: Node<(), Output = DVec2>,
	Shear: Node<(), Output = DVec2>,
{
	type Output = Subpath;
	fn eval(self, mut subpath: Subpath) -> Subpath {
		let translate = self.translate_node.eval(());
		let rotate = self.rotate_node.eval(());
		let scale = self.scale_node.eval(());
		let shear = self.shear_node.eval(());

		let (sin, cos) = rotate.sin_cos();

		subpath.apply_affine(DAffine2::from_cols_array(&[scale.x + cos, shear.y + sin, shear.x - sin, scale.y + cos, translate.x, translate.y]));
		subpath
	}
}
impl<Translation, Rotation, Scale, Shear> Node<Subpath> for &TransformSubpathNode<Translation, Rotation, Scale, Shear>
where
	Translation: Node<(), Output = DVec2> + Copy,
	Rotation: Node<(), Output = f64> + Copy,
	Scale: Node<(), Output = DVec2> + Copy,
	Shear: Node<(), Output = DVec2> + Copy,
{
	type Output = Subpath;
	fn eval(self, mut subpath: Subpath) -> Subpath {
		let translate = self.translate_node.eval(());
		let rotate = self.rotate_node.eval(());
		let scale = self.scale_node.eval(());
		let shear = self.shear_node.eval(());

		let (sin, cos) = rotate.sin_cos();

		subpath.apply_affine(DAffine2::from_cols_array(&[scale.x + cos, shear.y + sin, shear.x - sin, scale.y + cos, translate.x, translate.y]));
		subpath
	}
}
