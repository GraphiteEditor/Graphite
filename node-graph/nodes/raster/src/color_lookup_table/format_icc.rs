//! ICC "abstract" and "device link" profiles, evaluated through their `A2B0` tag.
//!
//! <https://www.color.org/specification/ICC.1-2022-05.pdf>

use super::{Curve, LabEncoding, Lut, LutParseError, LutTable, Stage, parse_triple};

pub(super) fn parse(bytes: &[u8]) -> Result<Lut, LutParseError> {
	let header = bytes.get(..128).ok_or(LutParseError::Unreadable)?;
	let lab = match (&header[12..16], &header[16..20], &header[20..24]) {
		(b"link", b"RGB ", b"RGB ") => false,
		(b"abst", b"Lab ", b"Lab ") => true,
		(b"link" | b"abst", ..) => return Err(LutParseError::IccColorSpaces),
		_ => return Err(LutParseError::IccProfileClass),
	};

	parse_transform(bytes, lab).ok_or(LutParseError::Unreadable)
}

/// The `A2B0` tag's transform, where `lab` is set for an abstract profile.
fn parse_transform(bytes: &[u8], lab: bool) -> Option<Lut> {
	let mut tag = None;
	for index in 0..big_endian_u32(bytes, 128)? as usize {
		let entry = 132 + index * 12;
		if bytes.get(entry..entry + 4)? == b"A2B0" {
			let offset = big_endian_u32(bytes, entry + 4)? as usize;
			let size = big_endian_u32(bytes, entry + 8)? as usize;
			tag = bytes.get(offset..offset + size);
		}
	}
	let tag = tag?;

	match tag.get(0..4)? {
		b"mft1" => parse_lut(tag, 1, lab.then_some(LabEncoding::Standard)),
		b"mft2" => parse_lut(tag, 2, lab.then_some(LabEncoding::Legacy)),
		b"mAB " => parse_lut_a_to_b(tag, lab.then_some(LabEncoding::Standard)),
		_ => None,
	}
}

/// The `lut8Type` and `lut16Type` layouts: input tables, a CLUT with the first channel varying slowest, then output tables.
fn parse_lut(tag: &[u8], width: usize, lab_encoding: Option<LabEncoding>) -> Option<Lut> {
	let (inputs, outputs, grid) = (*tag.get(8)? as usize, *tag.get(9)? as usize, *tag.get(10)? as usize);
	if inputs != 3 || outputs != 3 || grid < 2 {
		return None;
	}
	let (input_entries, output_entries, mut offset) = match width {
		1 => (256, 256, 48),
		_ => (big_endian_u16(tag, 48)? as usize, big_endian_u16(tag, 50)? as usize, 52),
	};

	let input_curves = read_curve_set(tag, &mut offset, input_entries, width)?;
	let samples = read_samples(tag, offset, grid * grid * grid * 3, width)?;
	offset += grid * grid * grid * 3 * width;
	let output_curves = read_curve_set(tag, &mut offset, output_entries, width)?;

	let entries: Vec<[f32; 3]> = samples.chunks(3).map(parse_triple).collect::<Option<_>>()?;
	Some(Lut {
		input_curves: Some(input_curves),
		output_stages: vec![Stage::Curves(output_curves)],
		lab_encoding,
		..Lut::new(Some(LutTable::ThreeDimensional {
			size: [grid; 3],
			red_fastest: false,
			entries,
		}))
	})
}

