use crate::tiff::file::TiffRead;
use crate::tiff::tags::{BITS_PER_SAMPLE, CFA_PATTERN, CFA_PATTERN_DIM, COMPRESSION, IMAGE_LENGTH, IMAGE_WIDTH, ROWS_PER_STRIP, SAMPLES_PER_PIXEL, SONY_DATA_OFFSET, STRIP_BYTE_COUNTS, STRIP_OFFSETS};
use crate::tiff::Ifd;
use crate::RawImage;
use bitstream_io::{BitRead, BitReader, Endianness, BE};
use std::io::{Read, Seek};

pub fn decode_a100<R: Read + Seek>(ifd: Ifd, file: &mut TiffRead<R>) -> RawImage {
	// let strip_offsets = ifd.get(STRIP_OFFSETS, file).unwrap();
	// let strip_byte_counts = ifd.get(STRIP_BYTE_COUNTS, file).unwrap();
	// assert!(strip_offsets.len() == strip_byte_counts.len());

	let data_offset = ifd.get(SONY_DATA_OFFSET, file).unwrap();

	let image_width: usize = ifd.get(IMAGE_WIDTH, file).unwrap().try_into().unwrap();
	let image_height: usize = ifd.get(IMAGE_LENGTH, file).unwrap().try_into().unwrap();
	// let rows_per_strip: usize = ifd.get(ROWS_PER_STRIP, file).unwrap().try_into().unwrap();
	// let bits_per_sample: usize = ifd.get(BITS_PER_SAMPLE, file).unwrap().into();
	// let bytes_per_sample: usize = bits_per_sample.div_ceil(8);
	// let samples_per_pixel: usize = ifd.get(SAMPLES_PER_PIXEL, file).unwrap().into();
	// let compression = ifd.get(COMPRESSION, file).unwrap();
	// assert!(compression == 32767);

	// let photometric_interpretation = ifd.get(PHOTOMETRIC_INTERPRETATION, file).unwrap();

	// let [cfa_pattern_width, cfa_pattern_height] = ifd.get(CFA_PATTERN_DIM, file).unwrap();
	// assert!(cfa_pattern_width == 2 && cfa_pattern_height == 2);

	// let cfa_pattern = ifd.get(CFA_PATTERN, file).unwrap();

	let image_width = 3881;
	let image_height = 2608;

	// assert!(strip_offsets.len() == 1);
	file.seek_from_start(data_offset).unwrap();

	let mut image = sony_arw_load_raw(image_width, image_height, &mut BitReader::<_, BE>::new(file)).unwrap();

	let len = image.len();
	image[len - 3881..].fill(0);

	RawImage {
		data: image,
		width: image_width,
		height: image_height,
	}
}

fn getbithuff<R: Read + Seek, E: Endianness>(nbits: u32, huff: Option<&[u16]>, file: &mut BitReader<R, E>) -> u32 {
	let x: u32 = file.read(nbits).unwrap();

	if let Some(huff) = huff {
		file.seek_bits(std::io::SeekFrom::Current(huff[x as usize].to_le_bytes()[1] as i64 - nbits as i64)).unwrap();
		huff[x as usize].to_le_bytes()[0].into()
	} else {
		x
	}
}

fn get_huff<R: Read + Seek, E: Endianness>(huff: &[u16], file: &mut BitReader<R, E>) -> u32 {
	getbithuff(huff[0].into(), Some(&huff[1..]), file)
}

fn get_bits<R: Read + Seek, E: Endianness>(nbits: u32, file: &mut BitReader<R, E>) -> u32 {
	getbithuff(nbits, None, file)
}

fn ljpeg_diff<R: Read + Seek, E: Endianness>(huff: &[u16], file: &mut BitReader<R, E>, dng_version: Option<u32>) -> i32 {
	let len = get_huff(huff, file);

	if len == 16 && dng_version.map(|x| x >= 0x1010000).unwrap_or(true) {
		return -32768;
	}

	let mut diff = get_bits(len, file) as i32;
	if len == 0 || (diff & (1 << (len - 1))) == 0 {
		diff -= (1 << len) - 1;
	}

	diff as i32
}

fn sony_arw_load_raw<R: Read + Seek>(width: usize, height: usize, file: &mut BitReader<R, BE>) -> Option<Vec<u16>> {
	let mut huff = [0u16; 32770];
	const TAB: [u16; 18] = [
		0xf11, 0xf10, 0xe0f, 0xd0e, 0xc0d, 0xb0c, 0xa0b, 0x90a, 0x809, 0x708, 0x607, 0x506, 0x405, 0x304, 0x303, 0x300, 0x202, 0x201,
	];

	huff[0] = 15;
	let mut n: usize = 0;
	for x in TAB {
		for _ in 0..(32768 >> (x >> 8)) {
			n += 1;
			huff[n] = x;
		}
	}

	// getbits(-1);

	let mut sum: i32 = 0;
	let mut image = vec![0u16; width * height];
	let mut counter = 0;
	for column in (0..width).rev() {
		for row in (0..height).step_by(2).chain((1..height).step_by(2)) {
			sum += ljpeg_diff(&huff, file, None);
			if (sum >> 12) != 0 {
				return None;
			}
			if counter < 10 {
				println!("sum: {}", sum);
				counter += 1;
			}
			if row < height {
				image[row * width + column] = sum as u16;
			}
		}
	}

	Some(image)
}
