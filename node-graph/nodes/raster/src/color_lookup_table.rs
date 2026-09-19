//! Lookup tables for the Color Lookup adjustment: parsing CUBE, 3DL, CSP, and LOOK files and ICC profiles, and evaluating them.

mod format_3dl;
mod format_csp;
mod format_cube;
mod format_icc;
mod format_look;

use crate::adjustments::{SRGB_TO_XYZ_D50, WHITE_XYZ_D50, XYZ_D50_TO_SRGB, multiply_matrix};
use graphene_resource::{Resource, ResourceHash};
use no_std_types::color::{linear_to_srgb, srgb_to_linear};
use std::sync::{Arc, Mutex, PoisonError};

pub const LUT_FILE_EXTENSIONS: &[&str] = &["cube", "3dl", "look", "csp", "icc", "icm"];

/// A file's parse result, in the form the cache shares out.
type ParsedLut = Result<Arc<Lut>, LutParseError>;

/// The last file a node parsed, kept as the node's own data so the file is not parsed anew each time the node runs again.
#[derive(Debug, Clone, Default)]
pub struct LutCache(Arc<Mutex<Option<(ResourceHash, ParsedLut)>>>);

impl LutCache {
	/// [`Lut::parse`] for a resource, reusing the last result while the file's content hash stays the same.
	pub fn parse(&self, resource: &Resource) -> ParsedLut {
		// A lock poisoned by a panic elsewhere still guards a usable cache
		let mut cached = self.0.lock().unwrap_or_else(PoisonError::into_inner);

		let hash = resource.hash();
		if let Some((cached_hash, result)) = cached.as_ref()
			&& *cached_hash == hash
		{
			return result.clone();
		}

		let result = Lut::parse(resource).map(Arc::new);
		*cached = Some((hash, result.clone()));
		result
	}
}

/// Why a file could not be read as a lookup table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LutParseError {
	/// An ICC profile that describes a device's colors, not an "abstract" or "device link" profile that remaps them.
	IccProfileClass,
	/// An ICC "abstract" profile that does not map Lab to Lab, or a "device link" profile that does not map RGB to RGB.
	IccColorSpaces,
	/// Not a lookup table in any of the formats, or one that is damaged.
	Unreadable,
}

/// A per-channel mapping applied around the table.
#[derive(Debug, Clone, PartialEq)]
pub enum Curve {
	/// Output samples at evenly spaced inputs across 0..1.
	Sampled(Vec<f32>),
	/// Output samples at the given inputs (the CSP format's pre-LUT).
	Piecewise {
		inputs: Vec<f32>,
		outputs: Vec<f32>,
	},
	Power(f32),
	/// One of the ICC parametric curve functions, with its parameters in the specification's order.
	Parametric {
		function: u16,
		parameters: Vec<f32>,
	},
}

impl Curve {
	fn apply(&self, value: f32) -> f32 {
		match self {
			Curve::Sampled(samples) => {
				let last = samples.len().saturating_sub(1);
				if last == 0 {
					return samples.first().copied().unwrap_or(value);
				}
				let scaled = value.clamp(0., 1.) * last as f32;
				let lower = (scaled.floor() as usize).min(last);
				let upper = (lower + 1).min(last);
				samples[lower] + (scaled - lower as f32) * (samples[upper] - samples[lower])
			}
			Curve::Piecewise { inputs, outputs } => {
				let count = inputs.len();
				if count == 0 {
					return value;
				}
				if value <= inputs[0] {
					return outputs[0];
				}
				if value >= inputs[count - 1] {
					return outputs[count - 1];
				}

				let upper = inputs.partition_point(|&input| input <= value).min(count - 1);
				let lower = upper - 1;
				let t = (value - inputs[lower]) / (inputs[upper] - inputs[lower]).max(f32::EPSILON);
				outputs[lower] + t * (outputs[upper] - outputs[lower])
			}
			Curve::Power(gamma) => value.max(0.).powf(*gamma),
			Curve::Parametric { function, parameters } => parametric_curve(*function, parameters, value),
		}
	}
}

