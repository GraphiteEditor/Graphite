use core::{fmt::Debug, marker::PhantomData};

use crate::Node;

use bytemuck::{Pod, Zeroable};
use glam::DVec2;

pub use self::color::{Color, Luma, SRGBA8};

#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::float::Float;

pub mod adjustments;
pub mod bbox;
#[cfg(not(target_arch = "spirv"))]
pub mod brightness_contrast;
#[cfg(not(target_arch = "spirv"))]
pub mod brush_cache;
pub mod color;
#[cfg(not(target_arch = "spirv"))]
pub mod curve;
pub mod discrete_srgb;
pub use adjustments::*;

pub trait Linear {
	fn from_f32(x: f32) -> Self;
	fn to_f32(self) -> f32;
	fn from_f64(x: f64) -> Self;
	fn to_f64(self) -> f64;
	fn lerp(self, other: Self, value: Self) -> Self
	where
		Self: Sized + Copy,
		Self: core::ops::Sub<Self, Output = Self>,
		Self: core::ops::Mul<Self, Output = Self>,
		Self: core::ops::Add<Self, Output = Self>,
	{
		self + (other - self) * value
	}
}

#[rustfmt::skip]
impl Linear for f32 {
	#[inline(always)] fn from_f32(x: f32) -> Self { x }
	#[inline(always)] fn to_f32(self) -> f32 { self }
	#[inline(always)] fn from_f64(x: f64) -> Self { x as f32 }
	#[inline(always)] fn to_f64(self) -> f64 { self as f64 }
}

#[rustfmt::skip]
impl Linear for f64 {
	#[inline(always)] fn from_f32(x: f32) -> Self { x as f64 }
	#[inline(always)] fn to_f32(self) -> f32 { self as f32 }
	#[inline(always)] fn from_f64(x: f64) -> Self { x }
	#[inline(always)] fn to_f64(self) -> f64 { self }
}

pub trait Channel: Copy + Debug {
	fn to_linear<Out: Linear>(self) -> Out;
	fn from_linear<In: Linear>(linear: In) -> Self;
}

pub trait LinearChannel: Channel {
	fn cast_linear_channel<Out: LinearChannel>(self) -> Out {
		Out::from_linear(self.to_linear::<f64>())
	}
}

impl<T: Linear + Debug + Copy> Channel for T {
	#[inline(always)]
	fn to_linear<Out: Linear>(self) -> Out {
		Out::from_f64(self.to_f64())
	}

	#[inline(always)]
	fn from_linear<In: Linear>(linear: In) -> Self {
		Self::from_f64(linear.to_f64())
	}
}

impl<T: Linear + Debug + Copy> LinearChannel for T {}

use num_derive::*;
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Num, NumCast, NumOps, One, Zero, ToPrimitive, FromPrimitive)]
struct SRGBGammaFloat(f32);

impl Channel for SRGBGammaFloat {
	#[inline(always)]
	fn to_linear<Out: Linear>(self) -> Out {
		let x = self.0;
		Out::from_f32(if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) })
	}

	#[inline(always)]
	fn from_linear<In: Linear>(linear: In) -> Self {
		let x = linear.to_f32();
		if x <= 0.0031308 {
			Self(x * 12.92)
		} else {
			Self(1.055 * x.powf(1. / 2.4) - 0.055)
		}
	}
}
pub trait RGBPrimaries {
	const RED: DVec2;
	const GREEN: DVec2;
	const BLUE: DVec2;
	const WHITE: DVec2;
}
pub trait Rec709Primaries {}
impl<T: Rec709Primaries> RGBPrimaries for T {
	const RED: DVec2 = DVec2::new(0.64, 0.33);
	const GREEN: DVec2 = DVec2::new(0.3, 0.6);
	const BLUE: DVec2 = DVec2::new(0.15, 0.06);
	const WHITE: DVec2 = DVec2::new(0.3127, 0.329);
}

pub trait SRGB: Rec709Primaries {}

