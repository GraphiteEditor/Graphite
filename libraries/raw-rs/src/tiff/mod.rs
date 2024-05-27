pub mod file;
pub mod tags;
mod types;
pub mod values;

use file::TiffRead;
use num_enum::{FromPrimitive, IntoPrimitive, TryFromPrimitive};
use std::io::{Read, Seek};
use thiserror::Error;

use tags::Tag;
use types::TagType;

#[derive(Copy, Clone, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u16)]
pub enum TagId {
	ImageWidth = 0x100,
	ImageLength = 0x101,
	BitsPerSample = 0x102,
	Compression = 0x103,
	PhotometricInterpretation = 0x104,
	StripOffsets = 0x111,
	SamplesPerPixel = 0x115,
	RowsPerStrip = 0x116,
	StripByteCounts = 0x117,
	SonySubIfd = 0x14a,
	JpegOffset = 0x201,
	JpegLength = 0x202,
	CfaPatternDim = 0x828d,
	CfaPattern = 0x828e,

	#[num_enum(catch_all)]
	Unknown(u16),
}

#[repr(u16)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum IfdTagType {
	Ascii = 2,
	Byte = 1,
	Short = 3,
	Long = 4,
	Rational = 5,
	SByte = 6,
	SShort = 8,
	SLong = 9,
	SRational = 10,
	Float = 11,
	Double = 12,
	Undefined = 7,
}

#[derive(Copy, Clone, Debug)]
pub struct IfdEntry {
	tag: TagId,
	type_: u16,
	count: u32,
	value: u32,
}

#[derive(Clone, Debug)]
pub struct Ifd {
	current_ifd_offset: u32,
	ifd_entries: Vec<IfdEntry>,
	next_ifd_offset: u32,
}

impl Ifd {
	pub fn new_first_ifd<R: Read + Seek>(file: &mut TiffRead<R>) -> std::io::Result<Self> {
		file.seek_from_start(4)?;
		let current_ifd_offset = file.read_u32()?;
		Ifd::new_from_offset(file, current_ifd_offset)
	}

	pub fn new_from_offset<R: Read + Seek>(file: &mut TiffRead<R>, offset: u32) -> std::io::Result<Self> {
		if offset == 0 {
			return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Ifd at offset zero does not exist"));
		}

		file.seek_from_start(offset)?;
		let num = file.read_u16()?;

		let mut ifd_entries = Vec::with_capacity(num.into());
		for _ in 0..num {
			let tag = file.read_u16()?.into();
			let type_ = file.read_u16()?;
			let count = file.read_u32()?;
			let value = file.read_u32()?;

			ifd_entries.push(IfdEntry { tag, type_, count, value });
		}

		let next_ifd_offset = file.read_u32()?;

		Ok(Ifd {
			current_ifd_offset: offset,
			ifd_entries,
			next_ifd_offset,
		})
	}

	fn next_ifd<R: Read + Seek>(&self, file: &mut TiffRead<R>) -> std::io::Result<Self> {
		Ifd::new_from_offset(file, self.next_ifd_offset)
	}

	pub fn ifd_entries(&self) -> &[IfdEntry] {
		&self.ifd_entries
	}

	pub fn iter(&self) -> impl Iterator<Item = &IfdEntry> {
		self.ifd_entries.iter()
	}

	pub fn get<T: TagType, R: Read + Seek>(&self, tag: Tag<T>, file: &mut TiffRead<R>) -> Result<T::Output, TiffError> {
		let tag_id = tag.id();
		let index: u32 = self.iter().position(|x| x.tag == tag_id).ok_or(TiffError::InvalidTag)?.try_into()?;

		file.seek_from_start(self.current_ifd_offset + 2 + 12 * index + 2)?;
		T::read(file)
	}
}

#[derive(Error, Debug)]
pub enum TiffError {
	#[error("The value was invalid")]
	InvalidValue,
	#[error("The type was invalid")]
	InvalidType,
	#[error("The count was invalid")]
	InvalidCount,
	#[error("The tag was invalid")]
	InvalidTag,
	#[error("An error occurred when converting integer from one type to another")]
	ConversionError(#[from] std::num::TryFromIntError),
	#[error("An IO Error ocurred")]
	IoError(#[from] std::io::Error),
}
