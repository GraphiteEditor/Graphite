use super::Color;
use crate::AlphaBlending;
use crate::color::float_to_srgb_u8;
use crate::raster_types::Raster;
use crate::table::{Table, TableRow};
use crate::vector::Vector;
use core::hash::{Hash, Hasher};
use dyn_any::{DynAny, StaticType};
use glam::{DAffine2, DVec2};
use std::vec::Vec;

mod base64_serde {
	//! Basic wrapper for [`serde`] to perform [`base64`] encoding

	use super::super::Pixel;
	use base64::Engine;
	use serde::{Deserialize, Deserializer, Serialize, Serializer};

	pub fn as_base64<S: Serializer, P: Pixel>(key: &[P], serializer: S) -> Result<S::Ok, S::Error> {
		let u8_data = bytemuck::cast_slice(key);
		let string = base64::engine::general_purpose::STANDARD.encode(u8_data);
		(key.len() as u64, string).serialize(serializer)
	}

	pub fn from_base64<'a, D: Deserializer<'a>, P: Pixel>(deserializer: D) -> Result<Vec<P>, D::Error> {
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

#[derive(Clone, PartialEq, Default, specta::Type, serde::Serialize, serde::Deserialize)]
pub struct Image<P: Pixel> {
	pub width: u32,
	pub height: u32,
	#[serde(serialize_with = "base64_serde::as_base64", deserialize_with = "base64_serde::from_base64")]
	pub data: Vec<P>,
	/// Optional: Stores a base64 string representation of the image which can be used to speed up the conversion
	/// to an svg string. This is used as a cache in order to not have to encode the data on every graph evaluation.
	#[serde(skip)]
	pub base64_string: Option<String>,
	// TODO: Add an `origin` field to store where in the local space the image is anchored.
	// TODO: Currently it is always anchored at the top left corner at (0, 0). The bottom right corner of the new origin field would correspond to (1, 1).
}

#[derive(Debug, Clone, dyn_any::DynAny, Default, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct TransformImage(pub DAffine2);

impl Hash for TransformImage {
	fn hash<H: std::hash::Hasher>(&self, _: &mut H) {}
}

impl<P: Pixel + Debug> Debug for Image<P> {
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
	type IntoIter = std::vec::IntoIter<P>;
	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
	}
}

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_image_frame<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Table<Raster<CPU>>, D::Error> {
	use serde::Deserialize;

	#[derive(Clone, Debug, Hash, PartialEq, DynAny)]
	enum RasterFrame {
		ImageFrame(Table<Image<Color>>),
	}
	impl<'de> serde::Deserialize<'de> for RasterFrame {
		fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
			Ok(RasterFrame::ImageFrame(Table::new_from_element(Image::deserialize(deserializer)?)))
		}
	}
	impl serde::Serialize for RasterFrame {
		fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
			match self {
				RasterFrame::ImageFrame(table) => table.serialize(serializer),
			}
		}
	}

	#[derive(Clone, Debug, Hash, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
	pub enum GraphicElement {
		GraphicGroup(Table<GraphicElement>),
		VectorData(Table<Vector>),
		RasterFrame(RasterFrame),
	}

	#[derive(Clone, Default, Debug, PartialEq, specta::Type, serde::Serialize, serde::Deserialize)]
	pub struct ImageFrame<P: Pixel> {
		pub image: Image<P>,
	}
	impl From<ImageFrame<Color>> for GraphicElement {
		fn from(image_frame: ImageFrame<Color>) -> Self {
			GraphicElement::RasterFrame(RasterFrame::ImageFrame(Table::new_from_element(image_frame.image)))
		}
	}
	impl From<GraphicElement> for ImageFrame<Color> {
		fn from(element: GraphicElement) -> Self {
			match element {
				GraphicElement::RasterFrame(RasterFrame::ImageFrame(image)) => Self {
					image: image.iter().next().unwrap().element.clone(),
				},
				_ => panic!("Expected Image, found {element:?}"),
			}
		}
	}

	unsafe impl<P> StaticType for ImageFrame<P>
	where
		P: dyn_any::StaticTypeSized + Pixel,
		P::Static: Pixel,
	{
		type Static = ImageFrame<P::Static>;
	}

	#[derive(Clone, Default, Debug, PartialEq, specta::Type, serde::Serialize, serde::Deserialize)]
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
		OlderImageFrameTable(OlderTable<ImageFrame<Color>>),
		OldImageFrameTable(OldTable<ImageFrame<Color>>),
		OldImageTable(OldTable<Image<Color>>),
		OldRasterTable(OldTable<Raster<CPU>>),
		ImageFrameTable(Table<ImageFrame<Color>>),
		ImageTable(Table<Image<Color>>),
		RasterTable(Table<Raster<CPU>>),
	}

	#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
	pub struct OldTable<T> {
		#[serde(alias = "instances", alias = "instance")]
		element: Vec<T>,
		transform: Vec<DAffine2>,
		alpha_blending: Vec<AlphaBlending>,
	}

	#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
	pub struct OlderTable<T> {
		id: Vec<u64>,
		#[serde(alias = "instances", alias = "instance")]
		element: Vec<T>,
	}

	fn from_image_table(table: Table<Image<Color>>) -> Table<Raster<CPU>> {
		Table::new_from_element(Raster::new_cpu(table.iter().next().unwrap().element.clone()))
	}

	fn old_table_to_new_table<T>(old_table: OldTable<T>) -> Table<T> {
		old_table
			.element
			.into_iter()
			.zip(old_table.transform.into_iter().zip(old_table.alpha_blending))
			.map(|(element, (transform, alpha_blending))| TableRow {
				element,
				transform,
				alpha_blending,
				source_node_id: None,
			})
			.collect()
	}

	fn older_table_to_new_table<T>(old_table: OlderTable<T>) -> Table<T> {
		old_table
			.element
			.into_iter()
			.map(|element| TableRow {
				element,
				transform: DAffine2::IDENTITY,
				alpha_blending: AlphaBlending::default(),
				source_node_id: None,
			})
			.collect()
	}

	fn from_image_frame_table(image_frame: Table<ImageFrame<Color>>) -> Table<Raster<CPU>> {
		Table::new_from_element(Raster::new_cpu(
			image_frame
				.iter()
				.next()
				.unwrap_or(Table::new_from_element(ImageFrame::default()).iter().next().unwrap())
				.element
				.image
				.clone(),
		))
	}

	Ok(match FormatVersions::deserialize(deserializer)? {
		FormatVersions::Image(image) => Table::new_from_element(Raster::new_cpu(image)),
		FormatVersions::OldImageFrame(OldImageFrame { image, transform, alpha_blending }) => {
			let mut image_frame_table = Table::new_from_element(Raster::new_cpu(image));
			*image_frame_table.iter_mut().next().unwrap().transform = transform;
			*image_frame_table.iter_mut().next().unwrap().alpha_blending = alpha_blending;
			image_frame_table
		}
		FormatVersions::OlderImageFrameTable(old_table) => from_image_frame_table(older_table_to_new_table(old_table)),
		FormatVersions::OldImageFrameTable(old_table) => from_image_frame_table(old_table_to_new_table(old_table)),
		FormatVersions::OldImageTable(old_table) => from_image_table(old_table_to_new_table(old_table)),
		FormatVersions::OldRasterTable(old_table) => old_table_to_new_table(old_table),
		FormatVersions::ImageFrameTable(image_frame) => from_image_frame_table(image_frame),
		FormatVersions::ImageTable(table) => from_image_table(table),
		FormatVersions::RasterTable(table) => table,
	})
}

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_image_frame_row<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<TableRow<Raster<CPU>>, D::Error> {
	use serde::Deserialize;

	#[derive(Clone, Debug, Hash, PartialEq, DynAny)]
	enum RasterFrame {
		/// A CPU-based bitmap image with a finite position and extent, equivalent to the SVG <image> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/image
		ImageFrame(Table<Image<Color>>),
	}
	impl<'de> serde::Deserialize<'de> for RasterFrame {
		fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
			Ok(RasterFrame::ImageFrame(Table::new_from_element(Image::deserialize(deserializer)?)))
		}
	}
	impl serde::Serialize for RasterFrame {
		fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
			match self {
				RasterFrame::ImageFrame(table) => table.serialize(serializer),
			}
		}
	}

	#[derive(Clone, Debug, Hash, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
	pub enum GraphicElement {
		/// Equivalent to the SVG <g> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/g
		GraphicGroup(Table<GraphicElement>),
		/// A vector shape, equivalent to the SVG <path> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/path
		VectorData(Table<Vector>),
		RasterFrame(RasterFrame),
	}

	#[derive(Clone, Default, Debug, PartialEq, specta::Type, serde::Serialize, serde::Deserialize)]
	pub struct ImageFrame<P: Pixel> {
		pub image: Image<P>,
	}
	impl From<ImageFrame<Color>> for GraphicElement {
		fn from(image_frame: ImageFrame<Color>) -> Self {
			GraphicElement::RasterFrame(RasterFrame::ImageFrame(Table::new_from_element(image_frame.image)))
		}
	}
	impl From<GraphicElement> for ImageFrame<Color> {
		fn from(element: GraphicElement) -> Self {
			match element {
				GraphicElement::RasterFrame(RasterFrame::ImageFrame(image)) => Self {
					image: image.iter().next().unwrap().element.clone(),
				},
				_ => panic!("Expected Image, found {element:?}"),
			}
		}
	}

	unsafe impl<P> StaticType for ImageFrame<P>
	where
		P: dyn_any::StaticTypeSized + Pixel,
		P::Static: Pixel,
	{
		type Static = ImageFrame<P::Static>;
	}

	#[derive(Clone, Default, Debug, PartialEq, specta::Type, serde::Serialize, serde::Deserialize)]
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
		ImageFrameTable(Table<ImageFrame<Color>>),
		RasterTable(Table<Raster<CPU>>),
		RasterTableRow(TableRow<Raster<CPU>>),
	}

	Ok(match FormatVersions::deserialize(deserializer)? {
		FormatVersions::Image(image) => TableRow {
			element: Raster::new_cpu(image),
			..Default::default()
		},
		FormatVersions::OldImageFrame(image_frame_with_transform_and_blending) => TableRow {
			element: Raster::new_cpu(image_frame_with_transform_and_blending.image),
			transform: image_frame_with_transform_and_blending.transform,
			alpha_blending: image_frame_with_transform_and_blending.alpha_blending,
			source_node_id: None,
		},
		FormatVersions::ImageFrameTable(image_frame) => TableRow {
			element: Raster::new_cpu(image_frame.iter().next().unwrap().element.image.clone()),
			..Default::default()
		},
		FormatVersions::RasterTable(image_frame_table) => image_frame_table.into_iter().next().unwrap_or_default(),
		FormatVersions::RasterTableRow(image_table_row) => image_table_row,
	})
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
		println!("{serialized}");
		let deserialized: Image<Color> = serde_json::from_str(&serialized).unwrap();
		println!("{deserialized:?}");

		assert_eq!(image, deserialized);
	}
}
