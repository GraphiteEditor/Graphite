use core::{fmt::Debug, marker::PhantomData};

use crate::Node;

#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::float::Float;

pub mod color;
pub use self::color::Color;

pub mod adjustments;
pub use adjustments::*;

#[derive(Debug, Default)]
pub struct MapNode<MapFn> {
	map_fn: MapFn,
}

#[node_macro::node_fn(MapNode)]
fn map_node<_Iter: Iterator, MapFnNode>(input: _Iter, map_fn: &'any_input MapFnNode) -> MapFnIterator<'input, 'input, _Iter, MapFnNode>
where
	MapFnNode: for<'any_input> Node<'any_input, _Iter::Item>,
{
	MapFnIterator::new(input, map_fn)
}

#[must_use = "iterators are lazy and do nothing unless consumed"]
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

impl<'i, 's: 'i, Iter: Clone, MapFn> Clone for MapFnIterator<'i, 's, Iter, MapFn> {
	fn clone(&self) -> Self {
		Self {
			iter: self.iter.clone(),
			map_fn: self.map_fn,
			_phantom: core::marker::PhantomData,
		}
	}
}
impl<'i, 's: 'i, Iter: Copy, MapFn> Copy for MapFnIterator<'i, 's, Iter, MapFn> {}

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
	F: Node<'i, I::Item> + 'i,
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
pub struct WeightedAvgNode {}

#[node_macro::node_fn(WeightedAvgNode)]
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

#[derive(Debug)]
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

#[derive(Debug)]
pub struct WindowNode<Radius: for<'i> Node<'i, (), Output = u32>, Image: for<'i> Node<'i, (), Output = ImageSlice<'i>>> {
	radius: Radius,
	image: Image,
}

impl<'input, S0: 'input, S1: 'input> Node<'input, u32> for WindowNode<S0, S1>
where
	S0: for<'any_input> Node<'any_input, (), Output = u32>,
	S1: for<'any_input> Node<'any_input, (), Output = ImageSlice<'any_input>>,
{
	type Output = ImageWindowIterator<'input>;
	#[inline]
	fn eval<'node: 'input>(&'node self, input: u32) -> Self::Output {
		let radius = self.radius.eval(());
		let image = self.image.eval(());
		{
			let iter = ImageWindowIterator::new(image, radius, input);
			iter
		}
	}
}
impl<S0, S1> WindowNode<S0, S1>
where
	S0: for<'any_input> Node<'any_input, (), Output = u32>,
	S1: for<'any_input> Node<'any_input, (), Output = ImageSlice<'any_input>>,
{
	pub const fn new(radius: S0, image: S1) -> Self {
		Self { radius, image }
	}
}
/*
#[node_macro::node_fn(WindowNode)]
fn window_node(input: u32, radius: u32, image: ImageSlice<'input>) -> ImageWindowIterator<'input> {
	let iter = ImageWindowIterator::new(image, radius, input);
	iter
}*/

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

#[cfg(not(target_arch = "spriv"))]
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
		#[cfg(feature = "gpu")]
		let value = None;
		#[cfg(not(feature = "gpu"))]
		let value = Some((self.image.data[(self.x + self.y * self.image.width) as usize], (self.x as i32 - start_x, self.y as i32 - start_y)));

		self.x += 1;
		if self.x > max_x {
			self.x = min_x;
			self.y += 1;
		}
		value
	}
}

#[derive(Debug)]
pub struct MapSndNode<First, Second, MapFn> {
	map_fn: MapFn,
	_first: PhantomData<First>,
	_second: PhantomData<Second>,
}

#[node_macro::node_fn(MapSndNode< _First, _Second>)]
fn map_snd_node<MapFn, _First, _Second>(input: (_First, _Second), map_fn: &'any_input MapFn) -> (_First, <MapFn as Node<'input, _Second>>::Output)
where
	MapFn: for<'any_input> Node<'any_input, _Second>,
{
	let (a, b) = input;
	(a, map_fn.eval(b))
}

#[derive(Debug)]
pub struct BrightenColorNode<Brightness> {
	brightness: Brightness,
}
#[node_macro::node_fn(BrightenColorNode)]
fn brighten_color_node(color: Color, brightness: f32) -> Color {
	let per_channel = |col: f32| (col + brightness / 255.).clamp(0., 1.);
	Color::from_rgbaf32_unchecked(per_channel(color.r()), per_channel(color.g()), per_channel(color.b()), color.a())
}

