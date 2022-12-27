use crate::Node;

pub mod color;
pub use self::color::Color;

#[derive(Debug, Clone, Copy)]
pub struct GrayscaleColorNode;

#[node_macro::node_fn(GrayscaleColorNode)]
fn grayscale_color_node(input: Color) -> Color {
	let avg = (input.r() + input.g() + input.b()) / 3.0;
	Color::from_rgbaf32_unchecked(avg, avg, avg, input.a())
}

#[derive(Debug, Clone, Copy)]
pub struct MapNode<Iter, MapFn, Item, Out> {
	map_fn: MapFn,
	_phantom: core::marker::PhantomData<(Iter, Item, Out)>,
}

impl<Iter, MapFn, Item, Out> MapNode<Iter, MapFn, Item, Out> {
	pub fn new(map_fn: MapFn) -> Self {
		Self {
			map_fn,
			_phantom: core::marker::PhantomData,
		}
	}
}

impl<Iter: Iterator<Item = Item>, MapFn: Node<Item>, Item, Out> Node<Iter> for MapNode<Iter, MapFn, Item, Out> {
	type Output = MapFnIterator<Iter, MapFn>;

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

#[derive(Debug, Clone, Copy)]
pub struct WeightedAvgNode<Iter> {
	_phantom: core::marker::PhantomData<Iter>,
}

impl<Iter> WeightedAvgNode<Iter> {
	pub fn new() -> Self {
		Self { _phantom: core::marker::PhantomData }
	}
}

fn weighted_avg_node<Iter: Iterator<Item = (Color, f32)> + Copy>(input: Iter) -> Color {
	let total_weight: f32 = input.map(|(_, weight)| weight).sum();
	let total_r: f32 = input.map(|(color, weight)| color.r() * weight).sum();
	let total_g: f32 = input.map(|(color, weight)| color.g() * weight).sum();
	let total_b: f32 = input.map(|(color, weight)| color.b() * weight).sum();
	let total_a: f32 = input.map(|(color, weight)| color.a() * weight).sum();
	Color::from_rgbaf32_unchecked(total_r / total_weight, total_g / total_weight, total_b / total_weight, total_a / total_weight)
}

impl<Iter: Iterator<Item = (Color, f32)> + Copy> Node<Iter> for WeightedAvgNode<Iter> {
	type Output = Color;
	fn eval(self, input: Iter) -> Self::Output {
		weighted_avg_node(input)
	}
}
impl<Iter: Iterator<Item = (Color, f32)> + Copy> Node<Iter> for &WeightedAvgNode<Iter> {
	type Output = Color;
	fn eval(self, input: Iter) -> Self::Output {
		weighted_avg_node(input)
	}
}

#[derive(Debug, Clone, Copy)]
pub struct GaussianNode<Sigma> {
	sigma: Sigma,
}

#[node_macro::node_fn(GaussianNode)]
fn gaussian_node(input: f32, sigma: f32) -> f32 {
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
fn image_index_iter_node(input: (i32, i32)) -> core::ops::Range<u32> {
	let (width, height) = input;
	0..(width * height) as u32
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
	fn eval(self, input: u32) -> Self::Output {
		let radius = self.radius.eval(());
		let image = self.image.eval(());
		let iter = ImageWindowIterator::new(image, radius, input);
		iter
	}
}
impl<'a, 'b: 'a, Radius: Node<(), Output = u32> + Copy, Index: Node<(), Output = ImageSlice<'b>> + Copy> Node<u32> for &'a WindowNode<Radius, Index> {
	type Output = ImageWindowIterator<'a>;
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
	type Item = (Color, (u32, u32));
	fn next(&mut self) -> Option<Self::Item> {
		let start_x = self.index as i32 % self.image.width as i32;
		let start_y = self.index as i32 / self.image.width as i32;
		let radius = self.radius as i32;

		let min_x = (start_x - radius).max(0) as u32;
		let max_x = (start_x + radius).min(self.image.width as i32 - 1) as u32;
		let max_y = (start_y + radius).min(self.image.height as i32 - 1) as u32;

		if self.x > max_x {
			self.x = min_x;
			self.y += 1;
		}
		if self.y > max_y {
			return None;
		}
		Some((self.image.data[(self.x + self.y * self.image.width) as usize], (self.x, self.y)))
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
	fn eval(self, input: (F, I)) -> Self::Output {
		(input.0, self.map_fn.eval(input.1))
	}
}
impl<MapFn: Node<I> + Copy, I, F> Node<(F, I)> for &MapSndNode<MapFn> {
	type Output = (F, MapFn::Output);
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

#[cfg(feature = "alloc")]
pub use image::{CollectNode, Image};
#[cfg(feature = "alloc")]
mod image {
	use super::Color;
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
	}

	impl IntoIterator for Image {
		type Item = Color;
		type IntoIter = alloc::vec::IntoIter<Color>;
		fn into_iter(self) -> Self::IntoIter {
			self.data.into_iter()
		}
	}

	impl<'a> IntoIterator for &'a Image {
		type Item = &'a Color;
		type IntoIter = alloc::slice::Iter<'a, Color>;
		fn into_iter(self) -> Self::IntoIter {
			self.data.iter()
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
	use super::*;

	#[test]
	fn map_node() {
		// let array = &mut [Color::from_rgbaf32(1.0, 0.0, 0.0, 1.0).unwrap()];
		(&GrayscaleColorNode).eval(Color::from_rgbf32_unchecked(1., 0., 0.));
		/*let map = ForEachNode(MutWrapper(GrayscaleNode));
		(&map).eval(array.iter_mut());
		assert_eq!(array[0], Color::from_rgbaf32(0.33333334, 0.33333334, 0.33333334, 1.0).unwrap());*/
	}
}
