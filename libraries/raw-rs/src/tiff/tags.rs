use super::types::{Array, ConstArray, TagType, TypeByte, TypeIfd, TypeLong, TypeNumber, TypeShort, TypeSonyToneCurve};
use super::{Ifd, TagId, TiffError, TiffRead};

use std::io::{Read, Seek};

pub struct TagOld<T: TagType> {
	tag_id: TagId,
	name: &'static str,
	tag_type: std::marker::PhantomData<T>,
}

impl<T: TagType> TagOld<T> {
	const fn new(tag_id: TagId, name: &'static str) -> Self {
		TagOld {
			tag_id,
			name,
			tag_type: std::marker::PhantomData,
		}
	}

	pub fn id(&self) -> TagId {
		self.tag_id
	}

	pub fn name(&self) -> &'static str {
		self.name
	}
}

pub const IMAGE_WIDTH: TagOld<TypeNumber> = TagOld::new(TagId::ImageWidth, "Image Width");
pub const IMAGE_LENGTH: TagOld<TypeNumber> = TagOld::new(TagId::ImageLength, "Image Length");
pub const BITS_PER_SAMPLE: TagOld<TypeShort> = TagOld::new(TagId::BitsPerSample, "Bits per Sample");
pub const COMPRESSION: TagOld<TypeShort> = TagOld::new(TagId::Compression, "Compression");
pub const PHOTOMETRIC_INTERPRETATION: TagOld<TypeShort> = TagOld::new(TagId::PhotometricInterpretation, "Photometric Interpretation");
pub const STRIP_OFFSETS: TagOld<Array<TypeNumber>> = TagOld::new(TagId::StripOffsets, "Strip Offsets");
pub const SAMPLES_PER_PIXEL: TagOld<TypeShort> = TagOld::new(TagId::SamplesPerPixel, "Samples per Pixel");
pub const ROWS_PER_STRIP: TagOld<TypeNumber> = TagOld::new(TagId::RowsPerStrip, "Rows per Strip");
pub const STRIP_BYTE_COUNTS: TagOld<Array<TypeNumber>> = TagOld::new(TagId::StripByteCounts, "Strip Byte Counts");
pub const SUBIFD: TagOld<TypeIfd> = TagOld::new(TagId::SubIfd, "SubIFD");
pub const JPEG_OFFSET: TagOld<TypeLong> = TagOld::new(TagId::JpegOffset, "Jpeg Offset");
pub const JPEG_LENGTH: TagOld<TypeLong> = TagOld::new(TagId::JpegLength, "Jpeg Length");
pub const APPLICATION_NOTES: TagOld<Array<TypeByte>> = TagOld::new(TagId::ApplicationNotes, "Application Notes");
pub const CFA_PATTERN_DIM: TagOld<ConstArray<TypeShort, 2>> = TagOld::new(TagId::CfaPatternDim, "CFA Pattern Dimension");
pub const CFA_PATTERN: TagOld<Array<TypeByte>> = TagOld::new(TagId::CfaPattern, "CFA Pattern");
pub const SONY_DATA_OFFSET: TagOld<TypeLong> = TagOld::new(TagId::SubIfd, "Sony Data Offset");
pub const SONY_TONE_CURVE: TagOld<TypeSonyToneCurve> = TagOld::new(TagId::SonyToneCurve, "Sony Tone Curve");

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

pub struct TagValue<T: Tag> {
	pub value: T::Output,
}
