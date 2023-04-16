use std::marker::PhantomData;

use glam::{DAffine2, DVec2};
use graphene_core::raster::{Color, Image, ImageFrame, RasterMut};
use graphene_core::transform::TransformMut;
use graphene_core::vector::VectorData;
use graphene_core::Node;
use node_macro::node_fn;

// Spacing is a consistent 0.2 apart, even when tiled across pixels (from 0.9 to the neighboring 0.1), to avoid bias
const MULTISAMPLE_GRID: [(f64, f64); 25] = [
	// Row 1
	(0.1, 0.1),
	(0.1, 0.3),
	(0.1, 0.5),
	(0.1, 0.7),
	(0.1, 0.9),
	// Row 2
	(0.3, 0.1),
	(0.3, 0.3),
	(0.3, 0.5),
	(0.3, 0.7),
	(0.3, 0.9),
	// Row 3
	(0.5, 0.1),
	(0.5, 0.3),
	(0.5, 0.5),
	(0.5, 0.7),
	(0.5, 0.9),
	// Row 4
	(0.7, 0.1),
	(0.7, 0.3),
	(0.7, 0.5),
	(0.7, 0.7),
	(0.7, 0.9),
	// Row 5
	(0.9, 0.1),
	(0.9, 0.3),
	(0.9, 0.5),
	(0.9, 0.7),
	(0.9, 0.9),
];

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
pub struct VectorPointsNode;

#[node_fn(VectorPointsNode)]
fn vector_points(vector: VectorData) -> Vec<DVec2> {
	vector.subpaths.iter().flat_map(|subpath| subpath.manipulator_groups().iter().map(|group| group.anchor)).collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct BrushTextureNode<ColorNode, Hardness, Flow> {
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
	let alpha = input.a() * (1.0 - flow as f32 * brush.a());
	Color::from_unassociated_alpha(input.r(), input.g(), input.b(), alpha)
}

#[node_fn(BrushTextureNode)]
fn brush_texture(diameter: f64, color: Color, hardness: f64, flow: f64) -> ImageFrame<Color> {
	// Diameter
	let radius = diameter / 2.;
	// TODO: Remove the 4px padding after figuring out why the brush stamp gets randomly offset by 1px up/down/left/right when clicking with the Brush tool
	let dimension = diameter.ceil() as u32 + 4;
	let center = DVec2::splat(radius + (dimension as f64 - diameter) / 2.);

	// Hardness
	let hardness = hardness / 100.;
	let feather_exponent = 1. / (1. - hardness);

	// Flow
	let flow = flow / 100.;

	// Color
	let color = color.apply_opacity(flow as f32);

	// Initial transparent image
	let mut image = Image::new(dimension, dimension, Color::TRANSPARENT);

	for y in 0..dimension {
		for x in 0..dimension {
			let summation = MULTISAMPLE_GRID.iter().fold(0., |acc, (offset_x, offset_y)| {
				let position = DVec2::new(x as f64 + offset_x, y as f64 + offset_y);
				let distance = (position - center).length();

				if distance < radius {
					acc + (1. - (distance / radius).powf(feather_exponent)).clamp(0., 1.)
				} else {
					acc
				}
			});

			let pixel_fill = summation / MULTISAMPLE_GRID.len() as f64;

			let pixel = image.get_pixel_mut(x, y).unwrap();
			*pixel = color.apply_opacity(pixel_fill as f32);
		}
	}

	ImageFrame {
		image,
		transform: DAffine2::from_scale_angle_translation(DVec2::splat(dimension as f64), 0., -DVec2::splat(radius)),
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
		let brush_texture_node = BrushTextureNode::new(ClonedNode::new(Color::BLACK), ClonedNode::new(100.), ClonedNode::new(100.));
		let size = 20.;
		let image = brush_texture_node.eval(size);
		assert_eq!(image.image.width, size.ceil() as u32 + 4);
		assert_eq!(image.image.height, size.ceil() as u32 + 4);
		assert_eq!(image.transform, DAffine2::from_scale_angle_translation(DVec2::splat(size.ceil() + 4.), 0., -DVec2::splat(size / 2.)));
		// center pixel should be BLACK
		assert_eq!(image.image.get_pixel(11, 11), Some(Color::BLACK));
	}

	#[test]
	fn test_brush() {
		let brush_texture_node = BrushTextureNode::new(ClonedNode::new(Color::BLACK), ClonedNode::new(1.0), ClonedNode::new(1.0));
		let image = brush_texture_node.eval(20.);
		let trace = vec![DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0)];
		let trace = ClonedNode::new(trace.into_iter());
		let translate_node = TranslateNode::new(ClonedNode::new(image));
		let frames = MapNode::new(ValueNode::new(translate_node));
		let frames = trace.then(frames).eval(()).collect::<Vec<_>>();
		assert_eq!(frames.len(), 2);
		assert_eq!(frames[0].image.width, 24);
		let background_bounds = ReduceNode::new(ClonedNode::new(None), ValueNode::new(MergeBoundingBoxNode::new()));
		let background_bounds = background_bounds.eval(frames.clone().into_iter());
		let background_bounds = ClonedNode::new(background_bounds.unwrap().to_transform());
		let background_image = background_bounds.then(EmptyImageNode::new(ClonedNode::new(Color::TRANSPARENT)));
		let blend_node = graphene_core::raster::BlendNode::new(ClonedNode::new(BlendMode::Normal), ClonedNode::new(1.0));
		let final_image = ReduceNode::new(background_image, ValueNode::new(BlendImageTupleNode::new(ValueNode::new(blend_node))));
		let final_image = final_image.eval(frames.into_iter());
		assert_eq!(final_image.image.height, 24);
		assert_eq!(final_image.image.width, 34);
		drop(final_image);
	}
}
