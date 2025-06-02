pub mod decoder;
pub mod demosaicing;
pub mod metadata;
pub mod postprocessing;
pub mod preprocessing;
pub mod processing;
pub mod tiff;

use crate::metadata::identify::CameraModel;
use processing::{Pixel, PixelTransform, RawPixel, RawPixelTransform};
use rawkit_proc_macros::Tag;
use std::io::{Read, Seek};
use thiserror::Error;
use tiff::file::TiffRead;
use tiff::tags::{Compression, ImageLength, ImageWidth, Orientation, StripByteCounts, SubIfd, Tag};
use tiff::values::Transform;
use tiff::{Ifd, TiffError};

pub(crate) const CHANNELS_IN_RGB: usize = 3;
pub(crate) type Histogram = [[usize; 0x2000]; CHANNELS_IN_RGB];

/// The amount of black level to be subtracted from Raw Image.
pub enum SubtractBlack {
	/// Don't subtract any value.
	None,

	/// Subtract a singular value for all pixels in Bayer CFA Grid.
	Value(u16),

	/// Subtract the appropriate value for pixels in Bayer CFA Grid.
	CfaGrid([u16; 4]),
}

/// Represents a Raw Image along with its metadata.
pub struct RawImage {
	/// Raw pixel data stored in linear fashion.
	pub data: Vec<u16>,

	/// Width of the raw image.
	pub width: usize,

	/// Height of the raw image.
	pub height: usize,

	/// Bayer CFA pattern used to arrange pixels in [`RawImage::data`].
	///
	/// It encodes Red, Blue and Green as 0, 1, and 2 respectively.
	pub cfa_pattern: [u8; 4],

	/// Transformation to be applied to negate the orientation of camera.
	pub transform: Transform,

	/// The maximum possible value of pixel that the camera sensor could give.
	pub maximum: u16,

	/// The minimum possible value of pixel that the camera sensor could give.
	///
	/// Used to subtract the black level from the raw image.
	pub black: SubtractBlack,

	/// Information regarding the company and model of the camera.
	pub camera_model: Option<CameraModel>,

	/// White balance specified in the metadata of the raw file.
	///
	/// It represents the 4 values of CFA Grid which follows the same pattern as [`RawImage::cfa_pattern`].
	pub camera_white_balance: Option<[f64; 4]>,

	/// White balance of the raw image.
	///
	/// It is the same as [`RawImage::camera_white_balance`] if the raw file contains the metadata.
	/// Otherwise it falls back to calculating the white balance from the color space conversion matrix.
	///
	/// It represents the 4 values of CFA Grid which follows the same pattern as [`RawImage::cfa_pattern`].
	pub white_balance: Option<[f64; 4]>,

	/// Color space conversion matrix to convert from camera's color space to sRGB.
	pub camera_to_rgb: Option<[[f64; 3]; 3]>,
}

/// Represents the final RGB Image.
pub struct Image<T> {
	/// Pixel data stored in a linear fashion.
	pub data: Vec<T>,

	/// Width of the image.
	pub width: usize,

	/// Height of the image.
	pub height: usize,

	/// The number of color channels in the image.
	///
	/// We can assume this will be 3 for all non-obscure, modern cameras.
	/// See <https://github.com/GraphiteEditor/Graphite/pull/1923#discussion_r1725070342> for more information.
	pub channels: u8,

	/// The transformation required to orient the image correctly.
	///
	/// This will be [`Transform::Horizontal`] after the transform step is applied.
	pub transform: Transform,
}

#[allow(dead_code)]
#[derive(Tag)]
struct ArwIfd {
	image_width: ImageWidth,
	image_height: ImageLength,
	compression: Compression,
	strip_byte_counts: StripByteCounts,
}

impl RawImage {
	/// Create a [`RawImage`] from an input stream.
	///
	/// Decodes the contents of `reader` and extracts raw pixel data and metadata.
	pub fn decode<R: Read + Seek>(reader: &mut R) -> Result<RawImage, DecoderError> {
		let mut file = TiffRead::new(reader)?;
		let ifd = Ifd::new_first_ifd(&mut file)?;

		let camera_model = metadata::identify::identify_camera_model(&ifd, &mut file).unwrap();
		let transform = ifd.get_value::<Orientation, _>(&mut file)?;

		let mut raw_image = if camera_model.model == "DSLR-A100" {
			decoder::arw1::decode_a100(ifd, &mut file)
		} else {
			let sub_ifd = ifd.get_value::<SubIfd, _>(&mut file)?;
			let arw_ifd = sub_ifd.get_value::<ArwIfd, _>(&mut file)?;

			if arw_ifd.compression == 1 {
				decoder::uncompressed::decode(sub_ifd, &mut file)
			} else if arw_ifd.strip_byte_counts[0] == arw_ifd.image_width * arw_ifd.image_height {
				decoder::arw2::decode(sub_ifd, &mut file)
			} else {
				// TODO: implement for arw 1.
				todo!()
			}
		};

		raw_image.camera_model = Some(camera_model);
		raw_image.transform = transform;

		raw_image.calculate_conversion_matrices();

		Ok(raw_image)
	}

