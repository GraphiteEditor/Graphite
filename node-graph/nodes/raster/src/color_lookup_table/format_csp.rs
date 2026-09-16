//! The cineSpace CSP format: a version line, the dimensionality, optional metadata, three pre-LUTs, the size line, then the entries with red varying fastest.
//!
//! <https://aswf-openrv.readthedocs.io/en/latest/rv-manuals/rv-user-manual/rv-user-manual-chapter-g.html>

use super::{Curve, Lut, LutTable, parse_numbers, parse_triple};

pub(super) fn parse(text: &str) -> Option<Lut> {
	let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
	lines.next().filter(|line| line.starts_with("CSPLUTV100"))?;
	let three_dimensional = match lines.next()?.to_ascii_uppercase().as_str() {
		"3D" => true,
		"1D" => false,
		_ => return None,
	};

	let mut line = lines.next()?;
	if line.eq_ignore_ascii_case("BEGIN METADATA") {
		loop {
			line = lines.next()?;
			if line.eq_ignore_ascii_case("END METADATA") {
				line = lines.next()?;
				break;
			}
		}
	}

	let mut curves = Vec::new();
	for channel in 0..3 {
		let count_line = if channel == 0 { line } else { lines.next()? };
		let count: usize = count_line.parse().ok()?;
		// Fewer than two points is an identity pre-LUT with no data lines
		if count < 2 {
			curves.push(Curve::Piecewise {
				inputs: vec![0., 1.],
				outputs: vec![0., 1.],
			});
			continue;
		}
		let inputs = parse_numbers(lines.next()?)?;
		let outputs = parse_numbers(lines.next()?)?;
		if inputs.len() != count || outputs.len() != count {
			return None;
		}
		curves.push(Curve::Piecewise { inputs, outputs });
	}
	let curves: [Curve; 3] = curves.try_into().ok()?;

	let sizes = parse_numbers(lines.next()?)?;
	let table = if three_dimensional {
		let size = [*sizes.first()? as usize, *sizes.get(1)? as usize, *sizes.get(2)? as usize];
		let entries: Vec<[f32; 3]> = lines.take(size[0] * size[1] * size[2]).map(|line| parse_triple(&parse_numbers(line)?)).collect::<Option<_>>()?;
		if size.iter().any(|&axis| axis < 2) || entries.len() != size[0] * size[1] * size[2] {
			return None;
		}
		LutTable::ThreeDimensional { size, red_fastest: true, entries }
	} else {
		let size = *sizes.first()? as usize;
		let entries: Vec<[f32; 3]> = lines.take(size).map(|line| parse_triple(&parse_numbers(line)?)).collect::<Option<_>>()?;
		if size < 2 || entries.len() != size {
			return None;
		}
		LutTable::OneDimensional { entries }
	};

	Some(Lut {
		input_curves: Some(curves),
		..Lut::new(Some(table))
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::color_lookup_table::tests::assert_close;

	#[test]
	fn pre_lut_scales_inputs() {
		let text = "CSPLUTV100\n1D\nBEGIN METADATA\nsomething\nEND METADATA\n2\n0 2\n0 1\n2\n0 2\n0 1\n2\n0 2\n0 1\n2\n0 0 0\n1 1 1\n";
		let lut = Lut::parse(text.as_bytes()).unwrap();
		assert_close(lut.apply([1., 0.5, 2.]), [0.5, 0.25, 1.]);
	}

	#[test]
	fn pre_lut_under_two_points_is_identity() {
		let text = "CSPLUTV100\n1D\n0\n1\n0\n2\n0 0 0\n1 1 1\n";
		let lut = Lut::parse(text.as_bytes()).unwrap();
		assert_close(lut.apply([0.25, 0.5, 0.75]), [0.25, 0.5, 0.75]);
	}
}
