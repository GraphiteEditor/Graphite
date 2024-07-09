use crate::tiff::file::TiffRead;
use crate::tiff::tags::{BitsPerSample, CfaPattern, CfaPatternDim, Compression, ImageLength, ImageWidth, RowsPerStrip, StripByteCounts, StripOffsets, Tag};
use crate::tiff::{Ifd, TiffError};
use crate::RawImage;
use std::io::{Read, Seek};
use tag_derive::Tag;

#[allow(dead_code)]
#[derive(Tag)]
struct ArwUncompressedIfd {
	image_width: ImageWidth,
	image_height: ImageLength,
	rows_per_strip: RowsPerStrip,
	bits_per_sample: BitsPerSample,
	compression: Compression,
	cfa_pattern: CfaPattern,
	cfa_pattern_dim: CfaPatternDim,
	strip_offsets: StripOffsets,
	strip_byte_counts: StripByteCounts,
}

pub fn decode<R: Read + Seek>(ifd: Ifd, file: &mut TiffRead<R>) -> RawImage {
	let ifd = ifd.get_value::<ArwUncompressedIfd, _>(file).unwrap();

	assert!(ifd.strip_offsets.len() == ifd.strip_byte_counts.len());
	assert!(ifd.strip_offsets.len() == 1);
	assert!(ifd.compression == 1); // 1 is the value for uncompressed format

	let image_width: usize = ifd.image_width.try_into().unwrap();
	let image_height: usize = ifd.image_height.try_into().unwrap();
	let rows_per_strip: usize = ifd.rows_per_strip.try_into().unwrap();
	let _bits_per_sample: usize = ifd.bits_per_sample.into();
	let [cfa_pattern_width, cfa_pattern_height] = ifd.cfa_pattern_dim;
	assert!(cfa_pattern_width == 2 && cfa_pattern_height == 2);

	let mut image: Vec<u16> = Vec::with_capacity(image_height * image_width);

	for i in 0..ifd.strip_offsets.len() {
		file.seek_from_start(ifd.strip_offsets[i]).unwrap();

		let last = i == ifd.strip_offsets.len();
		let rows = if last { image_height % rows_per_strip } else { rows_per_strip };

		for _ in 0..rows {
			for _ in 0..image_width {
				image.push(file.read_u16().unwrap());
			}
		}
	}

	RawImage {
		data: image,
		width: image_width,
		height: image_height,
	}
}