#[cfg(feature = "serde")]
pub trait Serde: serde::Serialize + for<'a> serde::Deserialize<'a> {}
#[cfg(not(feature = "serde"))]
pub trait Serde {}

#[cfg(feature = "serde")]
impl<T: serde::Serialize + for<'a> serde::Deserialize<'a>> Serde for T {}
#[cfg(not(feature = "serde"))]
impl<T> Serde for T {}

// TODO: Come up with a better name for this trait
pub trait Pixel: Clone + Pod + Zeroable {
	#[cfg(not(target_arch = "spirv"))]
	fn to_bytes(&self) -> Vec<u8> {
		bytemuck::bytes_of(self).to_vec()
	}
	// TODO: use u8 for Color
	fn from_bytes(bytes: &[u8]) -> Self {
		*bytemuck::try_from_bytes(bytes).expect("Failed to convert bytes to pixel")
	}

	fn byte_size() -> usize {
		core::mem::size_of::<Self>()
	}
}
pub trait RGB: Pixel {
	type ColorChannel: Channel;

	fn red(&self) -> Self::ColorChannel;
	fn r(&self) -> Self::ColorChannel {
		self.red()
	}
	fn green(&self) -> Self::ColorChannel;
	fn g(&self) -> Self::ColorChannel {
		self.green()
	}
	fn blue(&self) -> Self::ColorChannel;
	fn b(&self) -> Self::ColorChannel {
		self.blue()
	}
}
pub trait RGBMut: RGB {
	fn set_red(&mut self, red: Self::ColorChannel);
	fn set_green(&mut self, green: Self::ColorChannel);
	fn set_blue(&mut self, blue: Self::ColorChannel);
}

pub trait AssociatedAlpha: RGB + Alpha {
	fn to_unassociated<Out: UnassociatedAlpha>(&self) -> Out;
}

pub trait UnassociatedAlpha: RGB + Alpha {
	fn to_associated<Out: AssociatedAlpha>(&self) -> Out;
}

pub trait Alpha {
	type AlphaChannel: LinearChannel;
	const TRANSPARENT: Self;
	fn alpha(&self) -> Self::AlphaChannel;
	fn a(&self) -> Self::AlphaChannel {
		self.alpha()
	}
	fn multiplied_alpha(&self, alpha: Self::AlphaChannel) -> Self;
}

pub trait Depth {
	type DepthChannel: Channel;
	fn depth(&self) -> Self::DepthChannel;
	fn d(&self) -> Self::DepthChannel {
		self.depth()
	}
}

pub trait ExtraChannels<const NUM: usize> {
	type ChannelType: Channel;
	fn extra_channels(&self) -> [Self::ChannelType; NUM];
}

pub trait Luminance {
	type LuminanceChannel: LinearChannel;
	fn luminance(&self) -> Self::LuminanceChannel;
	fn l(&self) -> Self::LuminanceChannel {
		self.luminance()
	}
}

pub trait LuminanceMut: Luminance {
	fn set_luminance(&mut self, luminance: Self::LuminanceChannel);
}

// TODO: We might rename this to Raster at some point
pub trait Sample {
	type Pixel: Pixel;
	// TODO: Add an area parameter
	fn sample(&self, pos: DVec2, area: DVec2) -> Option<Self::Pixel>;
}

impl<'i, T: Sample> Sample for &'i T {
	type Pixel = T::Pixel;

	#[inline(always)]
	fn sample(&self, pos: DVec2, area: DVec2) -> Option<Self::Pixel> {
		(**self).sample(pos, area)
	}
}

pub trait Bitmap {
	type Pixel: Pixel;
	fn width(&self) -> u32;
	fn height(&self) -> u32;
	fn get_pixel(&self, x: u32, y: u32) -> Option<Self::Pixel>;
}

impl<'i, T: Bitmap> Bitmap for &'i T {
	type Pixel = T::Pixel;

	fn width(&self) -> u32 {
		(**self).width()
	}

	fn height(&self) -> u32 {
		(**self).height()
	}

