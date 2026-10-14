//! Lookup tables for the Color Lookup adjustment: parsing CUBE, 3DL, CSP, and LOOK files and ICC profiles, and evaluating them.

mod format_3dl;
mod format_csp;
mod format_cube;
mod format_icc;
mod format_look;

use crate::adjustment_steps::{split_count, texel};
use crate::adjustments::{SRGB_TO_XYZ_D50, WHITE_XYZ_D50, XYZ_D50_TO_SRGB, multiply_matrix};
use core_types::color::{linear_to_srgb, srgb_to_linear};
use graphene_resource::{Resource, ResourceHash, WeakResource};
use std::iter::once;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

pub const LUT_FILE_EXTENSIONS: &[&str] = &["cube", "3dl", "look", "csp", "icc", "icm"];

/// A file's parse result, in the form the cache shares out.
type ParsedLut = Result<Arc<Lut>, LutParseError>;

/// A parse result, kept while the file it came from is still loaded somewhere.
struct ParsedFile {
	hash: ResourceHash,
	file: WeakResource,
	parsed: ParsedLut,
}

/// The parse results of the files that are still loaded, shared by every adjustment that reads a lookup table.
static PARSED_FILES: Mutex<Vec<ParsedFile>> = Mutex::new(Vec::new());

fn parsed_files() -> MutexGuard<'static, Vec<ParsedFile>> {
	// A lock poisoned by a panic elsewhere still guards a usable cache
	PARSED_FILES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// [`Lut::parse`] for a resource, reusing the result for as long as a file with the same content hash stays loaded. O(n) in the loaded files.
pub fn parse_cached(resource: &Resource) -> ParsedLut {
	let hash = resource.hash();

	{
		let mut parsed_files = parsed_files();

		// Follows the copy in hand before pruning, in case another copy of the same content is the one that went away
		let cached = parsed_files.iter_mut().find(|entry| entry.hash == hash).map(|entry| {
			entry.file = resource.downgrade();
			entry.parsed.clone()
		});
		parsed_files.retain(|entry| entry.file.is_alive());

		if let Some(parsed) = cached {
			return parsed;
		}
	}

	// Parsed outside the lock, so a large file doesn't hold up the adjustments reading other files
	let parsed = Lut::parse(resource).map(Arc::new);

	let mut parsed_files = parsed_files();
	if !parsed_files.iter().any(|entry| entry.hash == hash) {
		parsed_files.push(ParsedFile {
			hash,
			file: resource.downgrade(),
			parsed: parsed.clone(),
		});
	}
	parsed
}

/// Why a file could not be read as a lookup table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LutParseError {
	/// An ICC profile that describes a device's colors, not an "abstract" or "device link" profile that remaps them.
	#[error("The ICC profile describes a device's colors, but only abstract and device link profiles work as LUTs")]
	IccProfileClass,
	/// An ICC "abstract" profile that does not map Lab to Lab, or a "device link" profile that does not map RGB to RGB.
	#[error("The ICC profile's color spaces are unsupported, as a device link profile must map RGB to RGB and an abstract profile Lab to Lab")]
	IccColorSpaces,
	/// Not a lookup table in any of the formats, or one that is damaged.
	#[error("The file is not a LUT in a supported format, or it is damaged")]
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

				// A NaN value leaves the search without a segment, so the clamp keeps it in bounds
				let upper = inputs.partition_point(|&input| input <= value).clamp(1, count - 1);
				let lower = upper - 1;
				let t = (value - inputs[lower]) / (inputs[upper] - inputs[lower]).max(f32::EPSILON);
				outputs[lower] + t * (outputs[upper] - outputs[lower])
			}
			Curve::Power(gamma) => value.max(0.).powf(*gamma),
			Curve::Parametric { function, parameters } => parametric_curve(*function, parameters, value),
		}
	}

	/// The curve as a block for the GPU's adjustment shader, holding its samples or parameters.
	fn texels(&self) -> Vec<[f32; 4]> {
		match self {
			Curve::Sampled(samples) => block(SAMPLED_CURVE, 0., samples.iter().map(|&sample| texel(&[sample]))),
			Curve::Piecewise { inputs, outputs } => block(PIECEWISE_CURVE, 0., inputs.iter().zip(outputs).map(|(&input, &output)| texel(&[input, output]))),
			Curve::Power(gamma) => block(POWER_CURVE, *gamma, []),
			Curve::Parametric { function, parameters } => {
				// Absent parameters read as zero, as the CPU reads them
				let parameter = |index: usize| parameters.get(index).copied().unwrap_or(0.);
				let [g, a, b, c, d, e, f] = std::array::from_fn(parameter);
				block(PARAMETRIC_CURVE, *function as f32, [[g, a, b, c], texel(&[d, e, f])])
			}
		}
	}

	/// How many texels [`Self::texels`] gives, counted without making them.
	fn texel_count(&self) -> usize {
		let body = match self {
			Curve::Sampled(samples) => samples.len(),
			Curve::Piecewise { inputs, outputs } => inputs.len().min(outputs.len()),
			Curve::Power(_) => 0,
			Curve::Parametric { .. } => 2,
		};
		1 + body
	}
}

