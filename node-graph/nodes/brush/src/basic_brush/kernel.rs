//! Brush kernel baking + caching.
//!
//! Kernel is a super-Gaussian `exp(-((v^2 + s^2) / 2)^p)`: p = 1 plain Gaussian, higher p
//! flattens center + steepens edge. Hardness controls p. Sweep along a segment has no
//! closed form, so baked numerically into a texture: row per perpendicular distance,
//! columns accumulate the along-axis integral. Segment = two LUT samples,
//! `F(v, t) - F(v, t - len)`. Normalized so a long stroke's interior settles at 1.
//!
//! Calibration defines diameter: find where resolved alpha crosses EDGE_ALPHA, scale so
//! that contour lands on `diameter / 2`. Painted width matches the setting, hard or soft.
//! p clamped so the edge stays >= MIN_EDGE_TEXELS on screen.
//!
//! Baked kernels: small LRU keyed by quantized p. Textures from the global pool, held
//! weakly; evicted under pressure -> bake again.

use super::consts::{LUT_CACHE_SIZE, LUT_SIZE, LUT_T_MAX, LUT_V_MAX, RIDGE_GAIN, SIGMA_PER_DIAMETER};
use super::stroke::StyledStroke;
use glam::UVec2;
use raster_types::{Texture, TextureWeakRef};
use std::sync::Mutex;
use wgpu_executor::WgpuExecutor;

const INTEGRATE_END: f64 = 12.;
const FINE_STEPS: usize = 4096;
const MIN_EDGE_TEXELS: f64 = 1.5;

const EDGE_WIDTH_FACTOR: f64 = 3.09;

const KEY_STEPS_PER_LN: f64 = 24.;

const SOFTEST: f64 = 0.7;
const HARDEST: f64 = 48.;

const EDGE_ALPHA: f64 = 0.05;

pub(super) struct Kernel {
	pub(super) texture: Texture,
	pub(super) scale: f32,
	pub(super) exponent: f32,
	pub(super) section_scale: f32,
}

struct Baked {
	scale: f32,
	exponent: f32,
	section_scale: f32,
	texture: TextureWeakRef,
}

#[derive(Default)]
pub(super) struct KernelCache {
	entries: Mutex<Vec<(i32, Baked)>>,
}

impl KernelCache {
	pub(super) fn get(&self, executor: &WgpuExecutor, stroke: &StyledStroke, scale: f64) -> Kernel {
		let sigma_texels = stroke.diameter.max(0.) * SIGMA_PER_DIAMETER * scale;
		let sharpest = (EDGE_WIDTH_FACTOR * sigma_texels / (2. * MIN_EDGE_TEXELS)).max(1.);
		let exponent = (SOFTEST * (HARDEST / SOFTEST).powf(stroke.hardness.clamp(0., 1.))).min(sharpest);
		let key = (exponent.ln() * KEY_STEPS_PER_LN).round() as i32;
		let mut entries = self.entries.lock().unwrap();
		if let Some(index) = entries.iter().position(|(cached, _)| *cached == key) {
			if let Some(texture) = entries[index].1.texture.upgrade() {
				let entry = entries.remove(index);
				let kernel = Kernel {
					texture,
					scale: entry.1.scale,
					exponent: entry.1.exponent,
					section_scale: entry.1.section_scale,
				};
				entries.insert(0, entry);
				return kernel;
			}
			entries.remove(index);
		}
		let kernel = bake(executor, (key as f64 / KEY_STEPS_PER_LN).exp());
		let baked = Baked {
			scale: kernel.scale,
			exponent: kernel.exponent,
			section_scale: kernel.section_scale,
			texture: kernel.texture.downgrade(),
		};
		entries.insert(0, (key, baked));
		entries.truncate(LUT_CACHE_SIZE);
		kernel
	}
}

fn kernel(v: f64, s: f64, exponent: f64) -> f64 {
	(-((v * v + s * s) / 2.).powf(exponent)).exp()
}

fn sweep_row(v: f64, exponent: f64) -> (Vec<f64>, f64) {
	let ds = 2. * INTEGRATE_END / FINE_STEPS as f64;
	let mut cumulative = Vec::with_capacity(FINE_STEPS + 1);
	let mut total = 0.;
	let mut previous = kernel(v, -INTEGRATE_END, exponent);
	cumulative.push(0.);
	for i in 1..=FINE_STEPS {
		let value = kernel(v, -INTEGRATE_END + i as f64 * ds, exponent);
		total += (previous + value) / 2. * ds;
		previous = value;
		cumulative.push(total);
	}
	let samples = (0..LUT_SIZE)
		.map(|j| {
			let t = -LUT_T_MAX + j as f64 * 2. * LUT_T_MAX / (LUT_SIZE - 1) as f64;
			let x = (t + INTEGRATE_END) / ds;
			let i = (x.floor() as usize).min(FINE_STEPS - 1);
			cumulative[i] + (cumulative[i + 1] - cumulative[i]) * (x - i as f64)
		})
		.collect();
	(samples, total)
}

fn calibrate(ridge: &[f64], target: f64) -> f64 {
	let step = LUT_V_MAX / (LUT_SIZE - 1) as f64;
	let Some(i) = ridge.iter().position(|&r| r < target).filter(|&i| i > 0) else {
		return LUT_V_MAX;
	};
	let (above, below) = (ridge[i - 1], ridge[i]);
	step * ((i - 1) as f64 + (above - target) / (above - below))
}

fn bake(executor: &WgpuExecutor, exponent: f64) -> Kernel {
	let mut rows = Vec::with_capacity((LUT_SIZE * LUT_SIZE) as usize);
	let mut ridge = Vec::with_capacity(LUT_SIZE as usize);
	let mut norm = 1.;
	for row in 0..LUT_SIZE {
		let v = row as f64 * LUT_V_MAX / (LUT_SIZE - 1) as f64;
		if kernel(v, 0., exponent) < 1e-9 {
			rows.resize(rows.len() + LUT_SIZE as usize, half::f16::ZERO);
			ridge.push(0.);
			continue;
		}
		let (samples, total) = sweep_row(v, exponent);
		if row == 0 {
			norm = 1. / total;
		}
		rows.extend(samples.into_iter().map(|value| half::f16::from_f64(value * norm)));
		ridge.push(total * norm);
	}
	let texture = executor.request_texture_with_format(UVec2::splat(LUT_SIZE), wgpu::TextureFormat::R16Float);
	executor.context().queue.write_texture(
		texture.as_image_copy(),
		bytemuck::cast_slice(&rows),
		wgpu::TexelCopyBufferLayout {
			offset: 0,
			bytes_per_row: Some(LUT_SIZE * 2),
			rows_per_image: Some(LUT_SIZE),
		},
		texture.size(),
	);
	let gain = RIDGE_GAIN as f64;
	let target = -(1. - EDGE_ALPHA * (1. - (-gain).exp())).ln() / gain;
	let a = calibrate(&ridge, target);
	Kernel {
		texture,
		scale: (a / 2.) as f32,
		exponent: exponent as f32,
		section_scale: ((2. * (1. / target).ln().powf(1. / exponent)).sqrt() / 2.) as f32,
	}
}
