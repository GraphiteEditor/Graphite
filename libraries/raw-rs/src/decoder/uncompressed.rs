use crate::tiff::file::TiffRead;
use crate::tiff::tags::{BITS_PER_SAMPLE, CFA_PATTERN, CFA_PATTERN_DIM, COMPRESSION, IMAGE_LENGTH, IMAGE_WIDTH, ROWS_PER_STRIP, SAMPLES_PER_PIXEL, STRIP_BYTE_COUNTS, STRIP_OFFSETS};
use crate::tiff::Ifd;
use crate::RawImage;
use std::io::{Read, Seek};

pub fn decode<R: Read + Seek>(ifd: Ifd, file: &mut TiffRead<R>) -> RawImage {
	let strip_offsets = ifd.get(STRIP_OFFSETS, file).unwrap();
	let strip_byte_counts = ifd.get(STRIP_BYTE_COUNTS, file).unwrap();
	assert!(strip_offsets.len() == strip_byte_counts.len());

	let image_width: usize = ifd.get(IMAGE_WIDTH, file).unwrap().try_into().unwrap();
	let image_height: usize = ifd.get(IMAGE_LENGTH, file).unwrap().try_into().unwrap();
	let rows_per_strip: usize = ifd.get(ROWS_PER_STRIP, file).unwrap().try_into().unwrap();
	let bits_per_sample: usize = ifd.get(BITS_PER_SAMPLE, file).unwrap().into();
	let bytes_per_sample: usize = bits_per_sample.div_ceil(8);
	let samples_per_pixel: usize = ifd.get(SAMPLES_PER_PIXEL, file).unwrap().into();
	let compression = ifd.get(COMPRESSION, file).unwrap();
	assert!(compression == 1); // 1 is the value for uncompressed format
						   // let photometric_interpretation = ifd.get(PHOTOMETRIC_INTERPRETATION, file).unwrap();

	let [cfa_pattern_width, cfa_pattern_height] = ifd.get(CFA_PATTERN_DIM, file).unwrap();
	assert!(cfa_pattern_width == 2 && cfa_pattern_height == 2);

	let cfa_pattern = ifd.get(CFA_PATTERN, file).unwrap();

	let rows_per_strip_last = image_height % rows_per_strip;
	let bytes_per_row = bytes_per_sample * samples_per_pixel * image_width;

	let mut image: Vec<u16> = Vec::with_capacity(image_height * image_width);

	for i in 0..strip_offsets.len() {
		file.seek_from_start(strip_offsets[i]).unwrap();
		let row_count = if i == strip_offsets.len() { rows_per_strip_last } else { rows_per_strip };
		for _ in 0..row_count {
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