/// The `lutAToBType` layout: A curves, a CLUT with the first channel varying slowest, M curves, a matrix, then B curves, located by offsets that are zero when absent.
fn parse_lut_a_to_b(tag: &[u8], lab_encoding: Option<LabEncoding>) -> Option<Lut> {
	if *tag.get(8)? != 3 || *tag.get(9)? != 3 {
		return None;
	}
	let offset_at = |at: usize| big_endian_u32(tag, at).map(|offset| offset as usize);
	let (b_curves, matrix, m_curves, clut, a_curves) = (offset_at(12)?, offset_at(16)?, offset_at(20)?, offset_at(24)?, offset_at(28)?);
	// The A curves and the CLUT come together or not at all
	if b_curves == 0 || (clut == 0) != (a_curves == 0) {
		return None;
	}

	let mut input_curves = None;
	let mut table = None;
	if clut != 0 {
		let grid = [*tag.get(clut)? as usize, *tag.get(clut + 1)? as usize, *tag.get(clut + 2)? as usize];
		let width = *tag.get(clut + 16)? as usize;
		if grid.iter().any(|&axis| axis < 2) || !matches!(width, 1 | 2) {
			return None;
		}
		let samples = read_samples(tag, clut + 20, grid[0] * grid[1] * grid[2] * 3, width)?;
		let entries: Vec<[f32; 3]> = samples.chunks(3).map(parse_triple).collect::<Option<_>>()?;
		input_curves = Some(read_curve_elements(tag, a_curves)?);
		table = Some(LutTable::ThreeDimensional {
			size: grid,
			red_fastest: false,
			entries,
		});
	}

	let mut output_stages = Vec::new();
	if m_curves != 0 {
		output_stages.push(Stage::Curves(read_curve_elements(tag, m_curves)?));
	}
	if matrix != 0 {
		let values: Vec<f32> = (0..12).map(|index| fixed_16(tag, matrix + index * 4)).collect::<Option<_>>()?;
		output_stages.push(Stage::Matrix {
			matrix: [[values[0], values[1], values[2]], [values[3], values[4], values[5]], [values[6], values[7], values[8]]],
			offset: [values[9], values[10], values[11]],
		});
	}
	output_stages.push(Stage::Curves(read_curve_elements(tag, b_curves)?));

	Some(Lut {
		input_curves,
		output_stages,
		lab_encoding,
		..Lut::new(table)
	})
}

/// Three consecutive `curveType` or `parametricCurveType` elements, each padded to a four byte boundary.
fn read_curve_elements(tag: &[u8], start: usize) -> Option<[Curve; 3]> {
	let mut offset = start;
	let mut curves = Vec::new();
	for _ in 0..3 {
		let (curve, length) = match tag.get(offset..offset + 4)? {
			b"curv" => {
				let count = big_endian_u32(tag, offset + 8)? as usize;
				let curve = match count {
					0 => Curve::Power(1.),
					1 => Curve::Power(big_endian_u16(tag, offset + 12)? as f32 / 256.),
					_ => Curve::Sampled(read_samples(tag, offset + 12, count, 2)?),
				};
				(curve, 12 + count * 2)
			}
			b"para" => {
				let function = big_endian_u16(tag, offset + 8)?;
				let count = *[1, 3, 4, 5, 7].get(function as usize)?;
				let parameters = (0..count).map(|index| fixed_16(tag, offset + 12 + index * 4)).collect::<Option<_>>()?;
				(Curve::Parametric { function, parameters }, 12 + count * 4)
			}
			_ => return None,
		};
		curves.push(curve);
		offset += length.next_multiple_of(4);
	}
	curves.try_into().ok()
}

/// Three consecutive sampled curves of `entries` samples each, advancing `offset` past them.
fn read_curve_set(bytes: &[u8], offset: &mut usize, entries: usize, width: usize) -> Option<[Curve; 3]> {
	let mut curves = Vec::new();
	for _ in 0..3 {
		curves.push(Curve::Sampled(read_samples(bytes, *offset, entries, width)?));
		*offset += entries * width;
	}
	curves.try_into().ok()
}

/// `count` unsigned samples of `width` bytes each, scaled to 0..1.
fn read_samples(bytes: &[u8], offset: usize, count: usize, width: usize) -> Option<Vec<f32>> {
	let data = bytes.get(offset..offset + count * width)?;
	Some(match width {
		1 => data.iter().map(|&byte| byte as f32 / 255.).collect(),
		_ => data.chunks(2).map(|pair| u16::from_be_bytes([pair[0], pair[1]]) as f32 / 65535.).collect(),
	})
}

fn big_endian_u32(bytes: &[u8], offset: usize) -> Option<u32> {
	Some(u32::from_be_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?))
}

