//! The kernels rendering each [`GeneratorPattern`] across any area of its space at any resolution.

use core_types::Color;
use glam::DVec2;
use graphic_types::generator::{CellularDistanceFunction, CellularReturnType, DomainWarpType, FractalType, GeneratorPattern, NoiseSettings, NoiseType};
use raster_types::parallel_rows::fill_rows;
use raster_types::{Image, ImageError};

/// The most iterations a point is tested for escaping the Mandelbrot set, beyond which it counts as inside.
const MANDELBROT_ITERATIONS: u32 = 255;

/// The pattern across `area` of its own space, from its minimum to its maximum corner, sampled at the centers of a `width` by `height` grid of pixels.
/// Fails where an image of that size can't be allocated.
/// The rows are rendered in parallel on native targets.
pub fn render_pattern(pattern: &GeneratorPattern, area: [DVec2; 2], width: u32, height: u32) -> Result<Image<Color>, ImageError> {
	let mut image = Image::try_new(width, height, Color::TRANSPARENT)?;
	let pixel_size = (area[1] - area[0]) / DVec2::new(width as f64, height as f64);
	let pixel_center = |x: usize, y: usize| area[0] + (DVec2::new(x as f64, y as f64) + 0.5) * pixel_size;

	let sample = pattern_sampler(pattern);
	fill_rows(&mut image.data, width as usize, |y, row| {
		row.iter_mut().enumerate().for_each(|(x, pixel)| *pixel = sample(pixel_center(x, y)));
	});

	Ok(image)
}

/// Samples the pattern's color at points in its own space, after the setup shared by every sample.
pub fn pattern_sampler(pattern: &GeneratorPattern) -> impl Fn(DVec2) -> Color + Sync + use<> {
	let noise = match pattern {
		GeneratorPattern::Noise(settings) => Some(NoiseSampler::new(settings)),
		GeneratorPattern::Mandelbrot => None,
	};

	move |point| match &noise {
		Some(noise) => Color::from_luminance(noise.sample(point)),
		None => {
			let value = mandelbrot_escape_iterations(point * 3. - DVec2::new(2., 1.5)) as f32 / MANDELBROT_ITERATIONS as f32;
			Color::from_rgbaf32_unchecked(value, value, value, 1.)
		}
	}
}

/// Samples a noise pattern's luminance, from 0 to 1, at positions in its space.
struct NoiseSampler {
	noise: fastnoise_lite::FastNoiseLite,
	white_noise_seed: Option<u64>,
	domain_warp: bool,
}

impl NoiseSampler {
	fn new(settings: &NoiseSettings) -> Self {
		let mut noise = fastnoise_lite::FastNoiseLite::with_seed(settings.seed as i32);
		noise.set_frequency(Some(1. / (settings.scale as f32).max(f32::EPSILON)));

		// Domain warp
		let domain_warp_type = match settings.domain_warp_type {
			DomainWarpType::None => None,
			DomainWarpType::OpenSimplex2 => Some(fastnoise_lite::DomainWarpType::OpenSimplex2),
			DomainWarpType::OpenSimplex2Reduced => Some(fastnoise_lite::DomainWarpType::OpenSimplex2Reduced),
			DomainWarpType::BasicGrid => Some(fastnoise_lite::DomainWarpType::BasicGrid),
		};
		let domain_warp = domain_warp_type.is_some() && settings.domain_warp_amplitude > 0.;
		noise.set_domain_warp_type(domain_warp_type);
		noise.set_domain_warp_amp(Some(settings.domain_warp_amplitude as f32));

		// Noise and fractal
		let noise_type = match settings.noise_type {
			NoiseType::Perlin => fastnoise_lite::NoiseType::Perlin,
			NoiseType::OpenSimplex2 => fastnoise_lite::NoiseType::OpenSimplex2,
			NoiseType::OpenSimplex2S => fastnoise_lite::NoiseType::OpenSimplex2S,
			NoiseType::Cellular => fastnoise_lite::NoiseType::Cellular,
			NoiseType::ValueCubic => fastnoise_lite::NoiseType::ValueCubic,
			NoiseType::Value => fastnoise_lite::NoiseType::Value,
			NoiseType::WhiteNoise => {
				return Self {
					noise,
					white_noise_seed: Some(settings.seed as u64),
					domain_warp: false,
				};
			}
		};
		noise.set_noise_type(Some(noise_type));
		let fractal_type = match settings.fractal_type {
			FractalType::None => fastnoise_lite::FractalType::None,
			FractalType::FBm => fastnoise_lite::FractalType::FBm,
			FractalType::Ridged => fastnoise_lite::FractalType::Ridged,
			FractalType::PingPong => fastnoise_lite::FractalType::PingPong,
			FractalType::DomainWarpProgressive => fastnoise_lite::FractalType::DomainWarpProgressive,
			FractalType::DomainWarpIndependent => fastnoise_lite::FractalType::DomainWarpIndependent,
		};
		noise.set_fractal_type(Some(fractal_type));
		noise.set_fractal_octaves(Some(settings.fractal_octaves as i32));
		noise.set_fractal_lacunarity(Some(settings.fractal_lacunarity as f32));
		noise.set_fractal_gain(Some(settings.fractal_gain as f32));
		noise.set_fractal_weighted_strength(Some(settings.fractal_weighted_strength as f32));
		noise.set_fractal_ping_pong_strength(Some(settings.fractal_ping_pong_strength as f32));

		// Cellular
		let cellular_distance_function = match settings.cellular_distance_function {
			CellularDistanceFunction::Euclidean => fastnoise_lite::CellularDistanceFunction::Euclidean,
			CellularDistanceFunction::EuclideanSq => fastnoise_lite::CellularDistanceFunction::EuclideanSq,
			CellularDistanceFunction::Manhattan => fastnoise_lite::CellularDistanceFunction::Manhattan,
			CellularDistanceFunction::Hybrid => fastnoise_lite::CellularDistanceFunction::Hybrid,
		};
		let cellular_return_type = match settings.cellular_return_type {
			CellularReturnType::CellValue => fastnoise_lite::CellularReturnType::CellValue,
			CellularReturnType::Nearest => fastnoise_lite::CellularReturnType::Distance,
			CellularReturnType::NextNearest => fastnoise_lite::CellularReturnType::Distance2,
			CellularReturnType::Average => fastnoise_lite::CellularReturnType::Distance2Add,
			CellularReturnType::Difference => fastnoise_lite::CellularReturnType::Distance2Sub,
			CellularReturnType::Product => fastnoise_lite::CellularReturnType::Distance2Mul,
			CellularReturnType::Division => fastnoise_lite::CellularReturnType::Distance2Div,
		};
		noise.set_cellular_distance_function(Some(cellular_distance_function));
		noise.set_cellular_return_type(Some(cellular_return_type));
		noise.set_cellular_jitter(Some(settings.cellular_jitter as f32));

		Self {
			noise,
			white_noise_seed: None,
			domain_warp,
		}
	}

