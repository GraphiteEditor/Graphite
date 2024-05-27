use super::types::{Array, ConstArray, TagType, TypeByte, TypeLong, TypeNumber, TypeShort};
use super::TagId;

pub struct Tag<T: TagType> {
	tag_id: TagId,
	name: &'static str,
	tag_type: std::marker::PhantomData<T>,
}

impl<T: TagType> Tag<T> {
	const fn new(tag_id: TagId, name: &'static str) -> Self {
		Tag {
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

pub const IMAGE_WIDTH: Tag<TypeNumber> = Tag::new(TagId::ImageWidth, "Image Width");
pub const IMAGE_LENGTH: Tag<TypeNumber> = Tag::new(TagId::ImageLength, "Image Length");
pub const BITS_PER_SAMPLE: Tag<TypeShort> = Tag::new(TagId::BitsPerSample, "Bits per Sample");
pub const COMPRESSION: Tag<TypeShort> = Tag::new(TagId::Compression, "Compression");
pub const PHOTOMETRIC_INTERPRETATION: Tag<TypeShort> = Tag::new(TagId::PhotometricInterpretation, "Photometric Interpretation");
pub const STRIP_OFFSETS: Tag<Array<TypeNumber>> = Tag::new(TagId::StripOffsets, "Strip Offsets");
pub const SAMPLES_PER_PIXEL: Tag<TypeShort> = Tag::new(TagId::SamplesPerPixel, "Samples per Pixel");
pub const ROWS_PER_STRIP: Tag<TypeNumber> = Tag::new(TagId::RowsPerStrip, "Rows per Strip");
pub const STRIP_BYTE_COUNTS: Tag<Array<TypeNumber>> = Tag::new(TagId::StripByteCounts, "Strip Byte Counts");
pub const SONY_SUBIFD: Tag<TypeLong> = Tag::new(TagId::SonySubIfd, "Sony SubIFD");
pub const JPEG_OFFSET: Tag<TypeLong> = Tag::new(TagId::JpegOffset, "Jpeg Offset");
pub const JPEG_LENGTH: Tag<TypeLong> = Tag::new(TagId::JpegLength, "Jpeg Length");
pub const CFA_PATTERN_DIM: Tag<ConstArray<TypeShort, 2>> = Tag::new(TagId::CfaPatternDim, "CFA Pattern Dimension");
pub const CFA_PATTERN: Tag<Array<TypeByte>> = Tag::new(TagId::CfaPattern, "CFA Pattern");
