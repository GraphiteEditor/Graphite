use core::fmt::Debug;

use crate::Node;

pub mod color;
pub use self::color::Color;

#[derive(Debug, Clone, Copy, Default)]
pub struct GrayscaleColorNode;

#[node_macro::node_fn(GrayscaleColorNode)]
fn grayscale_color_node(input: Color) -> Color {
	let avg = (input.r() + input.g() + input.b()) / 3.0;
	Color::from_rgbaf32_unchecked(avg, avg, avg, input.a())
}

#[derive(Debug)]
pub struct MapNode<Iter: Iterator, MapFn: Node<Iter::Item>> {
	map_fn: MapFn,
	_phantom: core::marker::PhantomData<Iter>,
}

impl<Iter: Iterator, MapFn: Node<Iter::Item> + Clone> Clone for MapNode<Iter, MapFn> {
	fn clone(&self) -> Self {
		Self {
			map_fn: self.map_fn.clone(),
			_phantom: self._phantom,
		}
	}
}
impl<Iter: Iterator, MapFn: Node<Iter::Item> + Copy> Copy for MapNode<Iter, MapFn> {}

impl<Iter: Iterator, MapFn: Node<Iter::Item>> MapNode<Iter, MapFn> {
	pub fn new(map_fn: MapFn) -> Self {
		Self {
			map_fn,
			_phantom: core::marker::PhantomData,
		}
	}
}

impl<Iter: Iterator<Item = Item>, MapFn: Node<Item, Output = Out>, Item, Out> Node<Iter> for MapNode<Iter, MapFn> {
	type Output = MapFnIterator<Iter, MapFn>;

	#[inline]
	fn eval(self, input: Iter) -> Self::Output {
		MapFnIterator::new(input, self.map_fn)
	}
}

impl<Iter: Iterator<Item = Item>, MapFn: Node<Item, Output = Out> + Copy, Item, Out> Node<Iter> for &MapNode<Iter, MapFn> {
	type Output = MapFnIterator<Iter, MapFn>;

	#[inline]
	fn eval(self, input: Iter) -> Self::Output {
		MapFnIterator::new(input, self.map_fn)
	}
}

#[must_use = "iterators are lazy and do nothing unless consumed"]
#[derive(Clone)]
pub struct MapFnIterator<Iter, MapFn> {
	iter: Iter,
	map_fn: MapFn,
}

impl<Iter: Debug, MapFn> Debug for MapFnIterator<Iter, MapFn> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("MapFnIterator").field("iter", &self.iter).field("map_fn", &"MapFn").finish()
	}
}

impl<Iter: Copy, MapFn: Copy> Copy for MapFnIterator<Iter, MapFn> {}

impl<Iter, MapFn> MapFnIterator<Iter, MapFn> {
	pub fn new(iter: Iter, map_fn: MapFn) -> Self {
		Self { iter, map_fn }
	}
}