	fn sample(&self, position: DVec2) -> f32 {
		// White noise gives each square of one document unit its own random value
		if let Some(seed) = self.white_noise_seed {
			let cell = position.floor().as_i64vec2();
			return white_noise(seed, cell.x, cell.y);
		}

		let (mut x, mut y) = (position.x as f32, position.y as f32);
		if self.domain_warp {
			(x, y) = self.noise.domain_warp_2d(x, y);
		}
		(self.noise.get_noise_2d(x, y) + 1.) * 0.5
	}
}

/// A random value from 0 to 1 for each cell of a grid, the same for the same seed and cell.
fn white_noise(seed: u64, x: i64, y: i64) -> f32 {
	// The SplitMix64 finalizer, which spreads every input bit across the whole result
	let mut value = seed ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
	value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
	value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
	value ^= value >> 31;

	// The top 24 bits, which an f32 holds exactly
	(value >> 40) as f32 / (1_u64 << 24) as f32
}

/// How many iterations `c` takes to escape the Mandelbrot set, or the iteration limit if it doesn't.
fn mandelbrot_escape_iterations(c: DVec2) -> u32 {
	let mut z = DVec2::ZERO;
	for iteration in 0..MANDELBROT_ITERATIONS {
		z = DVec2::new(z.x * z.x - z.y * z.y, 2. * z.x * z.y) + c;
		if z.length_squared() > 4. {
			return iteration;
		}
	}
	MANDELBROT_ITERATIONS
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn renders_of_overlapping_areas_agree_where_they_overlap() {
		let noise = GeneratorPattern::Noise(NoiseSettings {
			seed: 7,
			scale: 10.,
			noise_type: NoiseType::OpenSimplex2,
			domain_warp_type: DomainWarpType::None,
			domain_warp_amplitude: 0.,
			fractal_type: FractalType::FBm,
			fractal_octaves: 3,
			fractal_lacunarity: 2.,
			fractal_gain: 0.5,
			fractal_weighted_strength: 0.,
			fractal_ping_pong_strength: 2.,
			cellular_distance_function: CellularDistanceFunction::Euclidean,
			cellular_return_type: CellularReturnType::Nearest,
			cellular_jitter: 1.,
		});

		// One pixel per unit across 0 to 32, and again across 16 to 48, as two neighboring tiles of a render would sample it
		let whole = render_pattern(&noise, [DVec2::ZERO, DVec2::splat(32.)], 32, 32).unwrap();
		let shifted = render_pattern(&noise, [DVec2::splat(16.), DVec2::splat(48.)], 32, 32).unwrap();
		for y in 0..16 {
			for x in 0..16 {
				assert_eq!(whole.data[(y + 16) * 32 + x + 16], shifted.data[y * 32 + x]);
			}
		}
	}

	#[test]
	fn white_noise_is_the_same_wherever_and_however_finely_it_is_sampled() {
		let white_noise = |seed| {
			GeneratorPattern::Noise(NoiseSettings {
				seed,
				scale: 10.,
				noise_type: NoiseType::WhiteNoise,
				domain_warp_type: DomainWarpType::None,
				domain_warp_amplitude: 0.,
				fractal_type: FractalType::None,
				fractal_octaves: 1,
				fractal_lacunarity: 2.,
				fractal_gain: 0.5,
				fractal_weighted_strength: 0.,
				fractal_ping_pong_strength: 2.,
				cellular_distance_function: CellularDistanceFunction::Euclidean,
				cellular_return_type: CellularReturnType::Nearest,
				cellular_jitter: 1.,
			})
		};

		// Each unit cell keeps its value when sampled four times as finely
		let coarse = render_pattern(&white_noise(3), [DVec2::ZERO, DVec2::splat(4.)], 4, 4).unwrap();
		let fine = render_pattern(&white_noise(3), [DVec2::ZERO, DVec2::splat(4.)], 16, 16).unwrap();
		for y in 0..16 {
			for x in 0..16 {
				assert_eq!(fine.data[y * 16 + x], coarse.data[(y / 4) * 4 + x / 4]);
			}
		}

		// The values spread across the whole range, and another seed gives others
		let luminances: Vec<f32> = coarse.data.iter().map(|color| color.r()).collect();
		assert!(luminances.iter().any(|&value| value < 0.5) && luminances.iter().any(|&value| value > 0.5));
		assert_ne!(render_pattern(&white_noise(4), [DVec2::ZERO, DVec2::splat(4.)], 4, 4).unwrap().data, coarse.data);
	}
}
