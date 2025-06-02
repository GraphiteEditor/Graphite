use super::types::{Array, ConstArray, TagType, TypeByte, TypeIfd, TypeLong, TypeNumber, TypeOrientation, TypeSRational, TypeSShort, TypeShort, TypeSonyToneCurve, TypeString};
use super::{Ifd, TagId, TiffError, TiffRead};
use std::io::{Read, Seek};

pub trait SimpleTag {
	type Type: TagType;

	const ID: TagId;
	const NAME: &'static str;
}

pub struct ImageWidth;
pub struct ImageLength;
pub struct BitsPerSample;
pub struct Compression;
pub struct PhotometricInterpretation;
pub struct Make;
pub struct Model;
pub struct StripOffsets;
pub struct Orientation;
pub struct SamplesPerPixel;
pub struct RowsPerStrip;
pub struct StripByteCounts;
pub struct SubIfd;
pub struct JpegOffset;
pub struct JpegLength;
pub struct SonyDataOffset;
pub struct SonyToneCurve;
pub struct BlackLevel;
pub struct WhiteBalanceRggbLevels;
pub struct CfaPatternDim;
pub struct CfaPattern;
pub struct ColorMatrix1;
pub struct ColorMatrix2;

impl SimpleTag for ImageWidth {
	type Type = TypeNumber;

	const ID: TagId = TagId::ImageWidth;
	const NAME: &'static str = "Image Width";
}

impl SimpleTag for ImageLength {
	type Type = TypeNumber;

	const ID: TagId = TagId::ImageLength;
	const NAME: &'static str = "Image Length";
}

impl SimpleTag for BitsPerSample {
	type Type = TypeShort;

	const ID: TagId = TagId::BitsPerSample;
	const NAME: &'static str = "Bits per Sample";
}

impl SimpleTag for Compression {
	type Type = TypeShort;

	const ID: TagId = TagId::Compression;
	const NAME: &'static str = "Compression";
}

impl SimpleTag for PhotometricInterpretation {
	type Type = TypeShort;

	const ID: TagId = TagId::PhotometricInterpretation;
	const NAME: &'static str = "Photometric Interpretation";
}

impl SimpleTag for Make {
	type Type = TypeString;

	const ID: TagId = TagId::Make;
	const NAME: &'static str = "Make";
}

impl SimpleTag for Model {
	type Type = TypeString;

	const ID: TagId = TagId::Model;
	const NAME: &'static str = "Model";
}

impl SimpleTag for StripOffsets {
	type Type = Array<TypeNumber>;

	const ID: TagId = TagId::StripOffsets;
	const NAME: &'static str = "Strip Offsets";
}

impl SimpleTag for Orientation {
	type Type = TypeOrientation;

	const ID: TagId = TagId::Orientation;
	const NAME: &'static str = "Orientation";
}

impl SimpleTag for SamplesPerPixel {
	type Type = TypeShort;

	const ID: TagId = TagId::SamplesPerPixel;
	const NAME: &'static str = "Samples per Pixel";
}

impl SimpleTag for RowsPerStrip {
	type Type = TypeNumber;

	const ID: TagId = TagId::RowsPerStrip;
	const NAME: &'static str = "Rows per Strip";
}

impl SimpleTag for StripByteCounts {
	type Type = Array<TypeNumber>;

	const ID: TagId = TagId::StripByteCounts;
	const NAME: &'static str = "Strip Byte Counts";
}

impl SimpleTag for SubIfd {
	type Type = TypeIfd;

	const ID: TagId = TagId::SubIfd;
	const NAME: &'static str = "SubIFD";
}

impl SimpleTag for JpegOffset {
	type Type = TypeLong;

	const ID: TagId = TagId::JpegOffset;
	const NAME: &'static str = "Jpeg Offset";
}

impl SimpleTag for JpegLength {
	type Type = TypeLong;

	const ID: TagId = TagId::JpegLength;
	const NAME: &'static str = "Jpeg Length";
}

impl SimpleTag for CfaPatternDim {
	type Type = ConstArray<TypeShort, 2>;

	const ID: TagId = TagId::CfaPatternDim;
	const NAME: &'static str = "CFA Pattern Dimension";
}

impl SimpleTag for CfaPattern {
	type Type = Array<TypeByte>;

	const ID: TagId = TagId::CfaPattern;
	const NAME: &'static str = "CFA Pattern";
}

impl SimpleTag for ColorMatrix1 {
	type Type = Array<TypeSRational>;

	const ID: TagId = TagId::ColorMatrix1;
	const NAME: &'static str = "Color Matrix 1";
}

impl SimpleTag for ColorMatrix2 {
	type Type = Array<TypeSRational>;

	const ID: TagId = TagId::ColorMatrix2;
	const NAME: &'static str = "Color Matrix 2";
}

impl SimpleTag for SonyDataOffset {
	type Type = TypeLong;

	const ID: TagId = TagId::SubIfd;
	const NAME: &'static str = "Sony Data Offset";
}

impl SimpleTag for SonyToneCurve {
	type Type = TypeSonyToneCurve;

	const ID: TagId = TagId::SonyToneCurve;
	const NAME: &'static str = "Sony Tone Curve";
}

impl SimpleTag for BlackLevel {
	type Type = ConstArray<TypeShort, 4>;

	const ID: TagId = TagId::BlackLevel;
	const NAME: &'static str = "Black Level";
}

impl SimpleTag for WhiteBalanceRggbLevels {
	type Type = ConstArray<TypeSShort, 4>;

	const ID: TagId = TagId::WhiteBalanceRggbLevels;
	const NAME: &'static str = "White Balance Levels (RGGB)";
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
