use std::marker::PhantomData;

use glam::{DAffine2, DVec2};
use graphene_core::raster::{Alpha, Color, ImageFrame, Pixel, Sample};
use graphene_core::transform::{Transform, TransformMut};
use graphene_core::vector::VectorData;
use graphene_core::Node;
use node_macro::node_fn;

#[derive(Clone, Debug, PartialEq)]
pub struct ReduceNode<Initial, Lambda> {
	pub initial: Initial,
	pub lambda: Lambda,
}

#[node_fn(ReduceNode)]
fn reduce<I: Iterator, Lambda, T>(iter: I, initial: T, lambda: &'input Lambda) -> T
where
	Lambda: for<'a> Node<'a, (T, I::Item), Output = T>,
{
	iter.fold(initial, |a, x| lambda.eval((a, x)))
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChainApplyNode<Value> {
	pub value: Value,
}

#[node_fn(ChainApplyNode)]
async fn chain_apply<I: Iterator, T>(iter: I, mut value: T) -> T
where
	I::Item: for<'a> Node<'a, T, Output = T>,
{
	for lambda in iter {
		value = lambda.eval(value);
	}
	value
}

#[derive(Clone, Debug, PartialEq)]
pub struct IntoIterNode<T> {
	_t: PhantomData<T>,
}

#[node_fn(IntoIterNode<_T>)]
fn into_iter<'i: 'input, _T: Send + Sync>(vec: &'i Vec<_T>) -> Box<dyn Iterator<Item = &'i _T> + Send + Sync + 'i> {
	Box::new(vec.iter())
}

#[derive(Clone, Debug, PartialEq)]
pub struct VectorPointsNode;

#[node_fn(VectorPointsNode)]
fn vector_points(vector: VectorData) -> Vec<DVec2> {
	vector.subpaths.iter().flat_map(|subpath| subpath.manipulator_groups().iter().map(|group| group.anchor)).collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushStampGenerator<P: Pixel + Alpha> {
	color: P,
	feather_exponent: f32,
	transform: DAffine2,
}

impl<P: Pixel + Alpha> Transform for BrushStampGenerator<P> {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
}

impl<P: Pixel + Alpha> TransformMut for BrushStampGenerator<P> {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		&mut self.transform
	}
}

impl<P: Pixel + Alpha> Sample for BrushStampGenerator<P> {
	type Pixel = P;

	#[inline]
	fn sample(&self, position: DVec2, area: DVec2) -> Option<P> {
		let position = self.transform.inverse().transform_point2(position);
		let area = self.transform.inverse().transform_vector2(area);
		let aa_blur_radius = area.length() as f32 * 2.;
		let center = DVec2::splat(0.5);

		let distance = (position + area / 2. - center).length() as f32 * 2.;

		let edge_opacity = 1. - (1. - aa_blur_radius).powf(self.feather_exponent);
		let result = if distance < 1. - aa_blur_radius {
			1. - distance.powf(self.feather_exponent)
		} else if distance < 1. {
			// TODO: Replace this with a proper analytical AA implementation
			edge_opacity * ((1. - distance) / aa_blur_radius)
		} else {
			return None;
		};

		use graphene_core::raster::Channel;
		Some(self.color.multiplied_alpha(P::AlphaChannel::from_linear(result)))
	}
}

#[derive(Clone, Debug, PartialEq)]
pub struct BrushStampGeneratorNode<ColorNode, Hardness, Flow> {
	pub color: ColorNode,
	pub hardness: Hardness,
	pub flow: Flow,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EraseNode<Flow> {
	flow: Flow,
}

#[node_fn(EraseNode)]
fn erase(input: (Color, Color), flow: f64) -> Color {
	let (input, brush) = input;
	let alpha = input.a() * (1. - flow as f32 * brush.a());
	Color::from_unassociated_alpha(input.r(), input.g(), input.b(), alpha)
}

#[node_fn(BrushStampGeneratorNode)]
fn brush_stamp_generator_node(diameter: f64, color: Color, hardness: f64, flow: f64) -> BrushStampGenerator<Color> {
	// Diameter
	let radius = diameter / 2.;

	// Hardness
	let hardness = hardness / 100.;
	let feather_exponent = 1. / (1. - hardness) as f32;

	// Flow
	let flow = flow / 100.;

	// Color
	let color = color.apply_opacity(flow as f32);

	let transform = DAffine2::from_scale_angle_translation(DVec2::splat(diameter), 0., -DVec2::splat(radius));
	BrushStampGenerator { color, feather_exponent, transform }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TranslateNode<Translatable> {
	translatable: Translatable,
}

#[node_fn(TranslateNode)]
fn translate_node<Data: TransformMut>(offset: DVec2, mut translatable: Data) -> Data {
	translatable.translate(offset);
	translatable
}

#[derive(Debug, Clone, Copy)]
pub struct BlitNode<P, Texture, Positions, BlendFn> {
	texture: Texture,
	positions: Positions,
	blend_mode: BlendFn,
	_p: PhantomData<P>,
}

#[node_fn(BlitNode<_P>)]
fn blit_node<_P: Alpha + Pixel + std::fmt::Debug, BlendFn>(mut target: ImageFrame<_P>, texture: ImageFrame<_P>, positions: Vec<DVec2>, blend_mode: BlendFn) -> ImageFrame<_P>
where
	BlendFn: for<'any_input> Node<'any_input, (_P, _P), Output = _P>,
{
	for position in positions {
		let target_size = DVec2::new(target.image.width as f64, target.image.height as f64);
		let texture_size = DVec2::new(texture.image.width as f64, texture.image.height as f64);
		let document_to_target = target.transform.inverse();
		let start = document_to_target.transform_point2(position) * target_size - texture_size / 2.;
		let stop = start + texture_size;

		// Half-open integer ranges [start, stop).
		let clamp_start = start.clamp(DVec2::ZERO, target_size).as_uvec2();
		let clamp_stop = stop.clamp(DVec2::ZERO, target_size).as_uvec2();

		let blit_area_offset = (clamp_start.as_dvec2() - start).as_uvec2().min(texture_size.as_uvec2());
		let blit_area_dimensions = (clamp_stop - clamp_start).min(texture_size.as_uvec2() - blit_area_offset);

		// Tight blitting loop. Eagerly assert bounds to hopefully eliminate bounds check inside loop.
		let texture_index = |x: u32, y: u32| -> usize { (y as usize * texture.image.width as usize) + (x as usize) };
		let target_index = |x: u32, y: u32| -> usize { (y as usize * target.image.width as usize) + (x as usize) };

		let max_y = (blit_area_offset.y + blit_area_dimensions.y).saturating_sub(1);
		let max_x = (blit_area_offset.x + blit_area_dimensions.x).saturating_sub(1);
		assert!(texture_index(max_x, max_y) < texture.image.data.len());
		assert!(target_index(max_x, max_y) < target.image.data.len());

		for y in blit_area_offset.y..blit_area_offset.y + blit_area_dimensions.y {
			for x in blit_area_offset.x..blit_area_offset.x + blit_area_dimensions.x {
				let src_pixel = texture.image.data[texture_index(x, y)];
				let dst_pixel = &mut target.image.data[target_index(x + clamp_start.x, y + clamp_start.y)];
				*dst_pixel = blend_mode.eval((src_pixel, *dst_pixel));
			}
		}
	}

	target
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::raster::*;

	#[allow(unused_imports)]
	use graphene_core::ops::{AddNode, CloneNode};
	use graphene_core::raster::*;
	use graphene_core::structural::Then;
	use graphene_core::transform::{Transform, TransformMut};
	use graphene_core::value::{ClonedNode, ValueNode};

	use glam::DAffine2;

	#[test]
	fn test_translate_node() {
		let image = Image::new(10, 10, Color::TRANSPARENT);
		let mut image = ImageFrame { image, transform: DAffine2::IDENTITY };
		image.translate(DVec2::new(1., 2.));
		let translate_node = TranslateNode::new(ClonedNode::new(image));
		let image = translate_node.eval(DVec2::new(1., 2.));
		assert_eq!(image.transform(), DAffine2::from_translation(DVec2::new(2., 4.)));
	}

	#[test]
	fn test_reduce() {
		let reduce_node = ReduceNode::new(ClonedNode::new(0u32), ValueNode::new(AddNode));
		let sum = reduce_node.eval(vec![1, 2, 3, 4, 5].into_iter());
		assert_eq!(sum, 15);
	}

	#[test]
	fn test_brush_texture() {
		let brush_texture_node = BrushStampGeneratorNode::new(ClonedNode::new(Color::BLACK), ClonedNode::new(100.), ClonedNode::new(100.));
		let size = 20.;
		let image = brush_texture_node.eval(size);
		assert_eq!(image.transform(), DAffine2::from_scale_angle_translation(DVec2::splat(size.ceil()), 0., -DVec2::splat(size / 2.)));
		// center pixel should be BLACK
		assert_eq!(image.sample(DVec2::splat(0.), DVec2::ONE), Some(Color::BLACK));
	}

	#[test]
	fn test_brush() {
		let brush_texture_node = BrushStampGeneratorNode::new(ClonedNode::new(Color::BLACK), ClonedNode::new(1.), ClonedNode::new(1.));
		let image = brush_texture_node.eval(20.);
		let trace = vec![DVec2::new(0., 0.), DVec2::new(10., 0.)];
		let trace = ClonedNode::new(trace.into_iter());
		let translate_node = TranslateNode::new(ClonedNode::new(image));
		let frames = MapNode::new(ValueNode::new(translate_node));
		let frames = trace.then(frames).eval(()).collect::<Vec<_>>();
		assert_eq!(frames.len(), 2);
		let background_bounds = ReduceNode::new(ClonedNode::new(None), ValueNode::new(MergeBoundingBoxNode::new()));
		let background_bounds = background_bounds.eval(frames.clone().into_iter());
		let background_bounds = ClonedNode::new(background_bounds.unwrap().to_transform());
		let background_image = background_bounds.then(EmptyImageNode::new(ClonedNode::new(Color::TRANSPARENT)));
		let blend_node = graphene_core::raster::BlendNode::new(ClonedNode::new(BlendMode::Normal), ClonedNode::new(1.));
		let final_image = ReduceNode::new(background_image, ValueNode::new(BlendImageTupleNode::new(ValueNode::new(blend_node))));
		let final_image = final_image.eval(frames.into_iter());
		assert_eq!(final_image.image.height, 20);
		assert_eq!(final_image.image.width, 30);
		drop(final_image);
	}
}
