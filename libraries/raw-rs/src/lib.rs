pub mod decoder;
pub mod tiff;

use std::io::{Read, Seek};
use thiserror::Error;
use tiff::file::TiffRead;
use tiff::tags::SONY_SUBIFD;
use tiff::{Ifd, TiffError};

pub struct RawImage {
	pub data: Vec<u16>,
	pub width: usize,
	pub height: usize,
}

pub struct Image<T> {
	pub data: Vec<T>,
	pub width: usize,
	pub height: usize,
	pub channels: u8,
}

pub fn decode<R: Read + Seek>(reader: &mut R) -> Result<RawImage, DecoderError> {
	let mut file = TiffRead::new(reader)?;
	let ifd = Ifd::new_first_ifd(&mut file)?;

	// TODO: This is only for the tests to pass for now. Replace this with the correct implementation when the decoder is complete.
	let offset = ifd.get(SONY_SUBIFD, &mut file)?;
	let subifd = Ifd::new_from_offset(&mut file, offset)?;

	Ok(decoder::uncompressed::decode(subifd, &mut file))
}

pub fn process_8bit(image: RawImage) -> Image<u8> {
	todo!()
}

pub fn process_16bit(image: RawImage) -> Image<u16> {
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