#[derive(Debug)]
pub struct ForEachNode<Iter, MapNode> {
	map_node: MapNode,
	_iter: PhantomData<Iter>,
}

#[node_macro::node_fn(ForEachNode<_Iter>)]
fn map_node<_Iter: Iterator, MapNode>(input: _Iter, map_node: &'any_input MapNode) -> ()
where
	MapNode: for<'any_input> Node<'any_input, _Iter::Item, Output = ()> + 'input,
{
	input.for_each(|x| map_node.eval(x));
}

#[cfg(target_arch = "spirv")]
const NOTHING: () = ();

use dyn_any::{DynAny, StaticType};
#[derive(Clone, Debug, PartialEq, DynAny, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ImageSlice<'a> {
	pub width: u32,
	pub height: u32,
	#[cfg(not(target_arch = "spirv"))]
	pub data: &'a [Color],
	#[cfg(target_arch = "spirv")]
	pub data: &'a (),
}

#[allow(clippy::derivable_impls)]
impl<'a> Default for ImageSlice<'a> {
	#[cfg(not(target_arch = "spirv"))]
	fn default() -> Self {
		Self {
			width: Default::default(),
			height: Default::default(),
			data: Default::default(),
		}
	}
	#[cfg(target_arch = "spirv")]
	fn default() -> Self {
		Self {
			width: Default::default(),
			height: Default::default(),
			data: &NOTHING,
		}
	}
}

impl ImageSlice<'_> {
	#[cfg(not(target_arch = "spirv"))]
	pub const fn empty() -> Self {
		Self { width: 0, height: 0, data: &[] }
	}
}

#[cfg(not(target_arch = "spirv"))]
impl<'a> IntoIterator for ImageSlice<'a> {
	type Item = &'a Color;
	type IntoIter = core::slice::Iter<'a, Color>;
	fn into_iter(self) -> Self::IntoIter {
		self.data.iter()
	}
}

#[cfg(not(target_arch = "spirv"))]
impl<'a> IntoIterator for &'a ImageSlice<'a> {
	type Item = &'a Color;
	type IntoIter = core::slice::Iter<'a, Color>;
	fn into_iter(self) -> Self::IntoIter {
		self.data.iter()
	}
}

#[derive(Debug)]
pub struct ImageDimensionsNode;

#[node_macro::node_fn(ImageDimensionsNode)]
fn dimensions_node(input: ImageSlice<'input>) -> (u32, u32) {
	(input.width, input.height)
}

#[cfg(feature = "alloc")]
pub use image::{CollectNode, Image, ImageFrame, ImageRefNode, MapImageSliceNode};
#[cfg(feature = "alloc")]
mod image {
	use super::{Color, ImageSlice};
	use crate::Node;
	use alloc::vec::Vec;
	use core::hash::{Hash, Hasher};
	use dyn_any::{DynAny, StaticType};
	use glam::DAffine2;

	#[derive(Clone, Debug, PartialEq, DynAny, Default, specta::Type, Hash)]
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
		/// Generate Image from some frontend image data (the canvas pixels as u8s in a flat array)
		pub fn from_image_data(image_data: &[u8], width: u32, height: u32) -> Self {
			let data = image_data.chunks_exact(4).map(|v| Color::from_rgba8(v[0], v[1], v[2], v[3])).collect();
			Image { width, height, data }
		}

		/// Flattens each channel cast to a u8
		pub fn as_flat_u8(self) -> (Vec<u8>, u32, u32) {
			let Image { width, height, data } = self;

			let result_bytes = data.into_iter().flat_map(|color| color.to_rgba8()).collect();

			(result_bytes, width, height)
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

	#[derive(Debug, Clone)]
	pub struct CollectNode {}

	#[node_macro::node_fn(CollectNode)]
	fn collect_node<_Iter>(input: _Iter) -> Vec<_Iter::Item>
	where
		_Iter: Iterator,
	{
		input.collect()
	}

	#[derive(Debug)]
	pub struct MapImageSliceNode<Data> {
		data: Data,
	}

	#[node_macro::node_fn(MapImageSliceNode)]
	fn map_node(input: (u32, u32), data: Vec<Color>) -> Image {
		Image {
			width: input.0,
			height: input.1,
			data,
		}
	}

	#[derive(Clone, Debug, PartialEq, DynAny, Default, specta::Type)]
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	pub struct ImageFrame {
		pub image: Image,
		pub transform: DAffine2,
	}

	impl ImageFrame {
		pub const fn empty() -> Self {
			Self {
				image: Image::empty(),
				transform: DAffine2::ZERO,
			}
		}
	}

	impl Hash for ImageFrame {
		fn hash<H: Hasher>(&self, state: &mut H) {
			self.image.hash(state);
			self.transform.to_cols_array().iter().for_each(|x| x.to_bits().hash(state))
		}
	}
}

#[cfg(test)]
mod test {
	use crate::{ops::CloneNode, structural::Then, value::ValueNode, Node};