fn big_endian_u16(bytes: &[u8], offset: usize) -> Option<u16> {
	Some(u16::from_be_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?))
}

/// An ICC `s15Fixed16Number`.
fn fixed_16(bytes: &[u8], offset: usize) -> Option<f32> {
	Some(big_endian_u32(bytes, offset)? as i32 as f32 / 65536.)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::color_lookup_table::tests::{assert_close, assert_close_within};

	/// A profile of the given classes holding one `A2B0` tag.
	fn icc_profile(class: &[u8; 4], space: &[u8; 4], pcs: &[u8; 4], tag: &[u8]) -> Vec<u8> {
		let mut bytes = vec![0; 128];
		bytes[8] = 2;
		bytes[12..16].copy_from_slice(class);
		bytes[16..20].copy_from_slice(space);
		bytes[20..24].copy_from_slice(pcs);
		bytes[36..40].copy_from_slice(b"acsp");
		bytes.extend_from_slice(&1_u32.to_be_bytes());
		bytes.extend_from_slice(b"A2B0");
		bytes.extend_from_slice(&144_u32.to_be_bytes());
		bytes.extend_from_slice(&(tag.len() as u32).to_be_bytes());
		bytes.extend_from_slice(tag);
		bytes
	}

	#[test]
	fn a_profile_of_the_wrong_kind_says_why_it_is_refused() {
		let parse = |class, space, pcs| Lut::parse(&icc_profile(class, space, pcs, &[])).unwrap_err();

		// A monitor profile, a CMYK device link, and an abstract profile connected through XYZ
		assert_eq!(parse(b"mntr", b"RGB ", b"XYZ "), LutParseError::IccProfileClass);
		assert_eq!(parse(b"link", b"CMYK", b"Lab "), LutParseError::IccColorSpaces);
		assert_eq!(parse(b"abst", b"XYZ ", b"XYZ "), LutParseError::IccColorSpaces);

		// The right kind of profile with no readable transform
		assert_eq!(parse(b"link", b"RGB ", b"RGB "), LutParseError::Unreadable);
	}

	#[test]
	fn device_link_lut8_orders_first_channel_slowest() {
		let mut tag = vec![0; 48];
		tag[0..4].copy_from_slice(b"mft1");
		tag[8] = 3;
		tag[9] = 3;
		tag[10] = 2;
		let identity: Vec<u8> = (0..=255).collect();
		for _ in 0..3 {
			tag.extend_from_slice(&identity);
		}
		for r in 0..2_u8 {
			for g in 0..2_u8 {
				for b in 0..2_u8 {
					// Swaps red and blue
					tag.extend_from_slice(&[b * 255, g * 255, r * 255]);
				}
			}
		}
		for _ in 0..3 {
			tag.extend_from_slice(&identity);
		}
		let lut = Lut::parse(&icc_profile(b"link", b"RGB ", b"RGB ", &tag)).unwrap();
		assert_close(lut.apply([0.25, 0.5, 0.75]), [0.75, 0.5, 0.25]);
	}

	#[test]
	fn abstract_profile_round_trips_through_lab() {
		let mut tag = vec![0; 52];
		tag[0..4].copy_from_slice(b"mft2");
		tag[8] = 3;
		tag[9] = 3;
		tag[10] = 2;
		tag[48..50].copy_from_slice(&2_u16.to_be_bytes());
		tag[50..52].copy_from_slice(&2_u16.to_be_bytes());
		let identity = [0_u16.to_be_bytes(), 65535_u16.to_be_bytes()].concat();
		for _ in 0..3 {
			tag.extend_from_slice(&identity);
		}
		for l in 0..2_u16 {
			for a in 0..2_u16 {
				for b in 0..2_u16 {
					for value in [l, a, b] {
						tag.extend_from_slice(&(value * 65535).to_be_bytes());
					}
				}
			}
		}
		for _ in 0..3 {
			tag.extend_from_slice(&identity);
		}
		let lut = Lut::parse(&icc_profile(b"abst", b"Lab ", b"Lab ", &tag)).unwrap();
		let color = [0.25, 0.5, 0.75];
		assert_close_within(lut.apply(color), color, 1e-3);
	}

	#[test]
	fn lut_a_to_b_applies_curves_matrix_and_grid() {
		let curv = |gamma: Option<u16>| -> Vec<u8> {
			let mut bytes = b"curv\0\0\0\0".to_vec();
			match gamma {
				None => bytes.extend_from_slice(&0_u32.to_be_bytes()),
				Some(gamma) => {
					bytes.extend_from_slice(&1_u32.to_be_bytes());
					bytes.extend_from_slice(&gamma.to_be_bytes());
					bytes.extend_from_slice(&[0, 0]);
				}
			}
			bytes
		};
		let para = |gamma: i32| -> Vec<u8> {
			let mut bytes = b"para\0\0\0\0\0\0\0\0".to_vec();
			bytes.extend_from_slice(&gamma.to_be_bytes());
			bytes
		};

		// Identity A curves and grid, unit-gamma M curves, a matrix swapping red and blue, then unit-gamma parametric B curves
		let mut tag = b"mAB \0\0\0\0\x03\x03\0\0".to_vec();
		for offset in [232_u32, 184, 136, 68, 32] {
			tag.extend_from_slice(&offset.to_be_bytes());
		}
		for _ in 0..3 {
			tag.extend(curv(None));
		}
		tag.extend_from_slice(&[2, 2, 2]);
		tag.extend_from_slice(&[0; 13]);
		tag.push(2);
		tag.extend_from_slice(&[0; 3]);
		for r in 0..2_u16 {
			for g in 0..2_u16 {
				for b in 0..2_u16 {
					for value in [r, g, b] {
						tag.extend_from_slice(&(value * 65535).to_be_bytes());
					}
				}
			}
		}
		for _ in 0..3 {
			tag.extend(curv(Some(0x0100)));
		}
		for value in [0_i32, 0, 1, 0, 1, 0, 1, 0, 0, 0, 0, 0] {
			tag.extend_from_slice(&(value << 16).to_be_bytes());
		}
		for _ in 0..3 {
			tag.extend(para(1 << 16));
		}
		assert_eq!(tag.len(), 280);

		let lut = Lut::parse(&icc_profile(b"link", b"RGB ", b"RGB ", &tag)).unwrap();
		assert_close(lut.apply([0.25, 0.5, 0.75]), [0.75, 0.5, 0.25]);
	}

	#[test]
	fn lut_a_to_b_without_a_grid_applies_its_curves() {
		let mut tag = b"mAB \0\0\0\0\x03\x03\0\0".to_vec();
		for offset in [32_u32, 0, 0, 0, 0] {
			tag.extend_from_slice(&offset.to_be_bytes());
		}
		for _ in 0..3 {
			// A parametric square
			tag.extend_from_slice(b"para\0\0\0\0\0\0\0\0");
			tag.extend_from_slice(&(2_i32 << 16).to_be_bytes());
		}
		let lut = Lut::parse(&icc_profile(b"link", b"RGB ", b"RGB ", &tag)).unwrap();
		assert_close(lut.apply([0.5, 0.5, 0.5]), [0.25, 0.25, 0.25]);
	}

	#[test]
	fn parametric_curves_clip_to_the_unit_range() {
		let mut tag = b"mAB \0\0\0\0\x03\x03\0\0".to_vec();
		for offset in [32_u32, 0, 0, 0, 0] {
			tag.extend_from_slice(&offset.to_be_bytes());
		}
		for _ in 0..3 {
			// Function 2 with unit gain and a 0.5 offset: y = x + 0.5
			tag.extend_from_slice(b"para\0\0\0\0\0\x02\0\0");
			for parameter in [1_i32 << 16, 1 << 16, 0, 1 << 15] {
				tag.extend_from_slice(&parameter.to_be_bytes());
			}
		}
		let lut = Lut::parse(&icc_profile(b"link", b"RGB ", b"RGB ", &tag)).unwrap();
		assert_close(lut.apply([0.75, 0.75, 0.75]), [1., 1., 1.]);
	}
}
