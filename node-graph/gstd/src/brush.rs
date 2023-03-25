use std::marker::PhantomData;

use dyn_any::{DynAny, StaticType};

use glam::{BVec2, DAffine2, DVec2};
use graphene_core::raster::{Color, Image, ImageFrame};
use graphene_core::transform::{Transform, TransformMut};
use graphene_core::Node;
use node_macro::node_fn;

#[derive(Clone, Debug, PartialEq)]
pub struct ReduceNode<Initial, Lambda> {
	pub initial: Initial,
	pub lambda: Lambda,
}

#[node_fn(ReduceNode)]
fn reduce<I: Iterator, Lambda, T>(iter: I, initial: T, lambda: &'any_input Lambda) -> T
where
	Lambda: for<'a> Node<'a, (T, I::Item), Output = T>,
{
	iter.fold(initial, |a, x| lambda.eval((a, x)))
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
pub struct BrushTextureNode<ColorNode, Hardness, Opacity> {
	pub color: ColorNode,
	pub hardness: Hardness,
	pub opacity: Opacity,
}

#[node_fn(BrushTextureNode)]
fn brush_texture(radius: f64, color: Color, hardness: f64, opacity: f64) -> ImageFrame {
	let radius = radius.ceil() as u32;
	let diameter = radius * 2 + 2;
	let mut image = Image::new(diameter, diameter, Color::TRANSPARENT);
	let center = DVec2::new(radius as f64 + 0.5, radius as f64 + 0.5);
	for y in 0..diameter {
		for x in 0..diameter {
			let pos = DVec2::new(x as f64, y as f64);
			let dist = (pos - center).length();
			let alpha = if dist < radius as f64 {
				let alpha = (dist / radius as f64).powf(1.0 / hardness);
				(1.0 - alpha) * opacity
			} else {
				0.0
			};
			let pixel = image.get_mut(x, y).unwrap();
			*pixel = Color::from_rgbaf32_unchecked(color.r(), color.g(), color.b(), alpha as f32 * color.a());
		}
	}

	ImageFrame {
		image,
		transform: DAffine2::from_scale(DVec2::splat(diameter as f64)),
	}
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

#[cfg(test)]
mod test {
	use super::*;
	use crate::raster::*;
	use glam::DAffine2;
	use graphene_core::ops::{AddNode, CloneNode};
	use graphene_core::raster::*;
	use graphene_core::structural::Then;
	use graphene_core::transform::{Transform, TransformMut};
	use graphene_core::value::{ClonedNode, ValueNode};

	#[test]
	fn test_translate_node() {
		let image = Image::new(10, 10, Color::TRANSPARENT);
		let mut image = ImageFrame { image, transform: DAffine2::IDENTITY };
		image.translate(DVec2::new(1.0, 2.0));
		let translate_node = TranslateNode::new(ClonedNode::new(image));
		let image = translate_node.eval(DVec2::new(1.0, 2.0));
		assert_eq!(image.transform(), DAffine2::from_translation(DVec2::new(2.0, 4.0)));
	}

	#[test]
	fn test_reduce() {
		let reduce_node = ReduceNode::new(ClonedNode::new(0u32), ValueNode::new(AddNode));
		let sum = reduce_node.eval(vec![1, 2, 3, 4, 5].into_iter());
		assert_eq!(sum, 15);
	}

	#[test]
	fn test_brush_texture() {
		let brush_texture_node = BrushTextureNode::new(ClonedNode::new(Color::BLACK), ClonedNode::new(1.0), ClonedNode::new(1.0));
		let image = brush_texture_node.eval(10.0);
		assert_eq!(image.image.width, 20);
		assert_eq!(image.image.height, 20);
		assert_eq!(image.transform, DAffine2::from_scale(DVec2::splat(20.0)));
		// center pixel should be BLACK
		assert_eq!(image.image.get(10, 10).unwrap(), &Color::BLACK);
	}

	#[test]
	fn test_brush() {
		let brush_texture_node = BrushTextureNode::new(ClonedNode::new(Color::BLACK), ClonedNode::new(1.0), ClonedNode::new(1.0));
		let image = brush_texture_node.eval(10.);
		let trace = vec![DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0)];
		let trace = ClonedNode::new(trace.into_iter());
		let translate_node = TranslateNode::new(ClonedNode::new(image));
		let frames = MapNode::new(ValueNode::new(translate_node));
		let frames = trace.then(frames).eval(()).collect::<Vec<_>>();
		assert_eq!(frames.len(), 2);
		assert_eq!(frames[0].image.width, 20);
		let background_bounds = ReduceNode::new(ClonedNode::new(None), ValueNode::new(MergeBoundingBoxNode::new()));
		let background_bounds = background_bounds.eval(frames.clone().into_iter());
		let background_bounds = ClonedNode::new(background_bounds.unwrap().to_transform());
		let background_image = background_bounds.then(EmptyImageNode::new(ClonedNode::new(Color::TRANSPARENT)));
		let blend_node = graphene_core::raster::BlendNode::new(ClonedNode::new(BlendMode::Normal), ClonedNode::new(1.0));
		let final_image = ReduceNode::new(background_image, ValueNode::new(BlendImageTupleNode::new(ValueNode::new(blend_node))));
		let final_image = final_image.eval(frames.into_iter());
		assert_eq!(final_image.image.height, 20);
		assert_eq!(final_image.image.width, 30);
		drop(final_image);
	}
}