	fn get_pixel(&self, x: u32, y: u32) -> Option<Self::Pixel> {
		(**self).get_pixel(x, y)
	}
}

impl<'i, T: Bitmap> Bitmap for &'i mut T {
	type Pixel = T::Pixel;

	fn width(&self) -> u32 {
		(**self).width()
	}

	fn height(&self) -> u32 {
		(**self).height()
	}

	fn get_pixel(&self, x: u32, y: u32) -> Option<Self::Pixel> {
		(**self).get_pixel(x, y)
	}
}

pub trait BitmapMut: Bitmap {
	fn get_pixel_mut(&mut self, x: u32, y: u32) -> Option<&mut Self::Pixel>;
	fn set_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
		*self.get_pixel_mut(x, y).unwrap() = pixel;
	}
	fn map_pixels<F: Fn(Self::Pixel) -> Self::Pixel>(&mut self, map_fn: F) {
		for y in 0..self.height() {
			for x in 0..self.width() {
				let pixel = self.get_pixel(x, y).unwrap();
				self.set_pixel(x, y, map_fn(pixel));
			}
		}
	}
}

impl<'i, T: BitmapMut + Bitmap> BitmapMut for &'i mut T {
	fn get_pixel_mut(&mut self, x: u32, y: u32) -> Option<&mut Self::Pixel> {
		(*self).get_pixel_mut(x, y)
	}
}

#[derive(Debug, Default)]
pub struct MapNode<MapFn> {
	map_fn: MapFn,
}

#[node_macro::node_fn(MapNode)]
fn map_node<_Iter: Iterator, MapFnNode>(input: _Iter, map_fn: &'input MapFnNode) -> MapFnIterator<'input, _Iter, MapFnNode>
where
	MapFnNode: for<'any_input> Node<'any_input, _Iter::Item>,
{
	MapFnIterator::new(input, map_fn)
}

#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct MapFnIterator<'i, Iter, MapFn> {
	iter: Iter,
	map_fn: &'i MapFn,
}

impl<'i, Iter: Debug, MapFn> Debug for MapFnIterator<'i, Iter, MapFn> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("MapFnIterator").field("iter", &self.iter).field("map_fn", &"MapFn").finish()
	}
}

impl<'i, Iter: Clone, MapFn> Clone for MapFnIterator<'i, Iter, MapFn> {
	fn clone(&self) -> Self {
		Self {
			iter: self.iter.clone(),
			map_fn: self.map_fn,
		}
	}
}
impl<'i, Iter: Copy, MapFn> Copy for MapFnIterator<'i, Iter, MapFn> {}

impl<'i, Iter, MapFn> MapFnIterator<'i, Iter, MapFn> {
	pub fn new(iter: Iter, map_fn: &'i MapFn) -> Self {
		Self { iter, map_fn }
	}
}

impl<'i, I: Iterator + 'i, F> Iterator for MapFnIterator<'i, I, F>
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
pub struct ImageIndexIterNode<P> {
	_p: core::marker::PhantomData<P>,
}

#[node_macro::node_fn(ImageIndexIterNode<_P>)]
fn image_index_iter_node<_P>(input: ImageSlice<'input, _P>) -> core::ops::Range<u32> {
	0..(input.width * input.height)
}

#[derive(Debug)]
pub struct WindowNode<P, Radius: for<'i> Node<'i, (), Output = u32>, Image: for<'i> Node<'i, (), Output = ImageSlice<'i, P>>> {
	radius: Radius,
	image: Image,
	_pixel: core::marker::PhantomData<P>,
}

