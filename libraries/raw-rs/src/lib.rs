pub mod decoder;
pub mod demosiacing;
pub mod metadata;
pub mod preprocessing;
pub mod tiff;

use crate::preprocessing::camera_data::camera_to_xyz;

use tag_derive::Tag;
use tiff::file::TiffRead;
use tiff::tags::{Compression, ImageLength, ImageWidth, StripByteCounts, SubIfd, Tag};
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
	pub maximum: u16,
	pub black: SubtractBlack,
	pub camera_to_xyz: Option<[f64; 12]>,
}

pub struct Image<T> {
	pub data: Vec<T>,
	pub width: usize,
	pub height: usize,
	pub channels: u8,
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

	raw_image.camera_to_xyz = camera_to_xyz(&camera_model);

	Ok(raw_image)
}

pub fn process_8bit(raw_image: RawImage) -> Image<u8> {
	let raw_image = crate::preprocessing::subtract_black::subtract_black(raw_image);
	let raw_image = crate::preprocessing::raw_to_image::raw_to_image(raw_image);
	let raw_image = crate::preprocessing::scale_colors::scale_colors(raw_image);
	let image = crate::demosiacing::linear_demosiacing::linear_demosiac(raw_image);

	Image {
		channels: image.channels,
		data: image.data.iter().map(|x| (x >> 8) as u8).collect(),
		width: image.width,
		height: image.height,
	}
}

pub fn process_16bit(_image: RawImage) -> Image<u16> {
	todo!()
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
