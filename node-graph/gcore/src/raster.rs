use core::{fmt::Debug, marker::PhantomData};

use crate::{Node, NodeIO};

pub mod color;
pub use self::color::Color;

#[derive(Debug, Clone, Copy, Default)]
pub struct GrayscaleColorNode;

#[node_macro::node_fn(GrayscaleColorNode)]
fn grayscale_color_node(input: Color) -> Color {
	let avg = (input.r() + input.g() + input.b()) / 3.0;
	Color::from_rgbaf32_unchecked(avg, avg, avg, input.a())
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MapNode<Iter, MapFn> {
	map_fn: MapFn,
	_iter: PhantomData<Iter>,
}

#[node_macro::node_fn(MapNode<_Iter>)]
fn map_node<_Iter: Iterator, MapFnNode>(input: _Iter, map_fn: &'node MapFnNode) -> MapFnIterator<'input, 'input, _Iter, MapFnNode>
where
	MapFnNode: Node<'input, 'node, _Iter::Item> + 'node,
{
	MapFnIterator::new(input, map_fn)
}

#[must_use = "iterators are lazy and do nothing unless consumed"]
#[derive(Clone)]
pub struct MapFnIterator<'i, 's, Iter, MapFn> {
	iter: Iter,
	map_fn: &'s MapFn,
	_phantom: core::marker::PhantomData<&'i &'s ()>,
}

impl<'i, 's: 'i, Iter: Debug, MapFn> Debug for MapFnIterator<'i, 's, Iter, MapFn> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("MapFnIterator").field("iter", &self.iter).field("map_fn", &"MapFn").finish()
	}
}

impl<'i, 's: 'i, Iter: Copy, MapFn: Copy> Copy for MapFnIterator<'i, 's, Iter, MapFn> {}

impl<'i, 's: 'i, Iter, MapFn> MapFnIterator<'i, 's, Iter, MapFn> {
	pub fn new(iter: Iter, map_fn: &'s MapFn) -> Self {
		Self {
			iter,
			map_fn,
			_phantom: core::marker::PhantomData,
		}
	}
}

impl<'i, 's: 'i, I: Iterator + 's, F> Iterator for MapFnIterator<'i, 's, I, F>
where
	F: Node<'i, 's, I::Item> + Copy + 'i,
	Self: 'i,
{
	type Item = F::Output;

	#[inline]
	fn next(&mut self) -> Option<F::Output> {
		self.iter.next().map(|x| self.map_fn.eval(x))
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		self.iter.size_hint()
	}
}

#[derive(Debug, Clone, Copy)]
struct WeightedAvgNode<Iter> {
	_iter: PhantomData<Iter>,
}

#[node_macro::node_fn(WeightedAvgNode<_Iter>)]
fn weighted_avg_node<_Iter: Iterator<Item = (Color, f32)>>(input: _Iter) -> Color
where
	_Iter: Clone,
{
	let total_weight: f32 = input.clone().map(|(_, weight)| weight).sum();
	let total_r: f32 = input.clone().map(|(color, weight)| color.r() * weight).sum();
	let total_g: f32 = input.clone().map(|(color, weight)| color.g() * weight).sum();
	let total_b: f32 = input.clone().map(|(color, weight)| color.b() * weight).sum();
	let total_a: f32 = input.map(|(color, weight)| color.a() * weight).sum();
	Color::from_rgbaf32_unchecked(total_r / total_weight, total_g / total_weight, total_b / total_weight, total_a / total_weight)
}

#[derive(Debug, Clone, Copy)]
pub struct GaussianNode<Sigma> {
	sigma: Sigma,
}
#[node_macro::node_fn(GaussianNode)]
fn gaussian_node(input: f32, sigma: f64) -> f32 {
	let sigma = sigma as f32;
	(1.0 / (2.0 * core::f32::consts::PI * sigma * sigma).sqrt()) * (-input * input / (2.0 * sigma * sigma)).exp()
}

#[derive(Debug, Clone, Copy)]
pub struct DistanceNode;

#[node_macro::node_fn(DistanceNode)]
fn distance_node(input: (i32, i32)) -> f32 {
	let (x, y) = input;
	((x * x + y * y) as f32).sqrt()
}

#[derive(Debug, Clone, Copy)]
pub struct ImageIndexIterNode;

#[node_macro::node_fn(ImageIndexIterNode)]
fn image_index_iter_node(input: ImageSlice<'input>) -> core::ops::Range<u32> {
	0..(input.width * input.height)
}

#[derive(Debug, Clone, Copy)]
pub struct WindowNode<Radius, Image> {
	radius: Radius,
	image: Image,
}

#[node_macro::node_fn(WindowNode)]
fn window_node(input: u32, radius: u32, image: ImageSlice<'input>) -> ImageWindowIterator<'input> {
	let iter = ImageWindowIterator::new(image, radius, input);
	iter
}

#[derive(Debug, Clone, Copy)]
pub struct ImageWindowIterator<'a> {
	image: ImageSlice<'a>,
	radius: u32,
	index: u32,
	x: u32,
	y: u32,
}

