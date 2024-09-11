pub mod decoder;
pub mod demosaicing;
pub mod metadata;
pub mod postprocessing;
pub mod preprocessing;
pub mod tiff;

use crate::metadata::identify::CameraModel;

use tag_derive::Tag;
use tiff::file::TiffRead;
use tiff::tags::{Compression, ImageLength, ImageWidth, Orientation, StripByteCounts, SubIfd, Tag};
use tiff::values::Transform;
use tiff::{Ifd, TiffError};

use std::io::{Read, Seek};
use thiserror::Error;

pub enum SubtractBlack {
	None,
	Value(u16),
	CfaGrid([u16; 4]),
}

pub struct RawImage {
	pub data: Vec<u16>,
	pub width: usize,
	pub height: usize,
	pub cfa_pattern: [u8; 4],
	pub transform: Transform,
	pub maximum: u16,
	pub black: SubtractBlack,
	pub camera_model: Option<CameraModel>,
	pub camera_white_balance_multiplier: Option<[f64; 4]>,
	pub white_balance_multiplier: Option<[f64; 4]>,
	pub camera_to_rgb: Option<[[f64; 3]; 3]>,
	pub rgb_to_camera: Option<[[f64; 3]; 3]>,
}

pub struct Image<T> {
	pub data: Vec<T>,
	pub width: usize,
	pub height: usize,
	/// We can assume this will be 3 for all non-obscure, modern cameras.
	/// See <https://github.com/GraphiteEditor/Graphite/pull/1923#discussion_r1725070342> for more information.
	pub channels: u8,
	pub transform: Transform,
	pub rgb_to_camera: Option<[[f64; 3]; 3]>,
	pub(crate) histogram: Option<[[usize; 0x2000]; 3]>,
}

#[allow(dead_code)]
#[derive(Tag)]
struct ArwIfd {
	image_width: ImageWidth,
	image_height: ImageLength,
	compression: Compression,
	strip_byte_counts: StripByteCounts,
}

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

	Ok(raw_image)
}

pub fn process_8bit(raw_image: RawImage) -> Image<u8> {
	let image = process_16bit(raw_image);

	Image {
		channels: image.channels,
		data: image.data.iter().map(|x| (x >> 8) as u8).collect(),
		width: image.width,
		height: image.height,
		transform: image.transform,
		rgb_to_camera: image.rgb_to_camera,
		histogram: image.histogram,
	}
}

pub fn process_16bit(raw_image: RawImage) -> Image<u16> {
	let mut raw_image = crate::preprocessing::camera_data::calculate_conversion_matrices(raw_image);
	// let raw_image = crate::preprocessing::subtract_black::subtract_black(raw_image);
	// let raw_image = crate::preprocessing::scale_colors::scale_colors(raw_image);
	let subtract_black = raw_image.subtract_black_fn();
	let scale_colors = raw_image.scale_colors_fn();

	for (index, value) in raw_image.data.iter_mut().enumerate() {
		let row = index / raw_image.width;
		let column = index % raw_image.width;
		*value = scale_colors(subtract_black(*value, row, column), row, column);
	}

	let image = crate::demosaicing::linear_demosaicing::linear_demosaic(raw_image);
	let image = crate::postprocessing::convert_to_rgb::convert_to_rgb(image);
	let image = crate::postprocessing::transform::transform(image);
	crate::postprocessing::gamma_correction::gamma_correction(image)
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