impl<B, I: Iterator, F> Iterator for MapFnIterator<I, F>
where
	F: Node<I::Item, Output = B> + Copy,
{
	type Item = B;

	#[inline]
	fn next(&mut self) -> Option<B> {
		self.iter.next().map(|x| self.map_fn.eval(x))
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		self.iter.size_hint()
	}
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WeightedAvgNode<Iter> {
	_phantom: core::marker::PhantomData<Iter>,
}

impl<Iter> WeightedAvgNode<Iter> {
	pub fn new() -> Self {
		Self { _phantom: core::marker::PhantomData }
	}
}

#[inline]
fn weighted_avg_node<Iter: Iterator<Item = (Color, f32)> + Clone>(input: Iter) -> Color {
	let total_weight: f32 = input.clone().map(|(_, weight)| weight).sum();
	let total_r: f32 = input.clone().map(|(color, weight)| color.r() * weight).sum();
	let total_g: f32 = input.clone().map(|(color, weight)| color.g() * weight).sum();
	let total_b: f32 = input.clone().map(|(color, weight)| color.b() * weight).sum();
	let total_a: f32 = input.map(|(color, weight)| color.a() * weight).sum();
	Color::from_rgbaf32_unchecked(total_r / total_weight, total_g / total_weight, total_b / total_weight, total_a / total_weight)
}

impl<Iter: Iterator<Item = (Color, f32)> + Clone> Node<Iter> for WeightedAvgNode<Iter> {
	type Output = Color;

	#[inline]
	fn eval(self, input: Iter) -> Self::Output {
		weighted_avg_node(input)
	}
}
impl<Iter: Iterator<Item = (Color, f32)> + Clone> Node<Iter> for &WeightedAvgNode<Iter> {
	type Output = Color;

	#[inline]
	fn eval(self, input: Iter) -> Self::Output {
		weighted_avg_node(input)
	}
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
fn image_index_iter_node(input: ImageSlice<'static>) -> core::ops::Range<u32> {
	0..(input.width * input.height)
}

#[derive(Debug, Clone, Copy)]
pub struct WindowNode<Radius, Image> {
	radius: Radius,
	image: Image,
}

impl<Radius, Image> WindowNode<Radius, Image> {
	pub fn new(radius: Radius, image: Image) -> Self {
		Self { radius, image }
	}
}

impl<'a, Radius: Node<(), Output = u32>, Image: Node<(), Output = ImageSlice<'a>>> Node<u32> for WindowNode<Radius, Image> {
	type Output = ImageWindowIterator<'a>;
	#[inline]
	fn eval(self, input: u32) -> Self::Output {
		let radius = self.radius.eval(());
		let image = self.image.eval(());
		let iter = ImageWindowIterator::new(image, radius, input);
		iter
	}
}
impl<'a, 'b: 'a, Radius: Node<(), Output = u32> + Copy, Index: Node<(), Output = ImageSlice<'b>> + Copy> Node<u32> for &'a WindowNode<Radius, Index> {
	type Output = ImageWindowIterator<'a>;
	#[inline]
	fn eval(self, input: u32) -> Self::Output {
		let radius = self.radius.eval(());
		let image = self.image.eval(());
		let iter = ImageWindowIterator::new(image, radius, input);
		iter
	}
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

#[derive(Debug, Clone, Copy)]
pub struct MapSndNode<MapFn> {
	map_fn: MapFn,
}

impl<MapFn> MapSndNode<MapFn> {
	pub fn new(map_fn: MapFn) -> Self {
		Self { map_fn }
	}
}

impl<MapFn: Node<I>, I, F> Node<(F, I)> for MapSndNode<MapFn> {
	type Output = (F, MapFn::Output);
	#[inline]
	fn eval(self, input: (F, I)) -> Self::Output {
		(input.0, self.map_fn.eval(input.1))
	}
}
impl<MapFn: Node<I> + Copy, I, F> Node<(F, I)> for &MapSndNode<MapFn> {
	type Output = (F, MapFn::Output);
	#[inline]
	fn eval(self, input: (F, I)) -> Self::Output {
		(input.0, self.map_fn.eval(input.1))
	}
}

#[derive(Debug, Clone, Copy)]
pub struct BrightenColorNode<N: Node<(), Output = f32>>(N);

impl<N: Node<(), Output = f32>> Node<Color> for BrightenColorNode<N> {
	type Output = Color;
	fn eval(self, color: Color) -> Color {
		let brightness = self.0.eval(());
		let per_channel = |col: f32| (col + brightness / 255.).clamp(0., 1.);
		Color::from_rgbaf32_unchecked(per_channel(color.r()), per_channel(color.g()), per_channel(color.b()), color.a())
	}
}
impl<N: Node<(), Output = f32> + Copy> Node<Color> for &BrightenColorNode<N> {
	type Output = Color;
	fn eval(self, color: Color) -> Color {
		let brightness = self.0.eval(());
		let per_channel = |col: f32| (col + brightness / 255.).clamp(0., 1.);
		Color::from_rgbaf32_unchecked(per_channel(color.r()), per_channel(color.g()), per_channel(color.b()), color.a())
	}
}

impl<N: Node<(), Output = f32> + Copy> BrightenColorNode<N> {
	pub fn new(node: N) -> Self {
		Self(node)
	}
}

#[derive(Debug, Clone, Copy)]
pub struct GammaColorNode<N: Node<(), Output = f32>>(N);

impl<N: Node<(), Output = f32>> Node<Color> for GammaColorNode<N> {
	type Output = Color;
	fn eval(self, color: Color) -> Color {
		let gamma = self.0.eval(());
		let per_channel = |col: f32| col.powf(gamma);
		Color::from_rgbaf32_unchecked(per_channel(color.r()), per_channel(color.g()), per_channel(color.b()), color.a())
	}
}
impl<N: Node<(), Output = f32> + Copy> Node<Color> for &GammaColorNode<N> {
	type Output = Color;
	fn eval(self, color: Color) -> Color {
		let gamma = self.0.eval(());
		let per_channel = |col: f32| col.powf(gamma);
		Color::from_rgbaf32_unchecked(per_channel(color.r()), per_channel(color.g()), per_channel(color.b()), color.a())
	}
}

impl<N: Node<(), Output = f32> + Copy> GammaColorNode<N> {
	pub fn new(node: N) -> Self {
		Self(node)
	}
}

#[derive(Debug, Clone, Copy)]
#[cfg(not(target_arch = "spirv"))]
pub struct HueShiftColorNode<N: Node<(), Output = f32>>(N);

#[cfg(not(target_arch = "spirv"))]
impl<N: Node<(), Output = f32>> Node<Color> for HueShiftColorNode<N> {
	type Output = Color;
	fn eval(self, color: Color) -> Color {
		let hue_shift = self.0.eval(());
		let [hue, saturation, lightness, alpha] = color.to_hsla();
		Color::from_hsla(hue + hue_shift / 360., saturation, lightness, alpha)
	}
}
#[cfg(not(target_arch = "spirv"))]
impl<N: Node<(), Output = f32> + Copy> Node<Color> for &HueShiftColorNode<N> {
	type Output = Color;
	fn eval(self, color: Color) -> Color {
		let hue_shift = self.0.eval(());
		let [hue, saturation, lightness, alpha] = color.to_hsla();
		Color::from_hsla(hue + hue_shift / 360., saturation, lightness, alpha)
	}
}

#[cfg(not(target_arch = "spirv"))]
impl<N: Node<(), Output = f32> + Copy> HueShiftColorNode<N> {
	pub fn new(node: N) -> Self {
		Self(node)
	}
}

pub struct ForEachNode<MN>(pub MN);

impl<'n, I: Iterator<Item = S>, MN: 'n, S> Node<I> for &'n ForEachNode<MN>
where
	&'n MN: Node<S, Output = ()>,
{
	type Output = ();
	fn eval(self, input: I) -> Self::Output {
		input.for_each(|x| (&self.0).eval(x))
	}
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

#[derive(Debug, Clone, Copy)]
pub struct MapImageSliceNode<MapFn>(MapFn);

impl<MapFn> MapImageSliceNode<MapFn> {
	pub fn new(map_fn: MapFn) -> Self {
		Self(map_fn)
	}
}

impl<'a, MapFn: Node<ImageSlice<'a>, Output = Vec<Color>>> Node<ImageSlice<'a>> for MapImageSliceNode<MapFn> {
	type Output = Image;
	fn eval(self, image: ImageSlice<'a>) -> Self::Output {
		let data = self.0.eval(image);
		Image {
			width: image.width,
			height: image.height,
			data,
		}
	}
}

impl<'a, MapFn: Copy + Node<ImageSlice<'a>, Output = Vec<Color>>> Node<ImageSlice<'a>> for &MapImageSliceNode<MapFn> {
	type Output = Image;
	fn eval(self, image: ImageSlice<'a>) -> Self::Output {
		let data = self.0.eval(image);
		Image {
			width: image.width,
			height: image.height,
			data,
		}
	}
}

#[cfg(feature = "alloc")]
pub use image::{CollectNode, Image, ImageRefNode};
#[cfg(feature = "alloc")]
mod image {
	use super::{Color, ImageSlice};
	use alloc::vec::Vec;
	use dyn_any::{DynAny, StaticType};
	#[derive(Clone, Debug, PartialEq, DynAny, Default, specta::Type)]
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

	impl ImageRefNode {
		pub fn new() -> Self {
			Self
		}
	}

	impl<'a> Node<&'a Image> for ImageRefNode {
		type Output = ImageSlice<'a>;
		fn eval(self, image: &'a Image) -> Self::Output {
			image.as_slice()
		}
	}

	impl<'a> Node<&'a Image> for &ImageRefNode {
		type Output = ImageSlice<'a>;
		fn eval(self, image: &'a Image) -> Self::Output {
			image.as_slice()
		}
	}

	#[derive(Debug, Clone, Copy)]
	pub struct CollectNode;

	use crate::Node;
	impl<Iter: Iterator> Node<Iter> for CollectNode {
		type Output = Vec<Iter::Item>;
		fn eval(self, iter: Iter) -> Self::Output {
			iter.collect()
		}
	}
	impl<Iter: Iterator> Node<Iter> for &CollectNode {
		type Output = Vec<Iter::Item>;
		fn eval(self, iter: Iter) -> Self::Output {
			iter.collect()
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
		static DATA: &[Color] = &[Color::from_rgbf32_unchecked(1., 0., 0.); 25];
		let image = ValueNode::<_>::new(ImageSlice { width: 5, height: 5, data: DATA });
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
		static DATA: &[Color] = &[Color::from_rgbf32_unchecked(1., 0., 0.); 20];
		let image = ValueNode::<_>::new(ImageSlice { width: 10, height: 2, data: DATA });
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
		let _image = vec.eval(());
	}
}
