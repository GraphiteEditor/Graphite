use crate::{Bitmap, BitmapMut};
use core_types::Color;
use core_types::color::float_to_srgb_u8;
// use crate::vector::Vector; // TODO: Check if Vector is actually used, if so handle differently
use core_types::color::*;
use dyn_any::StaticType;
use glam::{DAffine2, DVec2};
use std::vec::Vec;

mod base64_serde {
	//! Basic wrapper for [`serde`] to perform [`base64`] encoding

	use base64::Engine;
	use core_types::color::*;
	use serde::{Deserializer, Serialize, Serializer};

	pub fn as_base64<S: Serializer, P: Pixel>(key: &[P], serializer: S) -> Result<S::Ok, S::Error> {
		let u8_data = bytemuck::cast_slice(key);
		let string = base64::engine::general_purpose::STANDARD.encode(u8_data);
		(key.len() as u64, string).serialize(serializer)
	}

	pub fn from_base64<'a, D: Deserializer<'a>, P: Pixel>(deserializer: D) -> Result<Vec<P>, D::Error> {
		use serde::de::Error;
		// Use a small visitor that accepts both borrowed bytes (from a streaming JSON deserializer) and owned strings (from an intermediate like `serde_json::Value`, which can't preserve the borrow).
		// The migration loader takes the second path, so without this allowance documents containing image base64 data fail with `expected a borrowed byte array`.
		struct LenAndBase64Visitor<P: Pixel>(std::marker::PhantomData<P>);

		impl<'de, P: Pixel> serde::de::Visitor<'de> for LenAndBase64Visitor<P> {
			type Value = Vec<P>;

			fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
				f.write_str("a tuple of (length, base64-encoded data)")
			}

			fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
				let len: u64 = seq.next_element()?.ok_or_else(|| A::Error::missing_field("length"))?;
				let base64_string: std::borrow::Cow<'de, str> = seq.next_element()?.ok_or_else(|| A::Error::missing_field("base64 data"))?;
				let mut output: Vec<P> = vec![P::zeroed(); len as usize];
				base64::engine::general_purpose::STANDARD
					.decode_slice(base64_string.as_bytes(), bytemuck::cast_slice_mut(output.as_mut_slice()))
					.map_err(|err| A::Error::custom(err.to_string()))?;
				Ok(output)
			}
		}

		deserializer.deserialize_tuple(2, LenAndBase64Visitor::<P>(std::marker::PhantomData))
	}
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Eq, Default)]
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
	// TODO: Add an `origin` field to store where in the local space the image is anchored.
	// TODO: Currently it is always anchored at the top left corner at (0, 0). The bottom right corner of the new origin field would correspond to (1, 1).
}

impl<P: Pixel + PartialEq> PartialEq for Image<P> {
	fn eq(&self, other: &Self) -> bool {
		self.width == other.width && self.height == other.height && self.data == other.data
	}
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, dyn_any::DynAny, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TransformImage(pub DAffine2);

impl core_types::CacheHash for TransformImage {
	fn cache_hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
		core_types::CacheHash::cache_hash(&self.0, state);
	}
}

impl<P: Pixel + std::fmt::Debug> std::fmt::Debug for Image<P> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let length = self.data.len();
		f.debug_struct("Image")
			.field("width", &self.width)
			.field("height", &self.height)
			.field("data", if length < 100 { &self.data } else { &length })
			.finish()
	}
}

unsafe impl<P> StaticType for Image<P>
where
	P: dyn_any::StaticTypeSized + Pixel,
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

impl<P: core_types::CacheHash + Pixel> core_types::CacheHash for Image<P> {
	fn cache_hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
		core_types::CacheHash::cache_hash(&self.width, state);
		core_types::CacheHash::cache_hash(&self.height, state);
		core_types::CacheHash::cache_hash(&self.data, state);
	}
}

impl<P: Pixel> Image<P> {
	pub fn new(width: u32, height: u32, color: P) -> Self {
		Self {
			width,
			height,
			data: vec![color; (width * height) as usize],
			base64_string: None,
		}
	}
}

impl Image<Color> {
	/// Generate Image from some frontend image data (the canvas pixels as u8s in a flat array)
	pub fn from_image_data(image_data: &[u8], width: u32, height: u32) -> Self {
		let data = image_data.chunks_exact(4).map(|v| SRGBA8::new(v[0], v[1], v[2], v[3]).into()).collect();
		Image {
			width,
			height,
			data,
			base64_string: None,
		}
	}

