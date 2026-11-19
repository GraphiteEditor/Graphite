//! Runs of adjustments encoded as the steps the GPU carries out, each from the kernel that [`visit_kernel`] prepares for the CPU.

use crate::MapColor;
use crate::adjustments::{AdjustmentVisitor, visit_kernel};
use graphic_types::adjustment::Adjustment;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;

/// The kind of a step, which picks the shader function carrying it out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StepKind {
	Desaturate,
	GammaCorrection,
	MakeOpaque,
	BrightnessContrast,
	Levels,
	Curves,
	BlackAndWhite,
	HueSaturation,
	Invert,
	Threshold,
	GradientMap,
	Vibrance,
	ChannelMixer,
	SelectiveColor,
	Posterize,
	Exposure,
	ColorBalance,
	PhotoFilter,
	ColorLookup,
	ColorOverlay,
	Solarize,
}

impl StepKind {
	/// Every kind, in the order of their codes.
	pub const ALL: [StepKind; 21] = [
		StepKind::Desaturate,
		StepKind::GammaCorrection,
		StepKind::MakeOpaque,
		StepKind::BrightnessContrast,
		StepKind::Levels,
		StepKind::Curves,
		StepKind::BlackAndWhite,
		StepKind::HueSaturation,
		StepKind::Invert,
		StepKind::Threshold,
		StepKind::GradientMap,
		StepKind::Vibrance,
		StepKind::ChannelMixer,
		StepKind::SelectiveColor,
		StepKind::Posterize,
		StepKind::Exposure,
		StepKind::ColorBalance,
		StepKind::PhotoFilter,
		StepKind::ColorLookup,
		StepKind::ColorOverlay,
		StepKind::Solarize,
	];

	/// The number a step's header holds for its kind, which the shader picks the step's function by.
	pub fn code(self) -> u32 {
		self as u32
	}
}

/// Bulky data that steps read from a texture of its own, which stays uploaded while the run's other settings change.
pub struct StepTable {
	/// Identifies the table's content, such as the hash of the file it was read from.
	pub key: u64,
	/// Makes the table's texels, no more than the given count where it can, only once no upload of them is kept.
	pub texels: Box<dyn Fn(usize) -> Vec<[f32; 4]> + Send + Sync>,
}

/// A run of adjustments as the data the GPU's shaders read: for each step, a texel holding how many texels the step spans and its kind's code,
/// then its parameters. The tables some steps read go in a texture of their own, after a texel locating each one.
pub struct AdjustmentSteps {
	kinds: Vec<StepKind>,
	texels: Vec<[f32; 4]>,
	/// Where each step's header lies among the texels.
	headers: Vec<usize>,
	tables: Vec<StepTable>,
}

impl AdjustmentSteps {
	/// The steps carrying out `run` in order, leaving out adjustments that leave every color as it is.
	pub fn encode(run: &[&Adjustment]) -> Self {
		struct EncodeStep<'a>(&'a mut AdjustmentSteps);

		impl AdjustmentVisitor for EncodeStep<'_> {
			type Output = ();

			fn visit<K: MapColor>(self, kernel: K) {
				kernel.encode_step(self.0);
			}

