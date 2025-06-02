use crate::tiff::file::{Endian, TiffRead};
use crate::tiff::tags::{BitsPerSample, CfaPattern, CfaPatternDim, Compression, ImageLength, ImageWidth, SonyToneCurve, StripByteCounts, StripOffsets, Tag, WhiteBalanceRggbLevels};
use crate::tiff::values::CurveLookupTable;
use crate::tiff::{Ifd, TiffError};
use crate::{RawImage, SubtractBlack, Transform};
use rawkit_proc_macros::Tag;
use std::io::{Read, Seek};

#[allow(dead_code)]
#[derive(Tag)]
struct Arw2Ifd {
	image_width: ImageWidth,
	image_height: ImageLength,
	bits_per_sample: BitsPerSample,
	compression: Compression,
	cfa_pattern: CfaPattern,
	cfa_pattern_dim: CfaPatternDim,
	strip_offsets: StripOffsets,
	strip_byte_counts: StripByteCounts,
	sony_tone_curve: SonyToneCurve,
	white_balance_levels: Option<WhiteBalanceRggbLevels>,
}

pub fn decode<R: Read + Seek>(ifd: Ifd, file: &mut TiffRead<R>) -> RawImage {
	let ifd = ifd.get_value::<Arw2Ifd, _>(file).unwrap();

	assert!(ifd.strip_offsets.len() == ifd.strip_byte_counts.len());
	assert!(ifd.strip_offsets.len() == 1);
	assert!(ifd.compression == 32767);

	let image_width: usize = ifd.image_width.try_into().unwrap();
	let image_height: usize = ifd.image_height.try_into().unwrap();
	let bits_per_sample: usize = ifd.bits_per_sample.into();
	assert!(bits_per_sample == 12);

	let [cfa_pattern_width, cfa_pattern_height] = ifd.cfa_pattern_dim;
	assert!(cfa_pattern_width == 2 && cfa_pattern_height == 2);

	file.seek_from_start(ifd.strip_offsets[0]).unwrap();
	let mut image = sony_arw2_load_raw(image_width, image_height, ifd.sony_tone_curve, file).unwrap();

	// Converting the bps from 12 to 14 so that ARW 2.3.1 and 2.3.5 have the same 14 bps.
	image.iter_mut().for_each(|x| *x <<= 2);

	RawImage {
		data: image,
		width: image_width,
		height: image_height,
		cfa_pattern: ifd.cfa_pattern.try_into().unwrap(),
		maximum: (1 << 14) - 1,
		black: SubtractBlack::CfaGrid([512, 512, 512, 512]), // TODO: Find the correct way to do this
		transform: Transform::Horizontal,
		camera_model: None,
		camera_white_balance: ifd.white_balance_levels.map(|arr| arr.map(|x| x as f64)),
		white_balance: None,
		camera_to_rgb: None,
	}
}

fn as_u32(buffer: &[u8], endian: Endian) -> Option<u32> {
	Some(match endian {
		Endian::Little => u32::from_le_bytes(buffer.try_into().ok()?),
		Endian::Big => u32::from_be_bytes(buffer.try_into().ok()?),
	})
}

fn as_u16(buffer: &[u8], endian: Endian) -> Option<u16> {
	Some(match endian {
		Endian::Little => u16::from_le_bytes(buffer.try_into().ok()?),
		Endian::Big => u16::from_be_bytes(buffer.try_into().ok()?),
	})
}

fn sony_arw2_load_raw<R: Read + Seek>(width: usize, height: usize, curve: CurveLookupTable, file: &mut TiffRead<R>) -> Option<Vec<u16>> {
	let mut image = vec![0_u16; height * width];
	let mut data = vec![0_u8; width + 1];

	for row in 0..height {
		file.read_exact(&mut data[0..width]).unwrap();

		let mut column = 0;
		let mut data_index = 0;

		while column < width - 30 {
			let data_value = as_u32(&data[data_index..][..4], file.endian()).unwrap();
			let max = (0x7ff & data_value) as u16;
			let min = (0x7ff & data_value >> 11) as u16;
			let index_to_set_max = 0x0f & data_value >> 22;
			let index_to_set_min = 0x0f & data_value >> 26;

			let max_minus_min = max as i32 - min as i32;
			let shift_by_bits = (0..4).find(|&shift| (0x80 << shift) > max_minus_min).unwrap_or(4);

			let mut pixels = [0_u16; 16];
			let mut bit = 30;
			for (i, pixel) in pixels.iter_mut().enumerate() {
				*pixel = match () {
					_ if i as u32 == index_to_set_max => max,
					_ if i as u32 == index_to_set_min => min,
					_ => {
						let result = as_u16(&data[(data_index + (bit >> 3))..][..2], file.endian()).unwrap();
						let result = ((result >> (bit & 7)) & 0x07f) << shift_by_bits;

						bit += 7;

						(result + min).min(0x7ff)
					}
				};
			}

			for value in pixels {
				image[row * width + column] = curve.get((value << 1).into()) >> 2;

				// Skip between interlaced columns
				column += 2;
			}

			// Switch to the opposite interlaced columns
			column -= if column & 1 == 0 { 31 } else { 1 };

			data_index += 16;
		}
	}

	Some(image)
}