	/// Decodes an image file of any supported format.
	pub fn from_encoded(data: &[u8]) -> Option<Self> {
		let image = decode(data)?;

		// Float samples hold linear light, as in HDR and EXR files, while integer samples are gamma encoded
		let linear = matches!(image.color(), ::image::ColorType::Rgb32F | ::image::ColorType::Rgba32F);
		// An EXR file stores its color multiplied by its alpha, unlike the other formats and unlike `Color`
		let premultiplied = ::image::guess_format(data).is_ok_and(|format| format == ::image::ImageFormat::OpenExr);
		// Light brighter than white is clipped, since adjustments, the GPU upload, and export all work within this range
		let in_range = |value: f32| if value.is_nan() { 0. } else { value.clamp(0., 1.) };

		let image = image.to_rgba32f();
		let data = image
			.chunks_exact(4)
			.map(|pixel| {
				if !linear {
					return Color::from_gamma_srgb_channels(pixel[0], pixel[1], pixel[2], pixel[3]);
				}

				let alpha = in_range(pixel[3]);
				let divisor = if premultiplied && alpha > 0. { alpha } else { 1. };
				Color::from_rgbaf32_unchecked(in_range(pixel[0] / divisor), in_range(pixel[1] / divisor), in_range(pixel[2] / divisor), alpha)
			})
			.collect();

		Some(Image {
			width: image.width(),
			height: image.height(),
			data,
			base64_string: None,
		})
	}

	/// The pixel size of an image file, if it decodes in full as [`Image::from_encoded`] needs it to.
	pub fn encoded_size(data: &[u8]) -> Option<(u32, u32)> {
		decode(data).map(|image| (image.width(), image.height()))
	}

	pub fn to_png(&self) -> Vec<u8> {
		use ::image::ImageEncoder;
		let (data, width, height) = self.to_flat_u8();
		let mut png = Vec::new();
		let encoder = ::image::codecs::png::PngEncoder::new(&mut png);
		encoder.write_image(&data, width, height, ::image::ExtendedColorType::Rgba8).expect("failed to encode image as png");
		png
	}
}

/// A TGA file has no signature to recognize it by, so a file of no recognized format is tried as one.
fn decode(data: &[u8]) -> Option<::image::DynamicImage> {
	let format = ::image::guess_format(data).unwrap_or(::image::ImageFormat::Tga);
	::image::load_from_memory_with_format(data, format).ok()
}

use super::*;
impl<P: Alpha + RGB> Image<P>
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
		for (color, out) in data.iter().zip(result.chunks_exact_mut(4)) {
			let a = color.a().to_f32();
			// Smaller alpha values than this would map to fully transparent
			// anyway, avoid expensive encoding.
			if a >= 0.5 / 255. {
				let r = color.r().to_f32();
				let g = color.g().to_f32();
				let b = color.b().to_f32();

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

				out[0] = last_r_srgb;
				out[1] = last_g_srgb;
				out[2] = last_b_srgb;
				out[3] = (a * 255. + 0.5) as u8;
			}
		}

		(result, *width, *height)
	}
}

impl<P: Pixel> IntoIterator for Image<P> {
	type Item = P;
	type IntoIter = std::vec::IntoIter<P>;
	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
	}
}

impl<P: std::fmt::Debug + Copy + Pixel> Sample for Image<P> {
	type Pixel = P;

	// TODO: Improve sampling logic
	#[inline(always)]
	fn sample(&self, pos: DVec2, _area: DVec2) -> Option<Self::Pixel> {
		let image_size = DVec2::new(self.width() as f64, self.height() as f64);
		if pos.x < 0. || pos.y < 0. || pos.x >= image_size.x || pos.y >= image_size.y {
			return None;
		}
		self.get_pixel(pos.x as u32, pos.y as u32)
	}
}

impl<P: Copy + Pixel> Image<P> {
	pub fn get_mut(&mut self, x: usize, y: usize) -> &mut P {
		&mut self.data[y * (self.width as usize) + x]
	}

	/// Clamps the provided point to ((0, 0), (ImageSize.x, ImageSize.y)) and returns the closest pixel
	pub fn sample(&self, position: DVec2) -> P {
		let x = position.x.clamp(0., self.width as f64 - 1.) as usize;
		let y = position.y.clamp(0., self.height as f64 - 1.) as usize;

		self.data[x + y * self.width as usize]
	}
}