impl<'a> ImageWindowIterator<'a> {
	fn new(image: ImageSlice<'a>, radius: u32, index: u32) -> Self {
		let start_x = index as i32 % image.width as i32;
		let start_y = index as i32 / image.width as i32;
		let min_x = (start_x - radius as i32).max(0) as u32;
		let min_y = (start_y - radius as i32).max(0) as u32;

		Self {
			image,
			radius,
			index,
			x: min_x,
			y: min_y,
		}
	}
}

impl<'a> Iterator for ImageWindowIterator<'a> {
	type Item = (Color, (i32, i32));
	#[inline]
	fn next(&mut self) -> Option<Self::Item> {
		let start_x = self.index as i32 % self.image.width as i32;
		let start_y = self.index as i32 / self.image.width as i32;
		let radius = self.radius as i32;

		let min_x = (start_x - radius).max(0) as u32;
		let max_x = (start_x + radius).min(self.image.width as i32 - 1) as u32;
		let max_y = (start_y + radius).min(self.image.height as i32 - 1) as u32;
		if self.y > max_y {
			return None;
		}
		let value = Some((self.image.data[(self.x + self.y * self.image.width) as usize], (self.x as i32 - start_x, self.y as i32 - start_y)));

		self.x += 1;
		if self.x > max_x {
			self.x = min_x;
			self.y += 1;
		}
		value
	}
}

#[derive(Debug, Clone)]
pub struct MapSndNode<First, Second, MapFn> {
	map_fn: MapFn,
	_first: PhantomData<First>,
	_second: PhantomData<Second>,
}

#[node_macro::node_fn(MapSndNode< _First, _Second>)]
fn map_snd_node<MapFn, _First, _Second>(input: (_First, _Second), map_fn: &'node MapFn) -> (_First, MapFn::Output)
where
	MapFn: Node<'input, 'node, _Second> + 'node,
{
	let (a, b) = input;
	(a, map_fn.eval(b))
}

#[derive(Debug, Clone, Copy)]
pub struct BrightenColorNode<Brightness> {
	brightness: Brightness,
}
#[node_macro::node_fn(BrightenColorNode)]
fn brighten_color_node(color: Color, brightness: f32) -> Color {
	let per_channel = |col: f32| (col + brightness / 255.).clamp(0., 1.);
	Color::from_rgbaf32_unchecked(per_channel(color.r()), per_channel(color.g()), per_channel(color.b()), color.a())
}

#[derive(Debug, Clone, Copy)]
pub struct GammaColorNode<Gamma> {
	gamma: Gamma,
}

#[node_macro::node_fn(GammaColorNode)]
fn gamma_color_node(color: Color, gamma: f32) -> Color {
	let per_channel = |col: f32| col.powf(gamma);
	Color::from_rgbaf32_unchecked(per_channel(color.r()), per_channel(color.g()), per_channel(color.b()), color.a())
}

#[cfg(not(target_arch = "spirv"))]
mod hue_shift {
	use super::*;
	#[derive(Debug, Clone, Copy)]
	pub struct HueShiftColorNode<Angle> {
		angle: Angle,
	}

	#[node_macro::node_fn(HueShiftColorNode)]
	fn hue_shift_color_node(color: Color, angle: f32) -> Color {
		let hue_shift = angle;
		let [hue, saturation, lightness, alpha] = color.to_hsla();
		Color::from_hsla(hue + hue_shift / 360., saturation, lightness, alpha)
	}
}

#[derive(Debug, Clone, Copy)]
pub struct ForEachNode<Iter, MapNode> {
	map_node: MapNode,
	_iter: PhantomData<Iter>,
}

#[node_macro::node_fn(ForEachNode<_Iter>)]
fn map_node<_Iter: Iterator, MapNode>(input: _Iter, map_node: &'node MapNode) -> ()
where
	MapNode: Node<'input, 'node, _Iter::Item, Output = ()> + 'node,
{
	input.for_each(|x| map_node.eval(x));
}

use dyn_any::{DynAny, StaticType};
#[derive(Clone, Debug, PartialEq, DynAny, Default, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ImageSlice<'a> {
	pub width: u32,
	pub height: u32,
	pub data: &'a [Color],
}

impl ImageSlice<'_> {
	pub const fn empty() -> Self {
		Self { width: 0, height: 0, data: &[] }
	}
}

impl<'a> IntoIterator for ImageSlice<'a> {
	type Item = &'a Color;
	type IntoIter = core::slice::Iter<'a, Color>;
	fn into_iter(self) -> Self::IntoIter {
		self.data.iter()
	}
}

impl<'a> IntoIterator for &'a ImageSlice<'a> {
	type Item = &'a Color;
	type IntoIter = core::slice::Iter<'a, Color>;
	fn into_iter(self) -> Self::IntoIter {
		self.data.iter()
	}
}

#[cfg(feature = "alloc")]
pub use image::{CollectNode, Image, ImageRefNode};
#[cfg(feature = "alloc")]
mod image {
	use super::{Color, ImageSlice};
	use crate::{Node, NodeIO};
	use alloc::vec::Vec;
	use dyn_any::{DynAny, StaticType};

