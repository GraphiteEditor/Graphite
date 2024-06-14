use crate::tiff::file::{Endian, TiffRead};
use crate::tiff::tags::{BitsPerSample, CfaPattern, CfaPatternDim, Compression, ImageLength, ImageWidth, SonyToneCurve, StripByteCounts, StripOffsets, Tag};
use crate::tiff::values::CurveLookupTable;
use crate::tiff::{Ifd, TiffError};
use crate::RawImage;
use std::io::{Read, Seek};
use tag_derive::Tag;

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
}

pub fn decode<R: Read + Seek>(ifd: Ifd, file: &mut TiffRead<R>) -> RawImage {
	let ifd = ifd.get_value::<Arw2Ifd, _>(file).unwrap();

	assert!(ifd.strip_offsets.len() == ifd.strip_byte_counts.len());
	assert!(ifd.strip_offsets.len() == 1);
	let image_width: usize = ifd.image_width.try_into().unwrap();
	let image_height: usize = ifd.image_height.try_into().unwrap();
	let bits_per_sample: usize = ifd.bits_per_sample.into();
	assert!(ifd.compression == 32767);
	let [cfa_pattern_width, cfa_pattern_height] = ifd.cfa_pattern_dim;
	assert!(cfa_pattern_width == 2 && cfa_pattern_height == 2);
	let cfa_pattern = ifd.cfa_pattern;

	file.seek_from_start(ifd.strip_offsets[0]).unwrap();
	let mut image = sony_arw2_load_raw(image_width, image_height, ifd.sony_tone_curve, file).unwrap();

	// Converting the bps from 12 to 14 so that ARW 2.3.1 and 2.3.5 have the same 14 bps.
	image.iter_mut().for_each(|x| *x *= 4);

	RawImage {
		data: image,
		width: image_width,
		height: image_height,
	}
}

fn get_u32(buf: &[u8], endian: Endian) -> Option<u32> {
	Some(match endian {
		Endian::Little => u32::from_le_bytes(buf.try_into().ok()?),
		Endian::Big => u32::from_be_bytes(buf.try_into().ok()?),
	})
}

fn get_u16(buf: &[u8], endian: Endian) -> Option<u16> {
	Some(match endian {
		Endian::Little => u16::from_le_bytes(buf.try_into().ok()?),
		Endian::Big => u16::from_be_bytes(buf.try_into().ok()?),
	})
}

fn sony_arw2_load_raw<R: Read + Seek>(width: usize, height: usize, curve: CurveLookupTable, file: &mut TiffRead<R>) -> Option<Vec<u16>> {
	let mut image = vec![0u16; height * width];
	let mut data = vec![0u8; width + 1];
	let mut pix = [0u16; 16];

	for row in 0..height {
		file.read_exact(&mut data[0..width]).unwrap();

		let mut col = 0;
		let mut dp = 0;
		while col < width - 30 {
			let val = get_u32(&data[dp..][..4], file.endian()).unwrap();
			let max = (0x7ff & val) as u16;
			let min = (0x7ff & val >> 11) as u16;
			let imax = 0x0f & val >> 22;
			let imin = 0x0f & val >> 26;

			let mut sh = 0;
			while sh < 4 && 0x80 << sh <= max as i32 - min as i32 {
				sh += 1;
			}

			let mut bit = 30;
			for i in 0..16 {
				if i == imax as usize {
					pix[i] = max;
				} else if i == imin as usize {
					pix[i] = min;
				} else {
					pix[i] = ((get_u16(&data[(dp + (bit >> 3))..][..2], file.endian()).unwrap() >> (bit & 7) & 0x07f) << sh) + min;
					if pix[i] > 0x7ff {
						pix[i] = 0x7ff;
					}
					bit += 7;
				}
			}

			for i in 0..16 {
				image[row * width + col] = curve.get((pix[i] << 1).into()) >> 2;
				col += 2;
			}

			col -= if col & 1 != 0 { 1 } else { 31 };

			dp += 16;
		}
	}

	Some(image)
}
