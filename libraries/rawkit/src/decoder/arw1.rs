use crate::tiff::Ifd;
use crate::tiff::file::TiffRead;
use crate::tiff::tags::SonyDataOffset;
use crate::{RawImage, SubtractBlack, Transform};
use bitstream_io::{BE, BitRead, BitReader, Endianness};
use std::io::{Read, Seek};

pub fn decode_a100<R: Read + Seek>(ifd: Ifd, file: &mut TiffRead<R>) -> RawImage {
	let data_offset = ifd.get_value::<SonyDataOffset, _>(file).unwrap();

	let image_width = 3881;
	let image_height = 2608;

	file.seek_from_start(data_offset).unwrap();
	let mut image = sony_arw_load_raw(image_width, image_height, &mut BitReader::<_, BE>::new(file)).unwrap();

	let len = image.len();
	image[len - image_width..].fill(0);

	RawImage {
		data: image,
		width: image_width,
		height: image_height,
		cfa_pattern: todo!(),
		#[allow(unreachable_code)]
		maximum: (1 << 12) - 1,
		black: SubtractBlack::None,
		transform: Transform::Horizontal,
		camera_model: None,
		camera_white_balance: None,
		white_balance: None,
		camera_to_rgb: None,
	}
}

fn read_and_huffman_decode_file<R: Read + Seek, E: Endianness>(huff: &[u16], file: &mut BitReader<R, E>) -> u32 {
	let number_of_bits = huff[0].into();
	let huffman_table = &huff[1..];

	// `number_of_bits` will be no more than 32, so the result is put into a u32
	let bits: u32 = file.read(number_of_bits).unwrap();
	let bits = bits as usize;

	let bits_to_seek_from = huffman_table[bits].to_le_bytes()[1] as i64 - number_of_bits as i64;
	file.seek_bits(std::io::SeekFrom::Current(bits_to_seek_from)).unwrap();

	huffman_table[bits].to_le_bytes()[0].into()
}

fn read_n_bits_from_file<R: Read + Seek, E: Endianness>(number_of_bits: u32, file: &mut BitReader<R, E>) -> u32 {
	// `number_of_bits` will be no more than 32, so the result is put into a u32
	file.read(number_of_bits).unwrap()
}

/// ljpeg is a lossless variant of JPEG which gets used for decoding the embedded (thumbnail) preview images in raw files
fn ljpeg_diff<R: Read + Seek, E: Endianness>(huff: &[u16], file: &mut BitReader<R, E>, dng_version: Option<u32>) -> i32 {
	let length = read_and_huffman_decode_file(huff, file);

	if length == 16 && dng_version.map(|x| x >= 0x1010000).unwrap_or(true) {
		return -32768;
	}

	let diff = read_n_bits_from_file(length, file) as i32;

	if length == 0 || (diff & (1 << (length - 1))) == 0 { diff - (1 << length) - 1 } else { diff }
}

fn sony_arw_load_raw<R: Read + Seek>(width: usize, height: usize, file: &mut BitReader<R, BE>) -> Option<Vec<u16>> {
	const TABLE: [u16; 18] = [
		0x0f11, 0x0f10, 0x0e0f, 0x0d0e, 0x0c0d, 0x0b0c, 0x0a0b, 0x090a, 0x0809, 0x0708, 0x0607, 0x0506, 0x0405, 0x0304, 0x0303, 0x0300, 0x0202, 0x0201,
	];

	let mut huffman_table = [0_u16; 32770];
	// The first element is the number of bits to read
	huffman_table[0] = 15;

	let mut n = 0;
	for x in TABLE {
		let first_byte = x >> 8;
		let repeats = 0x8000 >> first_byte;
		for _ in 0_u16..repeats {
			n += 1;
			huffman_table[n] = x;
		}
	}

	let mut sum = 0;
	let mut image = vec![0_u16; width * height];
	for column in (0..width).rev() {
		for row in (0..height).step_by(2).chain((1..height).step_by(2)) {
			sum += ljpeg_diff(&huffman_table, file, None);

			if (sum >> 12) != 0 {
				return None;
			}

			if row < height {
				image[row * width + column] = sum as u16;
			}
		}
	}

	Some(image)
}
