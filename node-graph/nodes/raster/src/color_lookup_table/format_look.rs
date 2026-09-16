//! The Iridas/Adobe LOOK format: XML whose `<LUT>` element bakes the look into a 3D table of hex-encoded little-endian floats with red varying fastest.

use super::{Lut, LutTable, parse_triple};

pub(super) fn parse(text: &str) -> Option<Lut> {
	let lut = xml_element_body(text, "LUT")?;
	let size: usize = xml_element_body(lut, "size")?.trim().trim_matches('"').parse().ok()?;
	let hex: Vec<u8> = xml_element_body(lut, "data")?.bytes().filter(|byte| !byte.is_ascii_whitespace() && *byte != b'"').collect();

	let floats: Vec<f32> = hex
		.chunks(8)
		.map(|word| {
			let word = std::str::from_utf8(word).ok().filter(|word| word.len() == 8)?;
			Some(f32::from_bits(u32::from_str_radix(word, 16).ok()?.swap_bytes()))
		})
		.collect::<Option<_>>()?;
	let entries: Vec<[f32; 3]> = floats.chunks(3).map(parse_triple).collect::<Option<_>>()?;
	if size < 2 || entries.len() != size * size * size {
		return None;
	}

	Some(Lut::new(Some(LutTable::ThreeDimensional {
		size: [size; 3],
		red_fastest: true,
		entries,
	})))
}

/// The text between an element's tags, enough for the attribute-free markup of a look file.
fn xml_element_body<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
	let open = format!("<{tag}>");
	let close = format!("</{tag}>");
	let start = text.find(&open)? + open.len();
	let end = start + text[start..].find(&close)?;
	Some(&text[start..end])
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::color_lookup_table::tests::assert_close;

	#[test]
	fn decodes_hex_floats_red_fastest() {
		let mut hex = String::new();
		for b in 0..2 {
			for g in 0..2 {
				for r in 0..2 {
					// Swaps red and blue
					for value in [b as f32, g as f32, r as f32] {
						hex.extend(value.to_le_bytes().iter().map(|byte| format!("{byte:02X}")));
					}
				}
			}
		}
		let text = format!(
			"<?xml version=\"1.0\" ?>\n<look>\n  <shaders>\n  </shaders>\n  <LUT>\n    <size>\"2\"</size>\n    <data>\"\n      {hex}\n    \"</data>\n  </LUT>\n  <LUT1D>\n    <size>\"2\"</size>\n    <data>\"00\"</data>\n  </LUT1D>\n</look>\n"
		);
		let lut = Lut::parse(text.as_bytes()).unwrap();
		assert_close(lut.apply([0.25, 0.5, 0.75]), [0.75, 0.5, 0.25]);
	}
}