	#[derive(Clone, Debug, PartialEq, DynAny, Default)]
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	pub struct Image {
		pub width: u32,
		pub height: u32,
		pub data: Vec<Color>,
	}

	impl Image {
		pub const fn empty() -> Self {
			Self {
				width: 0,
				height: 0,
				data: Vec::new(),
			}
		}
		pub fn as_slice(&self) -> ImageSlice {
			ImageSlice {
				width: self.width,
				height: self.height,
				data: self.data.as_slice(),
			}
		}
	}

	impl IntoIterator for Image {
		type Item = Color;
		type IntoIter = alloc::vec::IntoIter<Color>;
		fn into_iter(self) -> Self::IntoIter {
			self.data.into_iter()
		}
	}

	#[derive(Debug, Clone, Copy, Default)]
	pub struct ImageRefNode;

	#[node_macro::node_fn(ImageRefNode)]
	fn image_ref_node(image: &'input Image) -> ImageSlice<'input> {
		image.as_slice()
	}

	#[derive(Debug, Clone, Copy)]
	pub struct CollectNode<Iter> {
		_iter: core::marker::PhantomData<Iter>,
	}

	#[node_macro::node_fn(CollectNode<_Iter>)]
	fn collect_node<_Iter>(input: _Iter) -> Vec<_Iter::Item>
	where
		_Iter: Iterator,
	{
		input.collect()
	}

	#[derive(Debug, Clone)]
	pub struct MapImageSliceNode<MapFn> {
		map_fn: MapFn,
	}

	#[node_macro::node_fn(MapImageSliceNode)]
	fn map_node<MapFn>(image: ImageSlice<'input>, map_fn: &'node MapFn) -> Image
	where
		MapFn: Node<'input, 'node, ImageSlice<'input>, Output = Vec<Color>> + 'node,
	{
		let data = map_fn.eval(image);
		Image {
			width: image.width,
			height: image.height,
			data,
		}
	}
}

/*pub struct MutWrapper<N>(pub N);

impl<'n, T: Clone, N> Node<&'n mut T> for &'n MutWrapper<N>
where
	&'n N: Node<T, Output = T>,
{
	type Output = ();
	fn eval(self, value: &'n mut T) {
		*value = (&self.0).eval(value.clone());
	}
}*/

#[cfg(test)]
mod test {
	use crate::{
		ops::TypeNode,
		structural::{ComposeNode, Then},
		value::ValueNode,
	};

	use super::*;
	use alloc::vec::Vec;

	#[test]
	fn map_node() {
		// let array = &mut [Color::from_rgbaf32(1.0, 0.0, 0.0, 1.0).unwrap()];
		(&GrayscaleColorNode).eval(Color::from_rgbf32_unchecked(1., 0., 0.));
		/*let map = ForEachNode(MutWrapper(GrayscaleNode));
		(&map).eval(array.iter_mut());
		assert_eq!(array[0], Color::from_rgbaf32(0.33333334, 0.33333334, 0.33333334, 1.0).unwrap());*/
	}
	#[test]
	fn window_node() {
		let radius = ValueNode::new(1u32);
		static data: &[Color] = &[Color::from_rgbf32_unchecked(1., 0., 0.); 25];
		let image = ValueNode::<_>::new(ImageSlice { width: 5, height: 5, data });
		let window = WindowNode::new(radius, image);
		//let window: TypeNode<_, u32, ImageWindowIterator<'static>> = TypeNode::new(window);
		let vec = window.eval(0);
		assert_eq!(vec.count(), 4);
		let vec = window.eval(5);
		assert_eq!(vec.count(), 6);
		let vec = window.eval(12);
		assert_eq!(vec.count(), 9);
	}

	#[test]
	fn blur_node() {
		let radius = ValueNode::new(1u32);
		let sigma = ValueNode::new(3f64);
		static data: &[Color] = &[Color::from_rgbf32_unchecked(1., 0., 0.); 20];
		let image = ValueNode::<_>::new(ImageSlice { width: 10, height: 2, data });
		let window = WindowNode::new(radius, image);
		let window: TypeNode<_, u32, ImageWindowIterator<'static>> = TypeNode::new(window);
		let pos_to_dist = MapSndNode::new(DistanceNode);
		let distance = window.then(MapNode::new(pos_to_dist));
		let map_gaussian = MapSndNode::new(GaussianNode::new(sigma));
		let map_distances: MapNode<_, MapSndNode<_>> = MapNode::new(map_gaussian);
		let gaussian_iter = distance.then(map_distances);
		let avg = gaussian_iter.then(WeightedAvgNode::new());
		let avg: TypeNode<_, u32, Color> = TypeNode::new(avg);
		let blur_iter = MapNode::new(avg);
		let blur = image.then(ImageIndexIterNode).then(blur_iter);
		let blur: TypeNode<_, (), MapFnIterator<_, _>> = TypeNode::new(blur);
		let collect = CollectNode {};
		let vec = collect.eval(0..10);
		assert_eq!(vec.len(), 10);
		let vec = ComposeNode::new(blur, collect);
		let vec: TypeNode<_, (), Vec<Color>> = TypeNode::new(vec);
		let image = vec.eval(());
	}
}