/// The ICC `parametricCurveType` functions 0 through 4, which share the form `(a * x + b)^g` above a breakpoint.
fn parametric_curve(function: u16, parameters: &[f32], x: f32) -> f32 {
	let at = |index: usize| parameters.get(index).copied().unwrap_or(0.);
	let (g, a, b, c, d, e, f) = (at(0), at(1), at(2), at(3), at(4), at(5), at(6));
	let power = |x: f32| (a * x + b).max(0.).powf(g);
	let result = match function {
		0 => x.max(0.).powf(g),
		1 => {
			if x >= -b / a {
				power(x)
			} else {
				0.
			}
		}
		2 => {
			if x >= -b / a {
				power(x) + c
			} else {
				c
			}
		}
		3 => {
			if x >= d {
				power(x)
			} else {
				c * x
			}
		}
		4 => {
			if x >= d {
				power(x) + e
			} else {
				c * x + f
			}
		}
		_ => x,
	};

	// The specification clips every function to the unit range
	result.clamp(0., 1.)
}

/// A step applied to the table's output.
#[derive(Debug, Clone, PartialEq)]
pub enum Stage {
	Curves([Curve; 3]),
	/// The 3x4 matrix of an ICC `lutAToBType`.
	Matrix {
		matrix: [[f32; 3]; 3],
		offset: [f32; 3],
	},
}

impl Stage {
	fn apply(&self, values: [f32; 3]) -> [f32; 3] {
		match self {
			Stage::Curves(curves) => [curves[0].apply(values[0]), curves[1].apply(values[1]), curves[2].apply(values[2])],
			Stage::Matrix { matrix, offset } => {
				let product = multiply_matrix(matrix, values);
				[product[0] + offset[0], product[1] + offset[1], product[2] + offset[2]]
			}
		}
	}
}

/// How an abstract profile's table encodes CIELAB: the standard scale, or the legacy one of 16-bit tables.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LabEncoding {
	Standard,
	Legacy,
}

impl LabEncoding {
	fn encode(self, [l, a, b]: [f32; 3]) -> [f32; 3] {
		match self {
			LabEncoding::Standard => [l / 100., (a + 128.) / 255., (b + 128.) / 255.],
			LabEncoding::Legacy => [l / 100. * (65280. / 65535.), (a + 128.) * (256. / 65535.), (b + 128.) * (256. / 65535.)],
		}
	}

