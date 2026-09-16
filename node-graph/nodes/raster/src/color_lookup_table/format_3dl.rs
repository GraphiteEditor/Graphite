//! The Flame/Lustre 3DL format: integer entries with blue varying fastest, an input sample line that may double as a shaper, and tokens to ignore.
//!
//! <https://download.autodesk.com/us/systemdocs/help/2009/flame/files/WScba3ee2b36d8cb6f6dc202441162be3fa98-7ff8.htm>

use super::{Curve, Lut, LutTable, parse_numbers};

pub(super) fn parse(text: &str) -> Option<Lut> {
	let mut samples: Option<Vec<f32>> = None;
	let mut output_bits = None;
	let mut entries: Vec<[f32; 3]> = Vec::new();

	for raw_line in text.lines() {
		let line = raw_line.split('#').next().unwrap_or("").trim();
		if let Some(rest) = line.to_ascii_uppercase().strip_prefix("MESH") {
			output_bits = parse_numbers(rest).and_then(|bits| bits.get(1).map(|&bits| bits as u32));
			continue;
		}
		// Any other line that is not numbers is a token some writer needed
		let Some(values) = parse_numbers(line) else { continue };
		if values.len() == 3 {
			entries.push([values[0], values[1], values[2]]);
		} else if values.len() > 3 && samples.is_none() {
			samples = Some(values);
		} else if !values.is_empty() {
			return None;
		}
	}

	let size = (entries.len() as f64).cbrt().round() as usize;
	if size < 2 || size * size * size != entries.len() {
		return None;
	}

	// Entries are integers on the output bit depth's scale, inferred from the largest value when no header names it
	let largest = entries.iter().flatten().fold(0_f32, |largest, &value| largest.max(value));
	let scale = match output_bits {
		Some(bits) => ((1_u64 << bits) - 1) as f32,
		None => likely_bit_depth_scale(largest)?,
	};
	for entry in &mut entries {
		for value in entry {
			*value /= scale;
		}
	}

	// A sample line that is not a uniform ramp is a shaper applied before the cube
	let mut input_curves = None;
	if let Some(samples) = samples {
		let scale = likely_bit_depth_scale(samples.iter().fold(0_f32, |largest, &value| largest.max(value)))?;
		let step = scale / (samples.len() - 1) as f32;
		if samples.iter().enumerate().any(|(index, &value)| (index as f32 * step - value).abs() >= 2.) {
			let curve = Curve::Sampled(samples.iter().map(|&value| value / scale).collect());
			input_curves = Some([curve.clone(), curve.clone(), curve]);
		}
	}

	Some(Lut {
		input_curves,
		..Lut::new(Some(LutTable::ThreeDimensional {
			size: [size; 3],
			red_fastest: false,
			entries,
		}))
	})
}

/// The scale of a 3DL file's integers from their largest value: the first of 8, 10, and 12 bits they overshoot by less than twice, else 16, and none below 128.
fn likely_bit_depth_scale(largest: f32) -> Option<f32> {
	if largest < 128. {
		return None;
	}
	let bits = [8, 10, 12].into_iter().find(|&bits| largest <= (2_u32.pow(bits) * 2 - 1) as f32).unwrap_or(16);
	Some((2_u32.pow(bits) - 1) as f32)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::color_lookup_table::tests::assert_close;

	#[test]
	fn orders_blue_fastest() {
		let mut text = String::from("0 341 682 1023\n");
		for r in 0..4 {
			for g in 0..4 {
				for b in 0..4 {
					// Keeps red and green, inverts blue
					text += &format!("{} {} {}\n", r * 4095 / 3, g * 4095 / 3, (3 - b) * 4095 / 3);
				}
			}
		}
		let lut = Lut::parse(text.as_bytes()).unwrap();
		assert_close(lut.apply([0.5, 1., 0.]), [0.5, 1., 1.]);
	}

	#[test]
	fn flame_dialect_skips_its_tokens() {
		let mut text = String::from("#Tokens required by applications - do not edit\r\n\r\n3DMESH\r\nMesh 2 12\r\n0 256 512 768 1023\r\n\r\n");
		for r in 0..2 {
			for g in 0..2 {
				for b in 0..2 {
					text += &format!("{} {} {}\r\n", r * 4095, g * 4095, b * 4095);
				}
			}
		}
		text += "\r\nLUT8\r\ngamma 1.0\r\n";
		let lut = Lut::parse(text.as_bytes()).unwrap();
		assert_close(lut.apply([1., 0., 0.5]), [1., 0., 0.5]);
	}

	#[test]
	fn sizes_the_cube_from_its_entries() {
		let mut text = String::from("0 256 512 768 1023\n");
		for r in 0..2 {
			for g in 0..2 {
				for b in 0..2 {
					text += &format!("{} {} {}\n", r * 4095, g * 4095, b * 4095);
				}
			}
		}
		let lut = Lut::parse(text.as_bytes()).unwrap();
		assert_close(lut.apply([0.25, 0.5, 0.75]), [0.25, 0.5, 0.75]);
	}

	#[test]
	fn applies_a_non_uniform_sample_line_as_a_shaper() {
		let mut text = String::from("0 0 0 1023\n");
		for r in 0..2 {
			for g in 0..2 {
				for b in 0..2 {
					text += &format!("{} {} {}\n", r * 4095, g * 4095, b * 4095);
				}
			}
		}
		let lut = Lut::parse(text.as_bytes()).unwrap();
		assert_close(lut.apply([0.5, 0.5, 0.5]), [0., 0., 0.]);
		assert_close(lut.apply([1., 1., 1.]), [1., 1., 1.]);
	}

	#[test]
	fn infers_the_output_depth_allowing_overshoot() {
		let mut text = String::new();
		for r in 0..2 {
			for g in 0..2 {
				for b in 0..2 {
					text += &format!("{} {} {}\n", r * 1100, g * 1100, b * 1100);
				}
			}
		}
		let lut = Lut::parse(text.as_bytes()).unwrap();
		assert_close(lut.apply([1., 1., 1.]), [1100. / 1023.; 3]);
	}

	#[test]
	fn rejects_implausibly_small_values() {
		assert!(Lut::parse(b"0 0 0\n0 0 1\n0 1 0\n0 1 1\n1 0 0\n1 0 1\n1 1 0\n1 1 1\n").is_err());
	}
}
