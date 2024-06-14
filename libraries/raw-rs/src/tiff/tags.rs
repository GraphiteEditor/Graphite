use super::types::{Array, ConstArray, TagType, TypeByte, TypeIfd, TypeLong, TypeNumber, TypeShort, TypeSonyToneCurve};
use super::{Ifd, TagId, TiffError, TiffRead};

use std::io::{Read, Seek};

pub trait SimpleTag {
	const ID: TagId;
	type Type: TagType;
	const NAME: &'static str;
}

pub struct ImageWidth;
pub struct ImageLength;
pub struct BitsPerSample;
pub struct Compression;
pub struct PhotometricInterpretation;
pub struct StripOffsets;
pub struct SamplesPerPixel;
pub struct RowsPerStrip;
pub struct StripByteCounts;
pub struct SubIfd;
pub struct JpegOffset;
pub struct JpegLength;
pub struct CfaPatternDim;
pub struct CfaPattern;
pub struct SonyDataOffset;
pub struct SonyToneCurve;

impl SimpleTag for ImageWidth {
	const ID: TagId = TagId::ImageWidth;
	type Type = TypeNumber;
	const NAME: &'static str = "Image Width";
}

impl SimpleTag for ImageLength {
	const ID: TagId = TagId::ImageLength;
	type Type = TypeNumber;
	const NAME: &'static str = "Image Length";
}

impl SimpleTag for BitsPerSample {
	const ID: TagId = TagId::BitsPerSample;
	type Type = TypeShort;
	const NAME: &'static str = "Bits per Sample";
}

impl SimpleTag for Compression {
	const ID: TagId = TagId::Compression;
	type Type = TypeShort;
	const NAME: &'static str = "Compression";
}

impl SimpleTag for PhotometricInterpretation {
	const ID: TagId = TagId::PhotometricInterpretation;
	type Type = TypeShort;
	const NAME: &'static str = "Photometric Interpretation";
}

impl SimpleTag for StripOffsets {
	const ID: TagId = TagId::StripOffsets;
	type Type = Array<TypeNumber>;
	const NAME: &'static str = "Strip Offsets";
}

impl SimpleTag for SamplesPerPixel {
	const ID: TagId = TagId::SamplesPerPixel;
	type Type = TypeShort;
	const NAME: &'static str = "Samples per Pixel";
}

impl SimpleTag for RowsPerStrip {
	const ID: TagId = TagId::RowsPerStrip;
	type Type = TypeNumber;
	const NAME: &'static str = "Rows per Strip";
}

impl SimpleTag for StripByteCounts {
	const ID: TagId = TagId::StripByteCounts;
	type Type = Array<TypeNumber>;
	const NAME: &'static str = "Strip Byte Counts";
}

impl SimpleTag for SubIfd {
	const ID: TagId = TagId::SubIfd;
	type Type = TypeIfd;
	const NAME: &'static str = "SubIFD";
}

impl SimpleTag for JpegOffset {
	const ID: TagId = TagId::JpegOffset;
	type Type = TypeLong;
	const NAME: &'static str = "Jpeg Offset";
}

impl SimpleTag for JpegLength {
	const ID: TagId = TagId::JpegLength;
	type Type = TypeLong;
	const NAME: &'static str = "Jpeg Length";
}

impl SimpleTag for CfaPatternDim {
	const ID: TagId = TagId::CfaPatternDim;
	type Type = ConstArray<TypeShort, 2>;
	const NAME: &'static str = "CFA Pattern Dimension";
}

impl SimpleTag for CfaPattern {
	const ID: TagId = TagId::CfaPattern;
	type Type = Array<TypeByte>;
	const NAME: &'static str = "CFA Pattern";
}

impl SimpleTag for SonyDataOffset {
	const ID: TagId = TagId::SubIfd;
	type Type = TypeLong;
	const NAME: &'static str = "Sony Data Offset";
}

impl SimpleTag for SonyToneCurve {
	const ID: TagId = TagId::SonyToneCurve;
	type Type = TypeSonyToneCurve;
	const NAME: &'static str = "Sony Tone Curve";
}

pub trait Tag {
	type Output;

	fn get<R: Read + Seek>(ifd: &Ifd, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError>;
}

impl<T: SimpleTag> Tag for T {
	type Output = <T::Type as TagType>::Output;

	fn get<R: Read + Seek>(ifd: &Ifd, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		let tag_id = T::ID;
		let index: u32 = ifd.iter().position(|x| x.tag == tag_id).ok_or(TiffError::MissingTag)?.try_into()?;

		file.seek_from_start(ifd.current_ifd_offset + 2 + 12 * index + 2)?;
		T::Type::read(file)
	}
}

impl<T: Tag> Tag for Option<T> {
	type Output = Option<T::Output>;

	fn get<R: Read + Seek>(ifd: &Ifd, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		let result = T::get(ifd, file);

		match result {
			Err(TiffError::MissingTag) => Ok(None),
			Ok(x) => Ok(Some(x)),
			Err(x) => Err(x),
		}
	}
}
