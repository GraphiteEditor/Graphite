pub mod file;
pub mod tags;
mod types;
pub mod values;

use file::TiffRead;
use num_enum::{FromPrimitive, IntoPrimitive};
use std::fmt::Display;
use std::io::{Read, Seek};
use tags::Tag;
use thiserror::Error;

#[derive(Copy, Clone, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u16)]
pub enum TagId {
	ImageWidth = 0x100,
	ImageLength = 0x101,
	BitsPerSample = 0x102,
	Compression = 0x103,
	PhotometricInterpretation = 0x104,
	Make = 0x10f,
	Model = 0x110,
	StripOffsets = 0x111,
	Orientation = 0x112,
	SamplesPerPixel = 0x115,
	RowsPerStrip = 0x116,
	StripByteCounts = 0x117,
	SubIfd = 0x14a,
	JpegOffset = 0x201,
	JpegLength = 0x202,
	SonyToneCurve = 0x7010,
	BlackLevel = 0x7310,
	WhiteBalanceRggbLevels = 0x7313,
	CfaPatternDim = 0x828d,
	CfaPattern = 0x828e,
	ColorMatrix1 = 0xc621,
	ColorMatrix2 = 0xc622,

	#[num_enum(catch_all)]
	Unknown(u16),
}

#[repr(u16)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
pub enum IfdTagType {
	Byte = 1,
	Ascii = 2,
	Short = 3,
	Long = 4,
	Rational = 5,
	SByte = 6,
	Undefined = 7,
	SShort = 8,
	SLong = 9,
	SRational = 10,
	Float = 11,
	Double = 12,

	#[num_enum(catch_all)]
	Unknown(u16),
}

#[derive(Copy, Clone, Debug)]
pub struct IfdEntry {
	tag: TagId,
	the_type: IfdTagType,
	count: u32,
	value: u32,
}

#[derive(Clone, Debug)]
pub struct Ifd {
	current_ifd_offset: u32,
	ifd_entries: Vec<IfdEntry>,
	next_ifd_offset: Option<u32>,
}

impl Ifd {
	pub fn new_first_ifd<R: Read + Seek>(file: &mut TiffRead<R>) -> Result<Self, TiffError> {
		file.seek_from_start(4)?;
		let current_ifd_offset = file.read_u32()?;
		Ifd::new_from_offset(file, current_ifd_offset)
	}

	pub fn new_from_offset<R: Read + Seek>(file: &mut TiffRead<R>, offset: u32) -> Result<Self, TiffError> {
		if offset == 0 {
			return Err(TiffError::InvalidOffset);
		}

		file.seek_from_start(offset)?;
		let num = file.read_u16()?;

		let mut ifd_entries = Vec::with_capacity(num.into());
		for _ in 0..num {
			let tag = file.read_u16()?.into();
			let the_type = file.read_u16()?.into();
			let count = file.read_u32()?;
			let value = file.read_u32()?;

			ifd_entries.push(IfdEntry { tag, the_type, count, value });
		}

		let next_ifd_offset = file.read_u32()?;
		let next_ifd_offset = if next_ifd_offset == 0 { None } else { Some(next_ifd_offset) };

		Ok(Ifd {
			current_ifd_offset: offset,
			ifd_entries,
			next_ifd_offset,
		})
	}

	fn _next_ifd<R: Read + Seek>(&self, file: &mut TiffRead<R>) -> Result<Self, TiffError> {
		Ifd::new_from_offset(file, self.next_ifd_offset.unwrap_or(0))
	}

	pub fn ifd_entries(&self) -> &[IfdEntry] {
		&self.ifd_entries
	}

	pub fn iter(&self) -> impl Iterator<Item = &IfdEntry> {
		self.ifd_entries.iter()
	}

	pub fn get_value<T: Tag, R: Read + Seek>(&self, file: &mut TiffRead<R>) -> Result<T::Output, TiffError> {
		T::get(self, file)
	}
}

impl Display for Ifd {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str("IFD offset: ")?;
		self.current_ifd_offset.fmt(f)?;
		f.write_str("\n")?;

		for ifd_entry in self.ifd_entries() {
			f.write_fmt(format_args!(
				"|- Tag: {:x?}, Type: {:?}, Count: {}, Value: {:x}\n",
				ifd_entry.tag, ifd_entry.the_type, ifd_entry.count, ifd_entry.value
			))?;
		}

		f.write_str("Next IFD offset: ")?;
		if let Some(offset) = self.next_ifd_offset {
			offset.fmt(f)?;
		} else {
			f.write_str("None")?;
		}
		f.write_str("\n")?;

		Ok(())
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
	#[error("The tag was missing")]
	MissingTag,
	#[error("The offset was invalid or zero")]
	InvalidOffset,
	#[error("An error occurred when converting integer from one type to another")]
	ConversionError(#[from] std::num::TryFromIntError),
	#[error("An IO Error ocurred")]
	IoError(#[from] std::io::Error),
}
