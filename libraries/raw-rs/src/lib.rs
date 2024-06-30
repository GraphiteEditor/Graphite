pub mod decoder;
pub mod tiff;
pub mod preprocessing;

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
	pub black: SubtractBlack,
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

	// TODO: This is only for the tests to pass for now. Replace this with the correct implementation when the decoder is complete.
	let model = ifd.get_value::<Model, _>(&mut file)?;

	if model == "DSLR-A100" {
		Ok(decoder::arw1::decode_a100(ifd, &mut file))
	} else {
		let sub_ifd = ifd.get_value::<SubIfd, _>(&mut file)?;
		let arw_ifd = sub_ifd.get_value::<ArwIfd, _>(&mut file)?;

		if arw_ifd.compression == 1 {
			Ok(decoder::uncompressed::decode(sub_ifd, &mut file))
		} else if arw_ifd.strip_byte_counts[0] == arw_ifd.image_width * arw_ifd.image_height {
			Ok(decoder::arw2::decode(sub_ifd, &mut file))
		} else {
			// TODO: implement for arw 1.
			todo!()
		}
	}
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
