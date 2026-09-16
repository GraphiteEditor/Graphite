//! The Iridas/Adobe CUBE format: keyword lines, then one entry per line with red varying fastest.
//!
//! <https://web.archive.org/web/20220215173646/https://wwwimages2.adobe.com/content/dam/acom/en/products/speedgrade/cc/pdfs/cube-lut-specification-1.0.pdf>

use super::{Lut, LutTable, parse_numbers, parse_triple};

pub(super) fn parse(text: &str) -> Option<Lut> {
	let mut size_1d = None;
	let mut size_3d = None;
	let mut domain_min = [0.; 3];
	let mut domain_max = [1.; 3];
	let mut entries = Vec::new();

	for raw_line in text.lines() {
		let line = raw_line.split('#').next().unwrap_or("").trim();
		if line.is_empty() {
			continue;
		}
		let (keyword, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
		match keyword.to_ascii_uppercase().as_str() {
			"TITLE" => {}
			"LUT_1D_SIZE" => size_1d = Some(rest.trim().parse::<usize>().ok()?),
			"LUT_3D_SIZE" => size_3d = Some(rest.trim().parse::<usize>().ok()?),
			"DOMAIN_MIN" => domain_min = parse_triple(&parse_numbers(rest)?)?,
			"DOMAIN_MAX" => domain_max = parse_triple(&parse_numbers(rest)?)?,
			"LUT_1D_INPUT_RANGE" | "LUT_3D_INPUT_RANGE" => {
				let range = parse_numbers(rest)?;
				if range.len() != 2 {
					return None;
				}
				domain_min = [range[0]; 3];
				domain_max = [range[1]; 3];
			}
			_ => entries.push(parse_triple(&parse_numbers(line)?)?),
		}
	}

	let table = match (size_3d, size_1d) {
		(Some(size), _) if size >= 2 && entries.len() == size * size * size => LutTable::ThreeDimensional {
			size: [size; 3],
			red_fastest: true,
			entries,
		},
		(None, Some(size)) if size >= 2 && entries.len() == size => LutTable::OneDimensional { entries },
		_ => return None,
	};
	Some(Lut {
		domain_min,
		domain_max,
		..Lut::new(Some(table))
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::color_lookup_table::tests::assert_close;

	#[test]
	fn identity_and_swap() {
		let mut text = String::from("TITLE \"swap\"\nLUT_3D_SIZE 2\n");
		for b in 0..2 {
			for g in 0..2 {
				for r in 0..2 {
					// Swaps red and blue
					text += &format!("{b} {g} {r}\n");
				}
			}
		}
		let lut = Lut::parse(text.as_bytes()).unwrap();
		assert_close(lut.apply([0.25, 0.5, 0.75]), [0.75, 0.5, 0.25]);

		let one_dimensional = "LUT_1D_SIZE 3\n0 0 0\n0.25 0.5 0.75\n1 1 1\n";
		let lut = Lut::parse(one_dimensional.as_bytes()).unwrap();
		assert_close(lut.apply([0.25, 0.25, 0.25]), [0.125, 0.25, 0.375]);
	}
}