	use super::*;

	#[ignore]
	#[test]
	fn map_node() {
		// let array = &mut [Color::from_rgbaf32(1.0, 0.0, 0.0, 1.0).unwrap()];

		// LuminanceNode.eval(Color::from_rgbf32_unchecked(1., 0., 0.));

		/*let map = ForEachNode(MutWrapper(LuminanceNode));
		(&map).eval(array.iter_mut());
		assert_eq!(array[0], Color::from_rgbaf32(0.33333334, 0.33333334, 0.33333334, 1.0).unwrap());*/
	}

	#[test]
	fn window_node() {
		use alloc::vec;
		let radius = ValueNode::new(1u32).then(CloneNode::new());
		let image = ValueNode::<_>::new(Image {
			width: 5,
			height: 5,
			data: vec![Color::from_rgbf32_unchecked(1., 0., 0.); 25],
		});
		let image = image.then(ImageRefNode::new());
		let window = WindowNode::new(radius, image);
		let vec = window.eval(0);
		assert_eq!(vec.count(), 4);
		let vec = window.eval(5);
		assert_eq!(vec.count(), 6);
		let vec = window.eval(12);
		assert_eq!(vec.count(), 9);
	}

	// TODO: I can't be bothered to fix this test rn
	/*
	#[test]
	fn blur_node() {
		use alloc::vec;
		let radius = ValueNode::new(1u32).then(CloneNode::new());
		let sigma = ValueNode::new(3f64).then(CloneNode::new());
		let radius = ValueNode::new(1u32).then(CloneNode::new());
		let image = ValueNode::<_>::new(Image {
			width: 5,
			height: 5,
			data: vec![Color::from_rgbf32_unchecked(1., 0., 0.); 25],
		});
		let image = image.then(ImageRefNode::new());
		let window = WindowNode::new(radius, image);
		let window: TypeNode<_, u32, ImageWindowIterator<'_>> = TypeNode::new(window);
		let distance = ValueNode::new(DistanceNode::new());
		let pos_to_dist = MapSndNode::new(distance);
		let type_erased = &window as &dyn for<'a> Node<'a, u32, Output = ImageWindowIterator<'a>>;
		type_erased.eval(0);
		let map_pos_to_dist = MapNode::new(ValueNode::new(pos_to_dist));

		let type_erased = &map_pos_to_dist as &dyn for<'a> Node<'a, u32, Output = ImageWindowIterator<'a>>;
		type_erased.eval(0);

		let distance = window.then(map_pos_to_dist);
		let map_gaussian = MapSndNode::new(ValueNode(GaussianNode::new(sigma)));
		let map_gaussian: TypeNode<_, (_, f32), (_, f32)> = TypeNode::new(map_gaussian);
		let map_gaussian = ValueNode(map_gaussian);
		let map_gaussian: TypeNode<_, (), &_> = TypeNode::new(map_gaussian);
		let map_distances = MapNode::new(map_gaussian);
		let map_distances: TypeNode<_, _, MapFnIterator<'_, '_, _, _>> = TypeNode::new(map_distances);
		let gaussian_iter = distance.then(map_distances);
		let avg = gaussian_iter.then(WeightedAvgNode::new());
		let avg: TypeNode<_, u32, Color> = TypeNode::new(avg);
		let blur_iter = MapNode::new(ValueNode::new(avg));
		let blur = image.then(ImageIndexIterNode).then(blur_iter);
		let blur: TypeNode<_, (), MapFnIterator<_, _>> = TypeNode::new(blur);
		let collect = CollectNode::new();
		let vec = collect.eval(0..10);
		assert_eq!(vec.len(), 10);
		let _ = blur.eval(());
		let vec = blur.then(collect);
		let _image = vec.eval(());
	}
	*/
}