	fn decode(self, [l, a, b]: [f32; 3]) -> [f32; 3] {
		match self {
			LabEncoding::Standard => [l * 100., a * 255. - 128., b * 255. - 128.],
			LabEncoding::Legacy => [l * 100. * (65535. / 65280.), a * (65535. / 256.) - 128., b * (65535. / 256.) - 128.],
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub enum LutTable {
	/// One table per channel, each channel looked up by its own value.
	OneDimensional { entries: Vec<[f32; 3]> },
	/// A grid of `size` samples per axis addressed by all three channels, stored with either red or blue varying fastest.
	ThreeDimensional { size: [usize; 3], red_fastest: bool, entries: Vec<[f32; 3]> },
}

/// A color lookup table read from a CUBE, 3DL, CSP, or LOOK file or an ICC profile, mapping gamma-encoded RGB to gamma-encoded RGB.
#[derive(Debug, Clone, PartialEq)]
pub struct Lut {
	domain_min: [f32; 3],
	domain_max: [f32; 3],
	input_curves: Option<[Curve; 3]>,
	/// Absent when a profile applies only curves and a matrix.
	table: Option<LutTable>,
	output_stages: Vec<Stage>,
	/// Set when the table maps CIELAB rather than RGB, as an abstract profile's does.
	lab_encoding: Option<LabEncoding>,
}

impl Lut {
	fn new(table: Option<LutTable>) -> Self {
		Lut {
			domain_min: [0.; 3],
			domain_max: [1.; 3],
			input_curves: None,
			table,
			output_stages: Vec::new(),
			lab_encoding: None,
		}
	}

	/// Reads a CUBE, 3DL, CSP, or LOOK text file or an ICC abstract or device link profile.
	pub fn parse(bytes: &[u8]) -> Result<Self, LutParseError> {
		if bytes.get(36..40).is_some_and(|signature| signature == b"acsp") {
			return format_icc::parse(bytes);
		}

		Self::parse_text(bytes).ok_or(LutParseError::Unreadable)
	}

	fn parse_text(bytes: &[u8]) -> Option<Self> {
		let text = String::from_utf8_lossy(bytes);
		let trimmed = text.trim_start_matches('\u{feff}').trim_start();
		if trimmed.starts_with("CSPLUTV100") {
			return format_csp::parse(trimmed);
		}
		if trimmed.starts_with('<') {
			return format_look::parse(&text);
		}
		if text.contains("LUT_3D_SIZE") || text.contains("LUT_1D_SIZE") {
			return format_cube::parse(&text);
		}
		format_3dl::parse(&text)
	}

	pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
		let input = match self.lab_encoding {
			Some(encoding) => encoding.encode(srgb_to_lab(rgb)),
			None => rgb,
		};
		let mut normalized = [0.; 3];
		for channel in 0..3 {
			let value = match &self.input_curves {
				Some(curves) => curves[channel].apply(input[channel]),
				None => input[channel],
			};
			let span = (self.domain_max[channel] - self.domain_min[channel]).max(f32::EPSILON);
			normalized[channel] = ((value - self.domain_min[channel]) / span).clamp(0., 1.);
		}

		let mut result = match &self.table {
			None => normalized,
			Some(LutTable::OneDimensional { entries }) => {
				let last = entries.len() - 1;
				let mut result = [0.; 3];
				for channel in 0..3 {
					let scaled = normalized[channel] * last as f32;
					let lower = (scaled.floor() as usize).min(last);
					let upper = (lower + 1).min(last);
					let t = scaled - lower as f32;
					result[channel] = entries[lower][channel] + t * (entries[upper][channel] - entries[lower][channel]);
				}
				result
			}
			Some(LutTable::ThreeDimensional { size, red_fastest, entries }) => {
				let index = |r: usize, g: usize, b: usize| if *red_fastest { (b * size[1] + g) * size[0] + r } else { (r * size[1] + g) * size[2] + b };
				let sample = |r: usize, g: usize, b: usize| entries[index(r, g, b)];
				let coordinate = |axis: usize| {
					let scaled = normalized[axis] * (size[axis] - 1) as f32;
					let lower = (scaled.floor() as usize).min(size[axis] - 1);
					(lower, (lower + 1).min(size[axis] - 1), scaled - lower as f32)
				};
				let ((r0, r1, fr), (g0, g1, fg), (b0, b1, fb)) = (coordinate(0), coordinate(1), coordinate(2));

				// Tetrahedral interpolation: walk from the cell's origin to its far corner along the axes in decreasing fraction order
				let c000 = sample(r0, g0, b0);
				let c111 = sample(r1, g1, b1);
				let walk = |first: [f32; 3], first_t: f32, second: [f32; 3], second_t: f32, third_t: f32| {
					let mut result = c000;
					for channel in 0..3 {
						result[channel] += first_t * (first[channel] - c000[channel]) + second_t * (second[channel] - first[channel]) + third_t * (c111[channel] - second[channel]);
					}
					result
				};
				if fr >= fg && fg >= fb {
					walk(sample(r1, g0, b0), fr, sample(r1, g1, b0), fg, fb)
				} else if fr >= fb && fb >= fg {
					walk(sample(r1, g0, b0), fr, sample(r1, g0, b1), fb, fg)
				} else if fb >= fr && fr >= fg {
					walk(sample(r0, g0, b1), fb, sample(r1, g0, b1), fr, fg)
				} else if fg >= fr && fr >= fb {
					walk(sample(r0, g1, b0), fg, sample(r1, g1, b0), fr, fb)
				} else if fg >= fb && fb >= fr {
					walk(sample(r0, g1, b0), fg, sample(r0, g1, b1), fb, fr)
				} else {
					walk(sample(r0, g0, b1), fb, sample(r0, g1, b1), fg, fr)
				}
			}
		};

		for stage in &self.output_stages {
			result = stage.apply(result);
		}
		match self.lab_encoding {
			Some(encoding) => lab_to_srgb(encoding.decode(result)),
			None => result,
		}
	}
}

/// Gamma-encoded sRGB to CIELAB against the D50 white of the ICC profile connection space.
fn srgb_to_lab(rgb: [f32; 3]) -> [f32; 3] {
	let xyz = multiply_matrix(&SRGB_TO_XYZ_D50, rgb.map(srgb_to_linear));
	let f = |t: f32| if t > 216. / 24389. { t.cbrt() } else { (24389. / 27. * t + 16.) / 116. };
	let (fx, fy, fz) = (f(xyz[0] / WHITE_XYZ_D50[0]), f(xyz[1] / WHITE_XYZ_D50[1]), f(xyz[2] / WHITE_XYZ_D50[2]));
	[116. * fy - 16., 500. * (fx - fy), 200. * (fy - fz)]
}

fn lab_to_srgb([l, a, b]: [f32; 3]) -> [f32; 3] {
	let fy = (l + 16.) / 116.;
	let (fx, fz) = (fy + a / 500., fy - b / 200.);
	let inverse = |t: f32| if t > 6. / 29. { t * t * t } else { 3. * (6_f32 / 29.).powi(2) * (t - 4. / 29.) };
	let xyz = [inverse(fx) * WHITE_XYZ_D50[0], inverse(fy) * WHITE_XYZ_D50[1], inverse(fz) * WHITE_XYZ_D50[2]];
	multiply_matrix(&XYZ_D50_TO_SRGB, xyz).map(|channel| linear_to_srgb(channel.clamp(0., 1.)))
}

/// Every whitespace-separated token of the line as a number, or `None` if any token is not one.
fn parse_numbers(line: &str) -> Option<Vec<f32>> {
	line.split_whitespace().map(|token| token.parse::<f32>().ok()).collect()
}

fn parse_triple(values: &[f32]) -> Option<[f32; 3]> {
	(values.len() == 3).then(|| [values[0], values[1], values[2]])
}

#[cfg(test)]
mod tests {
	use super::*;

	pub(super) fn assert_close(actual: [f32; 3], expected: [f32; 3]) {
		assert_close_within(actual, expected, 1e-5);
	}

	pub(super) fn assert_close_within(actual: [f32; 3], expected: [f32; 3], tolerance: f32) {
		for channel in 0..3 {
			assert!((actual[channel] - expected[channel]).abs() < tolerance, "{actual:?} vs {expected:?}");
		}
	}

	#[test]
	fn text_outside_utf_8_does_not_reject_a_file() {
		let mut bytes = b"TITLE \"Cr\xe9\xe9 par\"\nLUT_1D_SIZE 2\n".to_vec();
		bytes.extend_from_slice(b"0 0 0\n1 1 1\n");
		let lut = Lut::parse(&bytes).unwrap();
		assert_close(lut.apply([0.25, 0.5, 0.75]), [0.25, 0.5, 0.75]);
	}

	#[test]
	fn rejects_garbage() {
		assert!(Lut::parse(b"").is_err());
		assert!(Lut::parse(b"hello world").is_err());
		assert!(Lut::parse(b"LUT_3D_SIZE 2\n0 0 0\n").is_err());
	}

	#[test]
	fn a_cache_parses_a_file_once_until_the_file_changes() {
		let file = Resource::new(b"LUT_1D_SIZE 2\n0 0 0\n1 1 1\n".to_vec());
		let cache = LutCache::default();
		let first = cache.parse(&file).unwrap();
		assert!(Arc::ptr_eq(&first, &cache.parse(&file).unwrap()));

		// A cloned node shares its cache
		assert!(Arc::ptr_eq(&first, &cache.clone().parse(&file).unwrap()));

		// Another file takes its place, a failed one included
		assert!(cache.parse(&Resource::new(b"hello world".to_vec())).is_err());
		assert!(!Arc::ptr_eq(&first, &cache.parse(&file).unwrap()));
	}
}