			fn unchanged(self) {}
		}

		let mut steps = Self {
			kinds: Vec::new(),
			texels: Vec::new(),
			headers: Vec::new(),
			tables: Vec::new(),
		};
		for adjustment in run {
			visit_kernel(adjustment, EncodeStep(&mut steps));
		}
		steps
	}

	/// Adds a step of `kind` that reads `parameters`.
	pub fn push(&mut self, kind: StepKind, parameters: impl IntoIterator<Item = [f32; 4]>) {
		let header = self.texels.len();
		self.texels.push([0.; 4]);
		self.texels.extend(parameters);

		let [low, high] = split_count(self.texels.len() - header);
		self.texels[header] = texel(&[low, high, kind.code() as f32]);
		self.kinds.push(kind);
		self.headers.push(header);
	}

	/// Adds a table that steps read, returning its index among the run's tables. A table with a `key` the run already holds is shared instead.
	pub fn push_table(&mut self, key: u64, texels: impl Fn(usize) -> Vec<[f32; 4]> + Send + Sync + 'static) -> usize {
		if let Some(index) = self.tables.iter().position(|table| table.key == key) {
			return index;
		}

		self.tables.push(StepTable { key, texels: Box::new(texels) });
		self.tables.len() - 1
	}

	/// The kind of each step in order.
	pub fn kinds(&self) -> &[StepKind] {
		&self.kinds
	}

	/// The texels of the steps, starting with the first step's.
	pub fn texels(&self) -> &[[f32; 4]] {
		&self.texels
	}

	/// The texels of the steps numbered within `steps`, starting with the first one's header.
	pub fn step_texels(&self, steps: Range<usize>) -> &[[f32; 4]] {
		let header = |step: usize| self.headers.get(step).copied().unwrap_or(self.texels.len());
		&self.texels[header(steps.start)..header(steps.end)]
	}

	/// Identifies the content of the run's tables together, or `None` without any.
	pub fn table_key(&self) -> Option<u64> {
		if self.tables.is_empty() {
			return None;
		}

		let mut hasher = DefaultHasher::new();
		self.tables.iter().for_each(|table| table.key.hash(&mut hasher));
		Some(hasher.finish())
	}

	/// The texels of the run's tables, no more than `max_texels` where they can be: a texel locating each table, then the tables in order.
	pub fn table_texels(&self, max_texels: usize) -> Vec<[f32; 4]> {
		let budget = max_texels.saturating_sub(self.tables.len()) / self.tables.len().max(1);

		let mut texels = vec![[0.; 4]; self.tables.len()];
		for (index, table) in self.tables.iter().enumerate() {
			let [low, high] = split_count(texels.len());
			texels[index] = texel(&[low, high]);
			texels.extend((table.texels)(budget));
		}
		texels
	}
}

/// Up to four values as a texel, with zeros after them.
pub(crate) fn texel(values: &[f32]) -> [f32; 4] {
	let mut texel = [0.; 4];
	texel.iter_mut().zip(values).for_each(|(slot, value)| *slot = *value);
	texel
}

/// A count as its low and high 16 bits, each exact as an `f32` however large the count, which the shader joins back together.
pub(crate) fn split_count(count: usize) -> [f32; 2] {
	[(count & 0xFFFF) as f32, (count >> 16) as f32]
}

/// A double-precision value as the nearest `f32` and the remainder past it, for the shader to compare and subtract single-precision values against
/// in two steps. That tells them apart as the CPU's double precision does, unless a compiler folds the steps into one (as fast math may).
pub(crate) fn f32_pair(value: f64) -> [f32; 2] {
	let high = value as f32;
	[high, (value - high as f64) as f32]
}

