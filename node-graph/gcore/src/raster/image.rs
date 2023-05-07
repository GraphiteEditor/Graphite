use super::{Color, ImageSlice};
use crate::Node;
use alloc::vec::Vec;
use core::hash::{Hash, Hasher};
use dyn_any::StaticType;
use glam::{DAffine2, DVec2};

#[cfg(feature = "serde")]
mod base64_serde {
	//! Basic wrapper for [`serde`] to perform [`base64`] encoding

	use super::super::Pixel;
	use serde::{Deserialize, Deserializer, Serializer};

	pub fn as_base64<S, P: Pixel>(key: &Vec<P>, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let u8_data = key.iter().flat_map(|color| color.to_bytes()).collect::<Vec<_>>();
		serializer.serialize_str(&base64::encode(u8_data))
	}

	pub fn from_base64<'a, D, P: Pixel>(deserializer: D) -> Result<Vec<P>, D::Error>
	where
		D: Deserializer<'a>,
	{
		use serde::de::Error;

		let color_from_chunk = |chunk: &[u8]| P::from_bytes(chunk.try_into().unwrap());

		let colors_from_bytes = |bytes: Vec<u8>| bytes.chunks_exact(P::byte_size()).map(color_from_chunk).collect();

		String::deserialize(deserializer)
			.and_then(|string| base64::decode(string).map_err(|err| Error::custom(err.to_string())))
			.map(colors_from_bytes)
			.map_err(serde::de::Error::custom)
	}
}

#[derive(Clone, Debug, PartialEq, Default, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Image<P: Pixel> {
	pub width: u32,
	pub height: u32,
	#[cfg_attr(feature = "serde", serde(serialize_with = "base64_serde::as_base64", deserialize_with = "base64_serde::from_base64"))]
	pub data: Vec<P>,
}

unsafe impl<P: StaticTypeSized + Pixel> StaticType for Image<P>
where
	P::Static: Pixel,
{
	type Static = Image<P::Static>;
}

impl<P: Copy + Pixel> Raster for Image<P> {
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

impl<P: Copy + Pixel> RasterMut for Image<P> {
	fn get_pixel_mut(&mut self, x: u32, y: u32) -> Option<&mut P> {
		self.data.get_mut((x + y * self.width) as usize)
	}
}

// TODO: Evaluate if this will be a problem for our use case.
/// Warning: This is an approximation of a hash, and is not guaranteed to not collide.
impl<P: Hash + Pixel> Hash for Image<P> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		const HASH_SAMPLES: u64 = 1000;
		let data_length = self.data.len() as u64;
		self.width.hash(state);
		self.height.hash(state);
		for i in 0..HASH_SAMPLES.min(data_length) {
			self.data[(i * data_length / HASH_SAMPLES) as usize].hash(state);
		}
	}
}

impl<P: Pixel> Image<P> {
	pub const fn empty() -> Self {
		Self {
			width: 0,
			height: 0,
			data: Vec::new(),
		}
	}

	pub fn new(width: u32, height: u32, color: P) -> Self {
		Self {
			width,
			height,
			data: vec![color; (width * height) as usize],
		}
	}

	pub fn as_slice(&self) -> ImageSlice<P> {
		ImageSlice {
			width: self.width,
			height: self.height,
			data: self.data.as_slice(),
		}
	}
}

impl Image<Color> {
	/// Generate Image from some frontend image data (the canvas pixels as u8s in a flat array)
	pub fn from_image_data(image_data: &[u8], width: u32, height: u32) -> Self {
		let data = image_data.chunks_exact(4).map(|v| Color::from_rgba8_srgb(v[0], v[1], v[2], v[3])).collect();
		Image { width, height, data }
	}
}

use super::*;
impl<P: Alpha + RGB + AssociatedAlpha> Image<P>
where
	P::ColorChannel: Linear,
{
	/// Flattens each channel cast to a u8
	pub fn into_flat_u8(self) -> (Vec<u8>, u32, u32) {
		let Image { width, height, data } = self;

		let to_gamma = SRGBGammaFloat::from_linear;
		let to_u8 = |x| (num_cast::<_, f32>(x).unwrap() * 255.) as u8;

		let result_bytes = data
			.into_iter()
			.flat_map(|color| {
				[
					to_u8(to_gamma(color.r() / color.a().to_channel())),
					to_u8(to_gamma(color.g() / color.a().to_channel())),
					to_u8(to_gamma(color.b() / color.a().to_channel())),
					(num_cast::<_, f32>(color.a()).unwrap() * 255.) as u8,
				]
			})
			.collect();

		(result_bytes, width, height)
	}
}

