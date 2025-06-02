use super::Color;
use super::discrete_srgb::float_to_srgb_u8;
use crate::AlphaBlending;
use crate::GraphicElement;
use crate::instances::{Instance, Instances};
use crate::transform::TransformMut;
use alloc::vec::Vec;
use core::hash::{Hash, Hasher};
use dyn_any::StaticType;
use glam::{DAffine2, DVec2};

#[cfg(feature = "serde")]
mod base64_serde {
	//! Basic wrapper for [`serde`] to perform [`base64`] encoding

	use super::super::Pixel;
	use base64::Engine;
	use serde::{Deserialize, Deserializer, Serialize, Serializer};

	pub fn as_base64<S, P: Pixel>(key: &[P], serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let u8_data = bytemuck::cast_slice(key);
		let string = base64::engine::general_purpose::STANDARD.encode(u8_data);
		(key.len() as u64, string).serialize(serializer)
	}

	pub fn from_base64<'a, D, P: Pixel>(deserializer: D) -> Result<Vec<P>, D::Error>
	where
		D: Deserializer<'a>,
	{
		use serde::de::Error;
		<(u64, &[u8])>::deserialize(deserializer)
			.and_then(|(len, str)| {
				let mut output: Vec<P> = vec![P::zeroed(); len as usize];
				base64::engine::general_purpose::STANDARD
					.decode_slice(str, bytemuck::cast_slice_mut(output.as_mut_slice()))
					.map_err(|err| Error::custom(err.to_string()))?;

				Ok(output)
			})
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
	// TODO: Add an `origin` field to store where in the local space the image is anchored.
	// TODO: Currently it is always anchored at the top left corner at (0, 0). The bottom right corner of the new origin field would correspond to (1, 1).
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

#[cfg(feature = "dyn-any")]
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
		encoder.write_image(&data, width, height, ::image::ExtendedColorType::Rgba8).expect("failed to encode image as png");
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

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_image_frame<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<ImageFrameTable<Color>, D::Error> {
	use serde::Deserialize;

	#[derive(Clone, Default, Debug, PartialEq, specta::Type)]
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	pub struct ImageFrame<P: Pixel> {
		pub image: Image<P>,
	}
	impl From<ImageFrame<Color>> for GraphicElement {
		fn from(image_frame: ImageFrame<Color>) -> Self {
			GraphicElement::RasterFrame(crate::RasterFrame::ImageFrame(ImageFrameTable::new(image_frame.image)))
		}
	}
	impl From<GraphicElement> for ImageFrame<Color> {
		fn from(element: GraphicElement) -> Self {
			match element {
				GraphicElement::RasterFrame(crate::RasterFrame::ImageFrame(image)) => Self {
					image: image.one_instance_ref().instance.clone(),
				},
				_ => panic!("Expected Image, found {:?}", element),
			}
		}
	}

	#[cfg(feature = "dyn-any")]
	unsafe impl<P> StaticType for ImageFrame<P>
	where
		P: dyn_any::StaticTypeSized + Pixel,
		P::Static: Pixel,
	{
		type Static = ImageFrame<P::Static>;
	}

	#[derive(Clone, Default, Debug, PartialEq, specta::Type)]
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	pub struct OldImageFrame<P: Pixel> {
		image: Image<P>,
		transform: DAffine2,
		alpha_blending: AlphaBlending,
	}

	#[derive(serde::Serialize, serde::Deserialize)]
	#[serde(untagged)]
	enum FormatVersions {
		Image(Image<Color>),
		OldImageFrame(OldImageFrame<Color>),
		ImageFrame(Instances<ImageFrame<Color>>),
		ImageFrameTable(ImageFrameTable<Color>),
	}

	Ok(match FormatVersions::deserialize(deserializer)? {
		FormatVersions::Image(image) => ImageFrameTable::new(image),
		FormatVersions::OldImageFrame(image_frame_with_transform_and_blending) => {
			let OldImageFrame { image, transform, alpha_blending } = image_frame_with_transform_and_blending;
			let mut image_frame_table = ImageFrameTable::new(image);
			*image_frame_table.one_instance_mut().transform = transform;
			*image_frame_table.one_instance_mut().alpha_blending = alpha_blending;
			image_frame_table
		}
		FormatVersions::ImageFrame(image_frame) => ImageFrameTable::new(image_frame.one_instance_ref().instance.image.clone()),
		FormatVersions::ImageFrameTable(image_frame_table) => image_frame_table,
	})
}

// TODO: Rename to ImageTable
pub type ImageFrameTable<P> = Instances<Image<P>>;

/// Construct a 0x0 image frame table. This is useful because ImageFrameTable::default() will return a 1x1 image frame table.
impl ImageFrameTable<Color> {
	pub fn one_empty_image() -> Self {
		let mut result = Self::new(Image::default());
		*result.transform_mut() = DAffine2::ZERO;
		result
	}
}

impl<P: Debug + Copy + Pixel> Sample for Image<P> {
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

impl<P> Sample for ImageFrameTable<P>
where
	P: Debug + Copy + Pixel,
	GraphicElement: From<Image<P>>,
{
	type Pixel = P;

	// TODO: Improve sampling logic
	#[inline(always)]
	fn sample(&self, pos: DVec2, area: DVec2) -> Option<Self::Pixel> {
		let image_transform = self.one_instance_ref().transform;
		let image = self.one_instance_ref().instance;

		let image_size = DVec2::new(image.width() as f64, image.height() as f64);
		let pos = (DAffine2::from_scale(image_size) * image_transform.inverse()).transform_point2(pos);

		Sample::sample(image, pos, area)
	}
}

impl<P> Bitmap for ImageFrameTable<P>
where
	P: Copy + Pixel,
	GraphicElement: From<Image<P>>,
{
	type Pixel = P;

	fn width(&self) -> u32 {
		let image = self.one_instance_ref().instance;

		image.width()
	}

	fn height(&self) -> u32 {
		let image = self.one_instance_ref().instance;

		image.height()
	}

	fn get_pixel(&self, x: u32, y: u32) -> Option<Self::Pixel> {
		let image = self.one_instance_ref().instance;

		image.get_pixel(x, y)
	}
}

impl<P> BitmapMut for ImageFrameTable<P>
where
	P: Copy + Pixel,
	GraphicElement: From<Image<P>>,
{
	fn get_pixel_mut(&mut self, x: u32, y: u32) -> Option<&mut Self::Pixel> {
		self.one_instance_mut().instance.get_pixel_mut(x, y)
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

impl From<Image<Color>> for Image<SRGBA8> {
	fn from(image: Image<Color>) -> Self {
		let data = image.data.into_iter().map(|x| x.into()).collect();
		Self {
			data,
			width: image.width,
			height: image.height,
			base64_string: None,
		}
	}
}

impl From<ImageFrameTable<Color>> for ImageFrameTable<SRGBA8> {
	fn from(image_frame_table: ImageFrameTable<Color>) -> Self {
		let mut result_table = ImageFrameTable::<SRGBA8>::empty();

		for image_frame_instance in image_frame_table.instance_iter() {
			result_table.push(Instance {
				instance: image_frame_instance.instance.into(),
				transform: image_frame_instance.transform,
				alpha_blending: image_frame_instance.alpha_blending,
				source_node_id: image_frame_instance.source_node_id,
			});
		}

		result_table
	}
}

impl From<Image<SRGBA8>> for Image<Color> {
	fn from(image: Image<SRGBA8>) -> Self {
		let data = image.data.into_iter().map(|x| x.into()).collect();
		Self {
			data,
			width: image.width,
			height: image.height,
			base64_string: None,
		}
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
		println!("{}", serialized);
		let deserialized: Image<Color> = serde_json::from_str(&serialized).unwrap();
		println!("{:?}", deserialized);

		assert_eq!(image, deserialized);
	}
}