/// The kinds of the blocks the GPU's adjustment shader reads a table from, numbered together since one loop of the shader carries out every block.
const SAMPLED_CURVE: u32 = 0;
const PIECEWISE_CURVE: u32 = 1;
const POWER_CURVE: u32 = 2;
const PARAMETRIC_CURVE: u32 = 3;
const NO_TABLE: u32 = 4;
const ONE_DIMENSIONAL_TABLE: u32 = 5;
const THREE_DIMENSIONAL_TABLE: u32 = 6;
const MATRIX_STAGE: u32 = 7;

/// The codes the shader knows the Lab encodings by.
const LAB_NONE: u32 = 0;
const LAB_STANDARD: u32 = 1;
const LAB_LEGACY: u32 = 2;

/// How many entries per axis a table too large for the GPU's texture is resampled to.
const RESAMPLED_TABLE_SIZE: usize = 65;

/// A block of texels for the GPU's adjustment shader: a header holding the block's kind, a setting, and how many texels it spans (in halves), then its `body`.
fn block(kind: u32, setting: f32, body: impl IntoIterator<Item = [f32; 4]>) -> Vec<[f32; 4]> {
	let mut texels = vec![[0.; 4]];
	texels.extend(body);

	let [low, high] = split_count(texels.len());
	texels[0] = [kind as f32, setting, low, high];
	texels
}