	/// Converts the [`RawImage`] to an [`Image`] with 8 bit resolution for each channel.
	///
	/// Applies all the processing steps to finally get RGB pixel data.
	pub fn process_8bit(self) -> Image<u8> {
		let image = self.process_16bit();

		Image {
			channels: image.channels,
			data: image.data.iter().map(|x| (x >> 8) as u8).collect(),
			width: image.width,
			height: image.height,
			transform: image.transform,
		}
	}

	/// Converts the [`RawImage`] to an [`Image`] with 16 bit resolution for each channel.
	///
	/// Applies all the processing steps to finally get RGB pixel data.
	pub fn process_16bit(self) -> Image<u16> {
		let subtract_black = self.subtract_black_fn();
		let scale_white_balance = self.scale_white_balance_fn();
		let scale_to_16bit = self.scale_to_16bit_fn();
		let raw_image = self.apply((subtract_black, scale_white_balance, scale_to_16bit));

		let convert_to_rgb = raw_image.convert_to_rgb_fn();
		let mut record_histogram = raw_image.record_histogram_fn();
		let image = raw_image.demosaic_and_apply((convert_to_rgb, &mut record_histogram));

		let gamma_correction = image.gamma_correction_fn(&record_histogram.histogram);
		if image.transform == Transform::Horizontal {
			image.apply(gamma_correction)
		} else {
			image.transform_and_apply(gamma_correction)
		}
	}
}

impl RawImage {
	pub fn apply(mut self, mut transform: impl RawPixelTransform) -> RawImage {
		for (index, value) in self.data.iter_mut().enumerate() {
			let pixel = RawPixel {
				value: *value,
				row: index / self.width,
				column: index % self.width,
			};
			*value = transform.apply(pixel);
		}

		self
	}

	pub fn demosaic_and_apply(self, mut transform: impl PixelTransform) -> Image<u16> {
		let mut image = vec![0; self.width * self.height * 3];
		for Pixel { values, row, column } in self.linear_demosaic_iter().map(|mut pixel| {
			pixel.values = transform.apply(pixel);
			pixel
		}) {
			let pixel_index = row * self.width + column;
			image[3 * pixel_index..3 * (pixel_index + 1)].copy_from_slice(&values);
		}

		Image {
			channels: 3,
			data: image,
			width: self.width,
			height: self.height,
			transform: self.transform,
		}
	}
}

impl Image<u16> {
	pub fn apply(mut self, mut transform: impl PixelTransform) -> Image<u16> {
		for (index, values) in self.data.chunks_exact_mut(3).enumerate() {
			let pixel = Pixel {
				values: values.try_into().unwrap(),
				row: index / self.width,
				column: index % self.width,
			};
			values.copy_from_slice(&transform.apply(pixel));
		}

		self
	}

	pub fn transform_and_apply(self, mut transform: impl PixelTransform) -> Image<u16> {
		let mut image = vec![0; self.width * self.height * 3];
		let (width, height, iter) = self.transform_iter();
		for Pixel { values, row, column } in iter.map(|mut pixel| {
			pixel.values = transform.apply(pixel);
			pixel
		}) {
			let pixel_index = row * width + column;
			image[3 * pixel_index..3 * (pixel_index + 1)].copy_from_slice(&values);
		}

		Image {
			channels: 3,
			data: image,
			width,
			height,
			transform: Transform::Horizontal,
		}
	}
}

#[derive(Error, Debug)]
pub enum DecoderError {
	#[error("An error occurred when trying to parse the TIFF format")]
	TiffError(#[from] TiffError),
	#[error("An error occurred when converting integer from one type to another")]
	ConversionError(#[from] std::num::TryFromIntError),
	#[error("An IO Error ocurred")]
	IoError(#[from] std::io::Error),
}