impl<'input, P: 'input, S0: 'input, S1: 'input> Node<'input, u32> for WindowNode<P, S0, S1>
where
	S0: for<'any_input> Node<'any_input, (), Output = u32>,
	S1: for<'any_input> Node<'any_input, (), Output = ImageSlice<'any_input, P>>,
{
	type Output = ImageWindowIterator<'input, P>;
	#[inline]
	fn eval(&'input self, input: u32) -> Self::Output {
		let radius = self.radius.eval(());
		let image = self.image.eval(());
		{
			let iter = ImageWindowIterator::new(image, radius, input);
			iter
		}
	}
}
impl<P, S0, S1> WindowNode<P, S0, S1>
where
	S0: for<'any_input> Node<'any_input, (), Output = u32>,
	S1: for<'any_input> Node<'any_input, (), Output = ImageSlice<'any_input, P>>,
{
	pub const fn new(radius: S0, image: S1) -> Self {
		Self {
			radius,
			image,
			_pixel: core::marker::PhantomData,
		}
	}
}
/*
#[node_macro::node_fn(WindowNode)]
fn window_node(input: u32, radius: u32, image: ImageSlice<'input>) -> ImageWindowIterator<'input> {
	let iter = ImageWindowIterator::new(image, radius, input);
	iter
}*/

#[derive(Debug, Clone, Copy)]
pub struct ImageWindowIterator<'a, P> {
	image: ImageSlice<'a, P>,
	radius: u32,
	index: u32,
	x: u32,
	y: u32,
}

impl<'a, P> ImageWindowIterator<'a, P> {
	fn new(image: ImageSlice<'a, P>, radius: u32, index: u32) -> Self {
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

#[cfg(not(target_arch = "spirv"))]
impl<'a, P: Copy> Iterator for ImageWindowIterator<'a, P> {
	type Item = (P, (i32, i32));
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
		#[cfg(target_arch = "spirv")]
		let value = None;
		#[cfg(not(target_arch = "spirv"))]
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
pub struct MapSecondNode<First, Second, MapFn> {
	map_fn: MapFn,
	_first: PhantomData<First>,
	_second: PhantomData<Second>,
}

#[node_macro::node_fn(MapSecondNode< _First, _Second>)]
fn map_snd_node<MapFn, _First, _Second>(input: (_First, _Second), map_fn: &'input MapFn) -> (_First, <MapFn as Node<'input, _Second>>::Output)
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
pub struct ForEachNode<MapNode> {
	map_node: MapNode,
}

#[node_macro::node_fn(ForEachNode)]
fn map_node<_Iter: Iterator, MapNode>(input: _Iter, map_node: &'input MapNode) -> ()
where
	MapNode: for<'any_input> Node<'any_input, _Iter::Item, Output = ()> + 'input,
{
	input.for_each(|x| map_node.eval(x));
}

#[cfg(target_arch = "spirv")]
const NOTHING: () = ();

use dyn_any::{StaticType, StaticTypeSized};
#[derive(Clone, Debug, PartialEq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ImageSlice<'a, Pixel> {
	pub width: u32,
	pub height: u32,
	#[cfg(not(target_arch = "spirv"))]
	pub data: &'a [Pixel],
	#[cfg(target_arch = "spirv")]
	pub data: &'a (),
	#[cfg(target_arch = "spirv")]
	pub _marker: PhantomData<Pixel>,
}

unsafe impl<P: StaticTypeSized> StaticType for ImageSlice<'_, P> {
	type Static = ImageSlice<'static, P::Static>;
}

#[allow(clippy::derivable_impls)]
impl<'a, P> Default for ImageSlice<'a, P> {
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
			_marker: PhantomData,
		}
	}
}

#[cfg(not(target_arch = "spirv"))]
impl<P: Copy + Debug + Pixel> Bitmap for ImageSlice<'_, P> {
	type Pixel = P;
	fn get_pixel(&self, x: u32, y: u32) -> Option<P> {
		self.data.get((x + y * self.width) as usize).copied()
	}
	fn width(&self) -> u32 {
		self.width
	}
	fn height(&self) -> u32 {
		self.height
	}
}

impl<P> ImageSlice<'_, P> {
	#[cfg(not(target_arch = "spirv"))]
	pub const fn empty() -> Self {
		Self { width: 0, height: 0, data: &[] }
	}
}