impl<P: Pixel> IntoIterator for Image<P> {
	type Item = P;
	type IntoIter = alloc::vec::IntoIter<P>;
	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
	}
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ImageRefNode<P> {
	_p: PhantomData<P>,
}

#[node_macro::node_fn(ImageRefNode<_P>)]
fn image_ref_node<_P: Pixel>(image: &'input Image<_P>) -> ImageSlice<'input, _P> {
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
fn map_node<P: Pixel>(input: (u32, u32), data: Vec<P>) -> Image<P> {
	Image {
		width: input.0,
		height: input.1,
		data,
	}
}

#[derive(Clone, Debug, PartialEq, Default, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageFrame<P: Pixel> {
	pub image: Image<P>,
	pub transform: DAffine2,
}

impl<P: Debug + Copy + Pixel> Sample for ImageFrame<P> {
	type Pixel = P;

	// TODO: Improve sampling logic
	fn sample(&self, pos: DVec2, _area: DVec2) -> Option<Self::Pixel> {
		let image_size = DVec2::new(self.image.width() as f64, self.image.height() as f64);
		let pos = (DAffine2::from_scale(image_size) * self.transform.inverse()).transform_point2(pos);
		if pos.x < 0. || pos.y < 0. || pos.x >= image_size.x || pos.y >= image_size.y {
			return None;
		}
		self.image.get_pixel(pos.x as u32, pos.y as u32)
	}
}

impl<P: Copy + Pixel> Raster for ImageFrame<P> {
	type Pixel = P;

	fn width(&self) -> u32 {
		self.image.width()
	}

	fn height(&self) -> u32 {
		self.image.height()
	}

	fn get_pixel(&self, x: u32, y: u32) -> Option<Self::Pixel> {
		self.image.get_pixel(x, y)
	}
}

impl<P: Copy + Pixel> RasterMut for ImageFrame<P> {
	fn get_pixel_mut(&mut self, x: u32, y: u32) -> Option<&mut Self::Pixel> {
		self.image.get_pixel_mut(x, y)
	}
}

unsafe impl<P: StaticTypeSized + Pixel> StaticType for ImageFrame<P>
where
	P::Static: Pixel,
{
	type Static = ImageFrame<P::Static>;
}

impl<P: Copy + Pixel> ImageFrame<P> {
	pub const fn empty() -> Self {
		Self {
			image: Image::empty(),
			transform: DAffine2::IDENTITY,
		}
	}

	pub fn get_mut(&mut self, x: usize, y: usize) -> &mut P {
		&mut self.image.data[y * (self.image.width as usize) + x]
	}

	/// Clamps the provided point to ((0, 0), (ImageSize.x, ImageSize.y)) and returns the closest pixel
	pub fn sample(&self, position: DVec2) -> P {
		let x = position.x.clamp(0., self.image.width as f64 - 1.) as usize;
		let y = position.y.clamp(0., self.image.height as f64 - 1.) as usize;

		self.image.data[x + y * self.image.width as usize]
	}
}

impl<P: Pixel> AsRef<ImageFrame<P>> for ImageFrame<P> {
	fn as_ref(&self) -> &ImageFrame<P> {
		self
	}
}

impl<P: Hash + Pixel> Hash for ImageFrame<P> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.transform.to_cols_array().iter().for_each(|x| x.to_bits().hash(state));
		0.hash(state);
		self.image.hash(state);
	}
}

use crate::text::FontCache;
#[derive(Clone, Debug, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EditorApi<'a> {
	#[cfg_attr(feature = "serde", serde(skip))]
	pub image_frame: Option<ImageFrame<Color>>,
	#[cfg_attr(feature = "serde", serde(skip))]
	pub font_cache: Option<&'a FontCache>,
}

unsafe impl StaticType for EditorApi<'_> {
	type Static = EditorApi<'static>;
}

impl EditorApi<'_> {
	pub fn empty() -> Self {
		Self { image_frame: None, font_cache: None }
	}
}

impl<'a> AsRef<EditorApi<'a>> for EditorApi<'a> {
	fn as_ref(&self) -> &EditorApi<'a> {
		self
	}
}

pub struct ExtractImageFrame;

impl<'a: 'input, 'input> Node<'input, EditorApi<'a>> for ExtractImageFrame {
	type Output = ImageFrame<Color>;
	fn eval(&'input self, mut editor_api: EditorApi<'a>) -> Self::Output {
		editor_api.image_frame.take().unwrap_or(ImageFrame::empty())
	}
}

impl ExtractImageFrame {
	pub fn new() -> Self {
		Self
	}
}
