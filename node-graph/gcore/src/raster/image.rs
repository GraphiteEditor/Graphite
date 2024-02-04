use super::discrete_srgb::float_to_srgb_u8;
use super::{Color, ImageSlice};
use crate::{AlphaBlending, Node};
use alloc::vec::Vec;
use core::hash::{Hash, Hasher};
use dyn_any::StaticType;
use glam::{DAffine2, DVec2};

#[cfg(feature = "serde")]
mod base64_serde {
	//! Basic wrapper for [`serde`] to perform [`base64`] encoding

	use super::super::Pixel;
	use base64::Engine;
	use serde::{Deserialize, Deserializer, Serializer};

	pub fn as_base64<S, P: Pixel>(key: &[P], serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let u8_data = key.iter().flat_map(|color| color.to_bytes()).collect::<Vec<_>>();
		serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(u8_data))
	}

	pub fn from_base64<'a, D, P: Pixel>(deserializer: D) -> Result<Vec<P>, D::Error>
	where
		D: Deserializer<'a>,
	{
		use serde::de::Error;

		let color_from_chunk = |chunk: &[u8]| P::from_bytes(chunk);

		let colors_from_bytes = |bytes: Vec<u8>| bytes.chunks_exact(P::byte_size()).map(color_from_chunk).collect();

		String::deserialize(deserializer)
			.and_then(|string| base64::engine::general_purpose::STANDARD.decode(string).map_err(|err| Error::custom(err.to_string())))
			.map(colors_from_bytes)
			.map_err(serde::de::Error::custom)
	}
}

#[derive(Clone, PartialEq, Default, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Image<P: Pixel> {
	pub width: u32,
	pub height: u32,
	#[cfg_attr(feature = "serde", serde(serialize_with = "base64_serde::as_base64", deserialize_with = "base64_serde::from_base64"))]
	pub data: Vec<P>,
	/// Optional: Stores a base64 string representation of the image which can be used to speed up the conversion
	/// to an svg string. This is used as a cache in order to not have to encode the data on every graph evaluation.
	#[cfg_attr(feature = "serde", serde(skip))]
	pub base64_string: Option<String>,
}

impl<P: Pixel + Debug> Debug for Image<P> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		let length = self.data.len();
		f.debug_struct("Image")
			.field("width", &self.width)
			.field("height", &self.height)
			.field("data", if length < 100 { &self.data } else { &length })
			.finish()
	}
}

unsafe impl<P: StaticTypeSized + Pixel> StaticType for Image<P>
where
	P::Static: Pixel,
{
	type Static = Image<P::Static>;
}

impl<P: Copy + Pixel> Bitmap for Image<P> {
	type Pixel = P;
	#[inline(always)]
	fn get_pixel(&self, x: u32, y: u32) -> Option<P> {
		self.data.get((x + y * self.width) as usize).copied()
	}
	#[inline(always)]
	fn width(&self) -> u32 {
		self.width
	}
	#[inline(always)]
	fn height(&self) -> u32 {
		self.height
	}
}

impl<P: Copy + Pixel> BitmapMut for Image<P> {
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
			base64_string: None,
		}
	}

	pub fn new(width: u32, height: u32, color: P) -> Self {
		Self {
			width,
			height,
			data: vec![color; (width * height) as usize],
			base64_string: None,
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
		Image {
			width,
			height,
			data,
			base64_string: None,
		}
	}

	pub fn to_png(&self) -> Vec<u8> {
		use ::image::ImageEncoder;
		let (data, width, height) = self.to_flat_u8();
		let mut png = Vec::new();
		let encoder = ::image::codecs::png::PngEncoder::new(&mut png);
		encoder.write_image(&data, width, height, ::image::ColorType::Rgba8).expect("failed to encode image as png");
		png
	}
}