#[cfg(not(target_arch = "spirv"))]
impl<'a, P: 'a> IntoIterator for ImageSlice<'a, P> {
	type Item = &'a P;
	type IntoIter = core::slice::Iter<'a, P>;
	fn into_iter(self) -> Self::IntoIter {
		self.data.iter()
	}
}

#[cfg(not(target_arch = "spirv"))]
impl<'a, P: 'a> IntoIterator for &'a ImageSlice<'a, P> {
	type Item = &'a P;
	type IntoIter = core::slice::Iter<'a, P>;
	fn into_iter(self) -> Self::IntoIter {
		self.data.iter()
	}
}

#[derive(Debug)]
pub struct ImageDimensionsNode<P> {
	_p: PhantomData<P>,
}

#[node_macro::node_fn(ImageDimensionsNode<_P>)]
fn dimensions_node<_P>(input: ImageSlice<'input, _P>) -> (u32, u32) {
	(input.width, input.height)
}

#[cfg(feature = "alloc")]
pub use self::image::{CollectNode, Image, ImageFrame, ImageRefNode, MapImageSliceNode};
#[cfg(feature = "alloc")]
pub(crate) mod image;

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
			base64_string: None,
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
	// #[test]
	// fn blur_node() {
	// 	use alloc::vec;
	// 	let radius = ValueNode::new(1u32).then(CloneNode::new());
	// 	let sigma = ValueNode::new(3f64).then(CloneNode::new());
	// 	let radius = ValueNode::new(1u32).then(CloneNode::new());
	// 	let image = ValueNode::<_>::new(Image {
	// 		width: 5,
	// 		height: 5,
	// 		data: vec![Color::from_rgbf32_unchecked(1., 0., 0.); 25],
	// 	});
	// 	let image = image.then(ImageRefNode::new());
	// 	let window = WindowNode::new(radius, image);
	// 	let window: TypeNode<_, u32, ImageWindowIterator<'_>> = TypeNode::new(window);
	// 	let distance = ValueNode::new(DistanceNode::new());
	// 	let pos_to_dist = MapSecondNode::new(distance);
	// 	let type_erased = &window as &dyn for<'a> Node<'a, u32, Output = ImageWindowIterator<'a>>;
	// 	type_erased.eval(0);
	// 	let map_pos_to_dist = MapNode::new(ValueNode::new(pos_to_dist));

	// let type_erased = &map_pos_to_dist as &dyn for<'a> Node<'a, u32, Output = ImageWindowIterator<'a>>;
	// type_erased.eval(0);

	// 	let distance = window.then(map_pos_to_dist);
	// 	let map_gaussian = MapSecondNode::new(ValueNode(GaussianNode::new(sigma)));
	// 	let map_gaussian: TypeNode<_, (_, f32), (_, f32)> = TypeNode::new(map_gaussian);
	// 	let map_gaussian = ValueNode(map_gaussian);
	// 	let map_gaussian: TypeNode<_, (), &_> = TypeNode::new(map_gaussian);
	// 	let map_distances = MapNode::new(map_gaussian);
	// 	let map_distances: TypeNode<_, _, MapFnIterator<'_, '_, _, _>> = TypeNode::new(map_distances);
	// 	let gaussian_iter = distance.then(map_distances);
	// 	let avg = gaussian_iter.then(WeightedAvgNode::new());
	// 	let avg: TypeNode<_, u32, Color> = TypeNode::new(avg);
	// 	let blur_iter = MapNode::new(ValueNode::new(avg));
	// 	let blur = image.then(ImageIndexIterNode).then(blur_iter);
	// 	let blur: TypeNode<_, (), MapFnIterator<_, _>> = TypeNode::new(blur);
	// 	let collect = CollectNode::new();
	// 	let vec = collect.eval(0..10);
	// 	assert_eq!(vec.len(), 10);
	// 	let _ = blur.eval(());
	// 	let vec = blur.then(collect);
	// 	let _image = vec.eval(());
	// }
}