/// The constants the shader's step functions share with the kernels, declared in WGSL: the codes the kernels encode settings with and the slacks they compare with.
pub fn shader_constants() -> String {
	let codes = crate::adjustments::shader_constants()
		.into_iter()
		.chain(crate::blending::shader_constants())
		.chain(crate::color_lookup_table::shader_constants());
	let slacks = [
		("THRESHOLD_SLACK", crate::adjustments::THRESHOLD_SLACK),
		("MAPPED_POSITION_SLACK", vector_types::gradient::MAPPED_POSITION_SLACK as f32),
		("BLEND_EDGE_TOLERANCE", core_types::color::BLEND_EDGE_TOLERANCE),
	];

	(codes.map(|(name, value)| format!("const {name}: u32 = {value}u;\n")))
		.chain(slacks.map(|(name, value)| format!("const {name}: f32 = {value:e};\n")))
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::Color;
	use core_types::list::Item;
	use graphene_resource::Resource;
	use vector_types::Gradient;

	#[test]
	fn a_run_of_every_kind_of_adjustment_encodes_as_one_list_of_steps() {
		let run = [
			Adjustment::Desaturate { method: Default::default() },
			Adjustment::GammaCorrection { gamma: 2.2, inverse: false },
			Adjustment::MakeOpaque,
			Adjustment::BrightnessContrast {
				brightness: 10.,
				contrast: 10.,
				use_classic: false,
				classic_pivot: 127.,
			},
			Adjustment::Levels(Default::default()),
			Adjustment::Curves(Default::default()),
			Adjustment::BlackAndWhite(Default::default()),
			Adjustment::HueSaturation(Default::default()),
			Adjustment::Invert,
			Adjustment::Threshold {
				min_luminance: 50.,
				max_luminance: 100.,
			},
			Adjustment::GradientMap {
				gradient: Item::new_from_element(Gradient::black_to_white()),
				reverse: false,
			},
			Adjustment::Vibrance { vibrance: 20., saturation: 0. },
			Adjustment::ChannelMixer(Default::default()),
			Adjustment::SelectiveColor(Default::default()),
			Adjustment::Posterize { levels: 4 },
			Adjustment::Exposure {
				exposure: 1.,
				offset: 0.,
				gamma_correction: 1.,
			},
			Adjustment::ColorBalance(Default::default()),
			Adjustment::PhotoFilter {
				color: Color::WHITE,
				density: 25.,
				preserve_luminosity: true,
			},
			Adjustment::ColorLookup {
				lut_file: Resource::new(b"LUT_1D_SIZE 2\n0 0 0\n1 1 1\n".to_vec()),
			},
			// A file that can't be read changes no color, so it leaves no step
			Adjustment::ColorLookup {
				lut_file: Resource::new(b"not a table".to_vec()),
			},
			Adjustment::ColorOverlay {
				color: Color::BLACK,
				blend_mode: Default::default(),
				opacity: 50.,
			},
			Adjustment::Solarize,
		];
		let steps = AdjustmentSteps::encode(&run.iter().collect::<Vec<_>>());

		// One step for each kind, each header holding its kind and leading to the next, through to the end of the one list the shader reads
		let texels = steps.texels();
		let mut header = 0;
		for kind in steps.kinds() {
			assert_eq!(texels[header][2], kind.code() as f32, "{kind:?}");
			header += texels[header][0] as usize + ((texels[header][1] as usize) << 16);
		}
		assert_eq!(header, texels.len());
		assert_eq!(steps.kinds(), StepKind::ALL);
		assert!(steps.table_key().is_some());

		// A range of steps starts at its first step's header and ends where the step after its last begins
		assert_eq!(steps.step_texels(0..StepKind::ALL.len()), texels);
		let lookup = steps.step_texels(18..19);
		assert_eq!((lookup.len(), lookup[0][2]), (2, StepKind::ColorLookup.code() as f32));
	}

	#[test]
	fn every_kind_is_listed_by_its_code() {
		for (code, kind) in StepKind::ALL.into_iter().enumerate() {
			assert_eq!(kind.code() as usize, code, "{kind:?}");
		}
	}

	#[test]
	fn counts_too_large_for_one_float_split_into_exact_halves() {
		for count in [0, 1, 65_535, 65_536, 16_777_217, 16_974_599] {
			let [low, high] = split_count(count);
			assert_eq!(low as usize + ((high as usize) << 16), count);
		}
	}

	#[test]
	fn a_value_and_its_remainder_compare_as_the_double_does() {
		let [high, low] = f32_pair(0.499_999_99);
		assert_eq!(high, 0.5);
		assert!(0.5 - high > low, "0.5 lies past 0.49999999");
	}

	#[test]
	fn a_table_read_twice_in_a_run_is_held_once() {
		let lookup = |file: &[u8]| Adjustment::ColorLookup {
			lut_file: Resource::new(file.to_vec()),
		};
		let (identity, inverse) = (lookup(b"LUT_1D_SIZE 2\n0 0 0\n1 1 1\n"), lookup(b"LUT_1D_SIZE 2\n1 1 1\n0 0 0\n"));
		let steps = AdjustmentSteps::encode(&[&identity, &inverse, &identity]);

		// Each step holds its header then the number of its table, and the tables start with a texel locating each
		let table_numbers: Vec<f32> = steps.texels().as_chunks::<2>().0.iter().map(|step| step[1][0]).collect();
		assert_eq!(table_numbers, [0., 1., 0.]);
		assert_eq!(steps.table_texels(usize::MAX)[0][0], 2.);
	}

	#[test]
	fn the_shader_is_given_the_kernels_slacks() {
		let constants = shader_constants();
		for constant in [
			"const THRESHOLD_SLACK: f32 = 1.1967678e-7;",
			"const MAPPED_POSITION_SLACK: f32 = 1.5258789e-5;",
			"const BLEND_EDGE_TOLERANCE: f32 = 1e-6;",
		] {
			assert!(constants.contains(constant), "{constant} is missing from\n{constants}");
		}
	}
}