use super::*;
impl<P: Alpha + RGB + AssociatedAlpha> Image<P>
where
	P::ColorChannel: Linear,
	<P as Alpha>::AlphaChannel: Linear,
{
	/// Flattens each channel cast to a u8
	pub fn to_flat_u8(&self) -> (Vec<u8>, u32, u32) {
		let Image { width, height, data, .. } = self;
		assert_eq!(data.len(), *width as usize * *height as usize);

		// Cache the last sRGB value we computed, speeds up fills.
		let mut last_r = 0.;
		let mut last_r_srgb = 0u8;
		let mut last_g = 0.;
		let mut last_g_srgb = 0u8;
		let mut last_b = 0.;
		let mut last_b_srgb = 0u8;

		let mut result = vec![0; data.len() * 4];
		let mut i = 0;
		for color in data {
			let a = color.a().to_f32();
			// Smaller alpha values than this would map to fully transparent
			// anyway, avoid expensive encoding.
			if a >= 0.5 / 255. {
				let undo_premultiply = 1. / a;
				let r = color.r().to_f32() * undo_premultiply;
				let g = color.g().to_f32() * undo_premultiply;
				let b = color.b().to_f32() * undo_premultiply;

				// Compute new sRGB value if necessary.
				if r != last_r {
					last_r = r;
					last_r_srgb = float_to_srgb_u8(r);
				}
				if g != last_g {
					last_g = g;
					last_g_srgb = float_to_srgb_u8(g);
				}
				if b != last_b {
					last_b = b;
					last_b_srgb = float_to_srgb_u8(b);
				}

				result[i] = last_r_srgb;
				result[i + 1] = last_g_srgb;
				result[i + 2] = last_b_srgb;
				result[i + 3] = (a * 255. + 0.5) as u8;
			}

			i += 4;
		}

		(result, *width, *height)
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
		base64_string: None,
	}
}

#[derive(Clone, Debug, PartialEq, Default, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageFrame<P: Pixel> {
	pub image: Image<P>,
	// The transform that maps image space to layer space.
	//
	// Image space is unitless [0, 1] for both axes, with x axis positive
	// going right and y axis positive going down, with the origin lying at
	// the topleft of the image and (1, 1) lying at the bottom right of the image.
	//
	// Layer space has pixels as its units for both axes, with the x axis
	// positive going right and y axis positive going down, with the origin
	// being an unspecified quantity.
	pub transform: DAffine2,
	pub alpha_blending: AlphaBlending,
}

impl<P: Debug + Copy + Pixel> Sample for ImageFrame<P> {
	type Pixel = P;

	// TODO: Improve sampling logic
	#[inline(always)]
	fn sample(&self, pos: DVec2, _area: DVec2) -> Option<Self::Pixel> {
		let image_size = DVec2::new(self.image.width() as f64, self.image.height() as f64);
		let pos = (DAffine2::from_scale(image_size) * self.transform.inverse()).transform_point2(pos);
		if pos.x < 0. || pos.y < 0. || pos.x >= image_size.x || pos.y >= image_size.y {
			return None;
		}
		self.image.get_pixel(pos.x as u32, pos.y as u32)
	}
}

impl<P: Copy + Pixel> Bitmap for ImageFrame<P> {
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

impl<P: Copy + Pixel> BitmapMut for ImageFrame<P> {
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
			transform: DAffine2::ZERO,
			alpha_blending: AlphaBlending::new(),
		}
	}

	pub const fn identity() -> Self {
		Self {
			image: Image::empty(),
			transform: DAffine2::IDENTITY,
			alpha_blending: AlphaBlending::new(),
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

impl<P: Pixel> ImageFrame<P> {
	/// Compute the pivot in local space with the current transform applied
	pub fn local_pivot(&self, normalized_pivot: DVec2) -> DVec2 {
		self.transform.transform_point2(normalized_pivot)
	}
}

/* This does not work because of missing specialization
 * so we have to manually implement this for now
impl<S: Into<P> + Pixel, P: Pixel> From<Image<S>> for Image<P> {
	fn from(image: Image<S>) -> Self {
		let data = image.data.into_iter().map(|x| x.into()).collect();
		Self {
			data,
			width: image.width,
			height: image.height,
		}
	}
}*/

impl From<ImageFrame<Color>> for ImageFrame<SRGBA8> {
	fn from(image: ImageFrame<Color>) -> Self {
		let data = image.image.data.into_iter().map(|x| x.into()).collect();
		Self {
			image: Image {
				data,
				width: image.image.width,
				height: image.image.height,
				base64_string: None,
			},
			transform: image.transform,
			alpha_blending: image.alpha_blending,
		}
	}
}

impl From<ImageFrame<SRGBA8>> for ImageFrame<Color> {
	fn from(image: ImageFrame<SRGBA8>) -> Self {
		let data = image.image.data.into_iter().map(|x| x.into()).collect();
		Self {
			image: Image {
				data,
				width: image.image.width,
				height: image.image.height,
				base64_string: None,
			},
			transform: image.transform,
			alpha_blending: image.alpha_blending,
		}
	}
}
