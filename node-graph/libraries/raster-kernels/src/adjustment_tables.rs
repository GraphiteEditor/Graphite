//! Bakes runs of adjustments into the lookup tables the GPU carries them out with, sampling the CPU kernels so the two agree by construction.

use crate::adjustments::adjust_colors;
use core_types::Color;
use graphic_types::adjustment::Adjustment;

/// Entries in a one-dimensional table, enough that interpolating between them strays less than a level from even a steep curve.
pub const CURVE_TABLE_SIZE: usize = 4096;

/// Grid points along each axis of a color cube, enough that interpolating within its tetrahedra stays within a few levels of the smooth kernels.
pub const CUBE_SIZE: usize = 65;

/// Grid points along each axis of a draft color cube, an eighth the work of [`CUBE_SIZE`] for settings changing every frame, at about three times its error.
pub const DRAFT_CUBE_SIZE: usize = 33;

/// One pass of the GPU's adjustment shader, with the tables it looks colors up in. Values are gamma-encoded.
#[derive(Clone, Debug, PartialEq)]
pub enum AdjustmentPass {
	/// The red, green, blue, and alpha channels each through their own curve, sampled at [`CURVE_TABLE_SIZE`] values from 0 to 1.
	Curves(Vec<[f32; 4]>),
	/// Colors through a cube of `size` samples along each of red, green, and blue, with red varying fastest and blue slowest.
	/// Alpha passes through its own curve of [`CURVE_TABLE_SIZE`] samples.
	Cube { size: usize, colors: Vec<[f32; 3]>, alpha: Vec<f32> },
	/// Each color replaced by the entry of [`CURVE_TABLE_SIZE`] its luma (the weighted sum of its channels) picks, keeping its alpha.
	Luma { weights: [f32; 3], colors: Vec<[f32; 3]> },
	/// Vibrance, whose steep toe near black no table captures, computed directly from its amounts in -1..1 and its saturation scale.
	Vibrance { vibrance: f32, saturation_scale: f32 },
}

impl AdjustmentPass {
	/// The passes that carry out `adjustments` in order, merging each run of those a table can capture into one pass, with cubes of `cube_size` along each side.
	pub fn plan(adjustments: &[Adjustment], cube_size: usize) -> Vec<Self> {
		let mut passes = Vec::new();
		let mut run: Vec<&Adjustment> = Vec::new();

		for adjustment in adjustments {
			let own_pass = match adjustment {
				Adjustment::Vibrance { vibrance, saturation } => Some(Self::Vibrance {
					vibrance: (vibrance / 100.) as f32,
					saturation_scale: (1. + saturation / 100.) as f32,
				}),
				// A step in luma, which a table of luma keeps sharp where a cube would blur it
				Adjustment::Threshold { .. } => Some(Self::Luma {
					weights: [4915., 9667., 1802.].map(|weight| weight / 16384.),
					colors: luma_table(adjustment, [4915., 9667., 1802.].map(|weight| weight / 16384.)),
				}),
				// A gradient's hard stops are steps too
				Adjustment::GradientMap { .. } => Some(Self::Luma {
					weights: [0.3, 0.59, 0.11],
					colors: luma_table(adjustment, [0.3, 0.59, 0.11]),
				}),
				_ => None,
			};

			match own_pass {
				Some(pass) => {
					passes.extend(Self::from_run(&run, cube_size));
					run.clear();
					passes.push(pass);
				}
				None => run.push(adjustment),
			}
		}
		passes.extend(Self::from_run(&run, cube_size));

		passes
	}

	/// Whether any of the passes looks colors up in a cube, the only table whose size varies.
	pub fn any_cube(passes: &[Self]) -> bool {
		passes.iter().any(|pass| matches!(pass, Self::Cube { .. }))
	}

	/// One pass carrying out a run of adjustments in order, as four curves if each acts on the channels separately, or else as a cube.
	fn from_run(run: &[&Adjustment], cube_size: usize) -> Option<Self> {
		if run.is_empty() {
			return None;
		}
		let apply_run = |colors: &mut [Color]| run.iter().for_each(|adjustment| adjust_colors(adjustment, colors));

		// The color channels never read alpha, and alpha depends on alpha alone, so one ramp feeds every channel at once
		let mut ramp: Vec<Color> = (0..CURVE_TABLE_SIZE)
			.map(|index| {
				let value = index as f32 / (CURVE_TABLE_SIZE - 1) as f32;
				Color::from_gamma_srgb_channels(value, value, value, value)
			})
			.collect();
		apply_run(&mut ramp);
		let ramp: Vec<[f32; 4]> = ramp.iter().map(Color::to_gamma_srgb_channels).collect();

		if run.iter().all(|adjustment| adjustment.is_per_channel()) {
			return Some(Self::Curves(ramp));
		}

		let step = |index: usize| index as f32 / (cube_size - 1) as f32;
		let mut cube = Vec::with_capacity(cube_size.pow(3));
		for blue in 0..cube_size {
			for green in 0..cube_size {
				for red in 0..cube_size {
					cube.push(Color::from_gamma_srgb_channels(step(red), step(green), step(blue), 1.));
				}
			}
		}
		apply_run(&mut cube);

		Some(Self::Cube {
			size: cube_size,
			colors: cube.iter().map(|color| color.to_gamma_srgb_channels()).map(|[red, green, blue, _]| [red, green, blue]).collect(),
			alpha: ramp.iter().map(|channels| channels[3]).collect(),
		})
	}
}

/// The adjustment's results for grays of each luma, which stand for every color of that luma when the adjustment reads only its luma.
fn luma_table(adjustment: &Adjustment, weights: [f32; 3]) -> Vec<[f32; 3]> {
	let total: f32 = weights.iter().sum();
	let mut grays: Vec<Color> = (0..CURVE_TABLE_SIZE)
		.map(|index| {
			let value = index as f32 / (CURVE_TABLE_SIZE - 1) as f32 / total;
			Color::from_gamma_srgb_channels(value, value, value, 1.)
		})
		.collect();
	adjust_colors(adjustment, &mut grays);

	grays.iter().map(|color| color.to_gamma_srgb_channels()).map(|[red, green, blue, _]| [red, green, blue]).collect()
}