/// The names the shader gives the codes of the table's blocks and Lab encodings.
pub(crate) fn shader_constants() -> Vec<(&'static str, u32)> {
	vec![
		("SAMPLED_CURVE", SAMPLED_CURVE),
		("PIECEWISE_CURVE", PIECEWISE_CURVE),
		("POWER_CURVE", POWER_CURVE),
		("PARAMETRIC_CURVE", PARAMETRIC_CURVE),
		("NO_TABLE", NO_TABLE),
		("ONE_DIMENSIONAL_TABLE", ONE_DIMENSIONAL_TABLE),
		("THREE_DIMENSIONAL_TABLE", THREE_DIMENSIONAL_TABLE),
		("MATRIX_STAGE", MATRIX_STAGE),
		("LAB_NONE", LAB_NONE),
		("LAB_STANDARD", LAB_STANDARD),
	]
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

		Self::parse_text(bytes).filter(Self::is_finite).ok_or(LutParseError::Unreadable)
	}

	/// O(n): whether every number in the table is finite.
	fn is_finite(&self) -> bool {
		let all_finite = |values: &[f32]| values.iter().all(|value| value.is_finite());
		let curve_is_finite = |curve: &Curve| match curve {
			Curve::Sampled(samples) => all_finite(samples),
			Curve::Piecewise { inputs, outputs } => all_finite(inputs) && all_finite(outputs),
			Curve::Power(gamma) => gamma.is_finite(),
			Curve::Parametric { parameters, .. } => all_finite(parameters),
		};
		let stage_is_finite = |stage: &Stage| match stage {
			Stage::Curves(curves) => curves.iter().all(curve_is_finite),
			Stage::Matrix { matrix, offset } => matrix.iter().all(|row| all_finite(row)) && all_finite(offset),
		};
		let table_is_finite = match &self.table {
			Some(LutTable::OneDimensional { entries } | LutTable::ThreeDimensional { entries, .. }) => entries.iter().all(|entry| all_finite(entry)),
			None => true,
		};

		all_finite(&self.domain_min) && all_finite(&self.domain_max) && self.input_curves.iter().flatten().all(curve_is_finite) && table_is_finite && self.output_stages.iter().all(stage_is_finite)
	}

	fn parse_text(bytes: &[u8]) -> Option<Self> {
		let text = String::from_utf8_lossy(bytes);
		let text = text.trim_start_matches('\u{feff}').trim_start();

		if text.starts_with("CSPLUTV100") {
			return format_csp::parse(text);
		}
		if text.starts_with('<') {
			return format_look::parse(text);
		}
		if text.contains("LUT_3D_SIZE") || text.contains("LUT_1D_SIZE") {
			return format_cube::parse(text);
		}
		format_3dl::parse(text)
	}

	pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
		let result = self.map_normalized(self.normalized(rgb));

		match self.lab_encoding {
			Some(encoding) => lab_to_srgb(encoding.decode(result)),
			None => result,
		}
	}

	/// Where a color lands within the table's domain, from 0 to 1 along each axis, once encoded and passed through any input curves.
	fn normalized(&self, rgb: [f32; 3]) -> [f32; 3] {
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
		normalized
	}

	/// The table and the output stages after it, mapping a color `normalized` within the table's domain, still encoded if the table maps CIELAB.
	fn map_normalized(&self, normalized: [f32; 3]) -> [f32; 3] {
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
		result
	}

	/// The table as the data the GPU's adjustment shader reads and evaluates as [`Self::apply`] does, from no more than `max_texels` texels.
	/// A table too large for that is first resampled (with its output stages) onto a grid of [`RESAMPLED_TABLE_SIZE`] entries per axis of its own domain.
	pub(crate) fn texels_within(&self, max_texels: usize) -> Vec<[f32; 4]> {
		if self.texel_count() <= max_texels {
			return self.texels();
		}

		self.resampled(RESAMPLED_TABLE_SIZE).texels()
	}

	/// The table's Lab encoding and how many blocks follow, then its domain's bounds, then its blocks in the order they map a color:
	/// any input curves (one for each channel), the table, and each output stage (three curves again, or a matrix with each row's offset).
	fn texels(&self) -> Vec<[f32; 4]> {
		let lab_encoding = match self.lab_encoding {
			None => LAB_NONE,
			Some(LabEncoding::Standard) => LAB_STANDARD,
			Some(LabEncoding::Legacy) => LAB_LEGACY,
		};
		let stage_blocks = |stage: &Stage| if matches!(stage, Stage::Curves(_)) { 3 } else { 1 };
		let block_count = self.input_curves.iter().flatten().count() + 1 + self.output_stages.iter().map(stage_blocks).sum::<usize>();

		let mut texels = Vec::with_capacity(self.texel_count());
		texels.extend([texel(&[lab_encoding as f32, block_count as f32]), texel(&self.domain_min), texel(&self.domain_max)]);
		texels.extend(self.input_curves.iter().flatten().flat_map(Curve::texels));

		let entry_texels = |entries: &[[f32; 3]]| entries.iter().map(|entry| texel(entry)).collect::<Vec<_>>();
		texels.extend(match &self.table {
			None => block(NO_TABLE, 0., []),
			Some(LutTable::OneDimensional { entries }) => block(ONE_DIMENSIONAL_TABLE, 0., entry_texels(entries)),
			Some(LutTable::ThreeDimensional { size, red_fastest, entries }) => {
				let size = texel(&size.map(|axis| axis as f32));
				block(THREE_DIMENSIONAL_TABLE, f32::from(*red_fastest), once(size).chain(entry_texels(entries)))
			}
		});

		for stage in &self.output_stages {
			match stage {
				Stage::Curves(curves) => texels.extend(curves.iter().flat_map(Curve::texels)),
				Stage::Matrix { matrix, offset } => texels.extend(block(MATRIX_STAGE, 0., (0..3).map(|row| [matrix[row][0], matrix[row][1], matrix[row][2], offset[row]]))),
			}
		}

		texels
	}

	/// How many texels [`Self::texels`] gives, counted without making them.
	fn texel_count(&self) -> usize {
		let curves = |curves: &[Curve; 3]| curves.iter().map(Curve::texel_count).sum::<usize>();
		let table = match &self.table {
			None => 1,
			Some(LutTable::OneDimensional { entries }) => 1 + entries.len(),
			Some(LutTable::ThreeDimensional { entries, .. }) => 2 + entries.len(),
		};
		let stages = (self.output_stages.iter())
			.map(|stage| match stage {
				Stage::Curves(stage_curves) => curves(stage_curves),
				Stage::Matrix { .. } => 4,
			})
			.sum::<usize>();

		3 + self.input_curves.as_ref().map_or(0, curves) + table + stages
	}

	/// O(size³): the table and its output stages as one plain three-dimensional table sampling them at `size` evenly spaced entries per axis of the domain,
	/// which colors reach as they reach this one, by the same encoding, input curves, and domain.
	fn resampled(&self, size: usize) -> Self {
		let last = (size - 1).max(1) as f32;
		let entries = (0..size * size * size)
			.map(|index| {
				let [r, g, b] = [index % size, index / size % size, index / (size * size)];
				self.map_normalized([r as f32 / last, g as f32 / last, b as f32 / last])
			})
			.collect();

		Lut {
			domain_min: self.domain_min,
			domain_max: self.domain_max,
			input_curves: self.input_curves.clone(),
			table: Some(LutTable::ThreeDimensional {
				size: [size; 3],
				red_fastest: true,
				entries,
			}),
			output_stages: Vec::new(),
			lab_encoding: self.lab_encoding,
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
	multiply_matrix(&XYZ_D50_TO_SRGB, xyz).map(|channel| linear_to_srgb(channel.clamp(0., 1.)).min(1.))
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
	fn a_byte_order_mark_is_skipped() {
		let lut = Lut::parse("\u{feff}LUT_1D_SIZE 2\n0 0 0\n1 1 1\n".as_bytes()).unwrap();
		assert_close(lut.apply([0.25, 0.5, 0.75]), [0.25, 0.5, 0.75]);
	}

	#[test]
	fn rejects_garbage() {
		assert!(Lut::parse(b"").is_err());
		assert!(Lut::parse(b"hello world").is_err());
		assert!(Lut::parse(b"LUT_3D_SIZE 2\n0 0 0\n").is_err());
	}

	#[test]
	fn rejects_a_number_that_is_not_finite() {
		// An entry and a domain bound of a text file
		assert!(Lut::parse(b"LUT_1D_SIZE 2\n0 0 0\nnan 1 1\n").is_err());
		assert!(Lut::parse(b"LUT_1D_SIZE 2\nDOMAIN_MAX inf inf inf\n0 0 0\n1 1 1\n").is_err());

		// Infinity among the hex floats of a look, which otherwise reads fine
		let look = |data: String| format!("<LUT><size>2</size><data>{data}</data></LUT>");
		assert!(Lut::parse(look("00000000".repeat(24)).as_bytes()).is_ok());
		assert!(Lut::parse(look("0000807F".to_string() + &"00000000".repeat(23)).as_bytes()).is_err());
	}

	#[test]
	fn the_cache_parses_a_file_once_while_it_stays_loaded() {
		let file = Resource::new(b"LUT_1D_SIZE 2\n0 0 0\n0.5 1 1\n".to_vec());
		let first = parse_cached(&file).unwrap();

		// Any number of other files, failed ones included, leave it in place
		let others: Vec<Resource> = (0..20).map(|index| Resource::new(format!("not a table {index}").into_bytes())).collect();
		others.iter().for_each(|other| assert!(parse_cached(other).is_err()));
		assert!(Arc::ptr_eq(&first, &parse_cached(&file).unwrap()));

		// Once no copy of it is loaded, parsing another file clears it out
		let content = file.to_vec();
		drop(file);
		assert!(parse_cached(&Resource::new(b"not a table either".to_vec())).is_err());
		assert!(!Arc::ptr_eq(&first, &parse_cached(&Resource::new(content)).unwrap()));
	}

	#[test]
	fn a_table_too_large_for_the_gpu_is_resampled_to_fit() {
		let mut cube = String::from("LUT_3D_SIZE 9\n");
		for index in 0..9 * 9 * 9 {
			let [r, g, b] = [index % 9, index / 9 % 9, index / 81].map(|step| step as f32 / 8.);
			cube += &format!("{} {} {}\n", r * r, (g + b) / 2., 1. - b);
		}
		let lut = Lut::parse(cube.as_bytes()).unwrap();

		// The settings, the domain's bounds, and the table's block with its size, then its entries
		assert_eq!(lut.texels_within(usize::MAX).len(), 3 + 2 + 9 * 9 * 9);
		assert_eq!(lut.texels_within(9 * 9 * 9).len(), 3 + 2 + RESAMPLED_TABLE_SIZE.pow(3));

		// The grid it's resampled onto includes the original's, and interpolates between them along the same tetrahedra
		let resampled = lut.resampled(RESAMPLED_TABLE_SIZE);
		for rgb in [[0., 0., 0.], [0.25, 0.5, 0.75], [0.3, 0.9, 0.1], [1., 1., 1.]] {
			assert_close_within(resampled.apply(rgb), lut.apply(rgb), 1e-3);
		}
	}

	#[test]
	fn a_resampled_table_keeps_the_domain_and_input_curves_colors_reach_it_by() {
		// A domain reaching past 0 and 1, where an earlier lookup can leave a color
		let mut cube = String::from("LUT_3D_SIZE 9\nDOMAIN_MIN -1 -1 -1\nDOMAIN_MAX 2 2 2\n");
		for index in 0..9 * 9 * 9 {
			let [r, g, b] = [index % 9, index / 9 % 9, index / 81].map(|step| step as f32 / 8.);
			cube += &format!("{r} {} {}\n", g * g, (r + b) / 2.);
		}
		let lut = Lut::parse(cube.as_bytes()).unwrap();
		let curved = Lut {
			input_curves: Some([Curve::Power(2.), Curve::Sampled(vec![-1., 0.4, 2.]), Curve::Power(0.5)]),
			..lut.clone()
		};

		for original in [lut, curved] {
			let resampled = original.resampled(RESAMPLED_TABLE_SIZE);
			for rgb in [[1.5, 1.5, 1.5], [-0.5, 0.25, 1.75], [0., 0.5, 1.], [2., -1., 0.8]] {
				assert_close_within(resampled.apply(rgb), original.apply(rgb), 1e-3);
			}
		}
	}

	#[test]
	fn a_table_counts_its_texels_and_blocks_as_it_lays_them_out() {
		let curves = || [Curve::Power(2.), Curve::Sampled(vec![0., 0.4, 1.]), Curve::Parametric { function: 0, parameters: vec![1.8] }];
		let piecewise = Curve::Piecewise {
			inputs: vec![0., 0.5, 1.],
			outputs: vec![0., 0.3, 1.],
		};
		let matrix = Stage::Matrix {
			matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
			offset: [0.1, 0., -0.1],
		};
		let one_dimensional = Lut::parse(b"LUT_1D_SIZE 3\n0 0 0\n0.5 0.4 0.6\n1 1 1\n").unwrap();
		let three_dimensional = Lut::parse(b"LUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n").unwrap();

		// A table alone is one block, and each curve and matrix around it is another
		let tables = [
			(Lut::new(None), 1.),
			(one_dimensional.clone(), 1.),
			(three_dimensional.clone(), 1.),
			(
				Lut {
					input_curves: Some([piecewise.clone(), piecewise.clone(), piecewise]),
					..three_dimensional
				},
				4.,
			),
			(
				Lut {
					input_curves: Some(curves()),
					output_stages: vec![Stage::Curves(curves()), matrix, Stage::Curves(curves())],
					..one_dimensional
				},
				11.,
			),
		];
		for (lut, block_count) in tables {
			let texels = lut.texels();
			assert_eq!((lut.texel_count(), texels[0][1]), (texels.len(), block_count), "{lut:?}");

			// Each block's header leads to the next, through to the end
			let mut block = 3;
			for _ in 0..block_count as usize {
				block += texels[block][2] as usize + ((texels[block][3] as usize) << 16);
			}
			assert_eq!(block, texels.len(), "{lut:?}");
		}
	}

	#[test]
	fn rejects_a_size_with_an_entry_count_that_overflows() {
		assert!(Lut::parse(b"LUT_3D_SIZE 4194304\n").is_err());
		assert!(Lut::parse(b"<LUT><size>4194304</size><data></data></LUT>").is_err());
		assert!(Lut::parse(b"CSPLUTV100\n3D\n0\n0\n0\n4194304 4194304 4194304\n").is_err());
	}
}
