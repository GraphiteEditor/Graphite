pub mod decoder;
pub mod preprocessing;
pub mod tiff;

use tag_derive::Tag;
use tiff::file::TiffRead;
use tiff::tags::{Compression, ImageLength, ImageWidth, Model, StripByteCounts, SubIfd, Tag};
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
	pub cam_to_xyz: Option<[f64; 12]>,
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

	println!("{}", ifd);

	// TODO: This is only for the tests to pass for now. Replace this with the correct implementation when the decoder is complete.
	let make = ifd.get_value::<Make, _>(&mut file)?;
	let model = ifd.get_value::<Model, _>(&mut file)?;

	let mut raw_image = if model == "DSLR-A100" {
		decoder::arw1::decode_a100(ifd, &mut file)
	} else {
		let sub_ifd = ifd.get_value::<SubIfd, _>(&mut file)?;
		let arw_ifd = sub_ifd.get_value::<ArwIfd, _>(&mut file)?;

		println!("{}", subifd);

		if arw_ifd.compression == 1 {
<<<<<<< Updated upstream
			Ok(decoder::uncompressed::decode(sub_ifd, &mut file))
		} else if arw_ifd.strip_byte_counts[0] == arw_ifd.image_width * arw_ifd.image_height {
			Ok(decoder::arw2::decode(sub_ifd, &mut file))
=======
			decoder::uncompressed::decode(subifd, &mut file)
		} else if arw_ifd.strip_byte_counts[0] == arw_ifd.image_width * arw_ifd.image_height {
			decoder::arw2::decode(subifd, &mut file)
>>>>>>> Stashed changes
		} else {
			// TODO: implement for arw 1.
			todo!()
		}
	};

	// raw_image.cam_to_xyz = get_cam_to_xyz(&make, &model);
	//
	// let raw_image = crate::preprocessing::subtract_black::subtract_black(raw_image);
	// let raw_image = crate::preprocessing::scale_colors::scale_colors(raw_image);

	Ok(raw_image)
}

pub fn process_8bit(_image: RawImage) -> Image<u8> {
	todo!()
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