impl<P: Pixel> AsRef<Image<P>> for Image<P> {
	fn as_ref(&self) -> &Image<P> {
		self
	}
}

#[cfg(test)]
mod test {
	#[test]
	fn test_image_serialization_roundtrip() {
		use super::*;
		use crate::Color;
		let image = Image {
			width: 2,
			height: 2,
			data: vec![Color::WHITE, Color::BLACK, Color::RED, Color::GREEN],
			base64_string: None,
		};

		let serialized = serde_json::to_string(&image).unwrap();
		println!("{serialized}");
		let deserialized: Image<Color> = serde_json::from_str(&serialized).unwrap();
		println!("{deserialized:?}");

		assert_eq!(image, deserialized);
	}

	#[test]
	fn image_data_round_trips_translucent_pixels() {
		use super::*;
		let bytes = [255, 0, 0, 128, 0, 255, 0, 1, 255, 255, 255, 41, 10, 20, 30, 255];

		let image = Image::from_image_data(&bytes, 4, 1);

		assert_eq!(image.to_flat_u8().0, bytes);
	}

	#[test]
	fn decodes_each_format_including_one_with_no_signature() {
		use super::*;
		use ::image::ImageFormat::{Bmp, Ico, Png, Tga, Tiff, WebP};

		// Opaque red, then half transparent blue
		let pixels = ::image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 128]).unwrap();

		for format in [Png, WebP, Tiff, Bmp, Tga, Ico] {
			let mut encoded = Vec::new();
			pixels.write_to(&mut std::io::Cursor::new(&mut encoded), format).unwrap();

			let image = Image::from_encoded(&encoded).unwrap_or_else(|| panic!("{format:?} should decode"));
			assert_eq!(Image::encoded_size(&encoded), Some((2, 1)), "{format:?}");
			assert_eq!(image.to_flat_u8().0, pixels.as_raw().as_slice(), "{format:?}");
		}

		assert!(Image::from_encoded(b"not an image").is_none());
	}

	#[test]
	fn recognized_format_that_cannot_be_read_is_not_tried_as_tga() {
		use super::*;

		// The signature of a PNM file, a format that is recognized but not read, begins what is also a well-formed TGA header
		let mut file = vec![0; 18];
		file[..3].copy_from_slice(b"P6\n");
		// One pixel wide and tall, at 24 bits per pixel and per color map entry, since the `6` reads as having a color map
		file[7] = 24;
		file[12] = 1;
		file[14] = 1;
		file[16] = 24;
		// The ID field whose length the `P` gives, then a run of one blue pixel
		file.extend([0; b'P' as usize]);
		file.extend([0, 255, 0, 0]);

		assert!(::image::load_from_memory_with_format(&file, ::image::ImageFormat::Tga).is_ok());
		assert!(Image::from_encoded(&file).is_none());
	}

	#[test]
	fn float_samples_are_read_as_linear_light_within_range() {
		use super::*;

		let exr = |image: ::image::DynamicImage| {
			let mut encoded = Vec::new();
			image.write_to(&mut std::io::Cursor::new(&mut encoded), ::image::ImageFormat::OpenExr).unwrap();
			Image::from_encoded(&encoded).unwrap().data[0]
		};

		// Not decoded as gamma, light brighter than white clipped, and a sample that is not a number zeroed
		let color = exr(::image::DynamicImage::ImageRgb32F(::image::Rgb32FImage::from_raw(1, 1, vec![0.5, 2., f32::NAN]).unwrap()));
		assert_eq!((color.r(), color.g(), color.b(), color.a()), (0.5, 1., 0., 1.));

		// Color stored multiplied by its alpha comes out straight, and stays as stored where the alpha is zero
		let color = exr(::image::DynamicImage::ImageRgba32F(::image::Rgba32FImage::from_raw(1, 1, vec![0.25, 0.125, 0., 0.5]).unwrap()));
		assert_eq!((color.r(), color.g(), color.b(), color.a()), (0.5, 0.25, 0., 0.5));
		let color = exr(::image::DynamicImage::ImageRgba32F(::image::Rgba32FImage::from_raw(1, 1, vec![0.25, 0., 0., 0.]).unwrap()));
		assert_eq!((color.r(), color.a()), (0.25, 0.));
	}
}
