//! The kernels rendering each [`GeneratorPattern`] across any area of its space at any resolution.

use crate::{RasterKernel, RasterKernelVisitor, Wgsl, gpu_params};
use glam::{DVec2, UVec2, Vec2};
use graphic_types::generator::{CellularDistanceFunction, CellularReturnType, DomainWarpType, FractalType, GeneratorPattern, NoiseSettings, NoiseType};

/// Hands `visitor` the kernel rendering `pattern` across `area` of its own space, from its minimum to its maximum corner, into a grid of `size` pixels.
pub fn visit_generator_kernel<V: RasterKernelVisitor>(pattern: &GeneratorPattern, area: [DVec2; 2], size: UVec2, visitor: V) -> V::Output {
	let texel = ((area[1] - area[0]).as_vec2() / size.as_vec2()).to_array();
	let corner = area[0].as_vec2().to_array();

	match pattern {
		GeneratorPattern::Noise(settings) => visit_noise_kernel(settings, corner, texel, visitor),
		GeneratorPattern::Mandelbrot => visitor.visit(&Mandelbrot {
			corner: (Vec2::from(corner) * 3. - Vec2::new(2., 1.5)).to_array(),
			texel: (Vec2::from(texel) * 3.).to_array(),
		}),
	}
}

fn visit_noise_kernel<V: RasterKernelVisitor>(settings: &NoiseSettings, corner: [f32; 2], texel: [f32; 2], visitor: V) -> V::Output {
	let NoiseSettings {
		seed,
		scale,
		noise_type,
		domain_warp_type,
		domain_warp_amplitude,
		fractal_type,
		fractal_octaves,
		fractal_lacunarity,
		fractal_gain,
		fractal_weighted_strength,
		fractal_ping_pong_strength,
		cellular_distance_function,
		cellular_return_type,
		cellular_jitter,
	} = *settings;

	if noise_type == NoiseType::WhiteNoise {
		return visitor.visit(&WhiteNoise { corner, texel, seed: seed as u32 });
	}

	let domain_warp_type = match domain_warp_type {
		DomainWarpType::None => None,
		DomainWarpType::OpenSimplex2 => Some(0),
		DomainWarpType::OpenSimplex2Reduced => Some(1),
		DomainWarpType::BasicGrid => Some(2),
	};
	let noise = NoiseState {
		seed: seed as i32,
		frequency: 1. / (scale as f32).max(f32::EPSILON),
		noise_type: match noise_type {
			NoiseType::OpenSimplex2 | NoiseType::WhiteNoise => 0,
			NoiseType::OpenSimplex2S => 1,
			NoiseType::Cellular => 2,
			NoiseType::Perlin => 3,
			NoiseType::ValueCubic => 4,
			NoiseType::Value => 5,
		},
		rotation_type_3d: 0,
		fractal_type: match fractal_type {
			FractalType::None => 0,
			FractalType::FBm => 1,
			FractalType::Ridged => 2,
			FractalType::PingPong => 3,
			FractalType::DomainWarpProgressive => 4,
			FractalType::DomainWarpIndependent => 5,
		},
		octaves: fractal_octaves as i32,
		lacunarity: fractal_lacunarity as f32,
		gain: fractal_gain as f32,
		weighted_strength: fractal_weighted_strength as f32,
		ping_pong_strength: fractal_ping_pong_strength as f32,
		cellular_distance_func: match cellular_distance_function {
			CellularDistanceFunction::Euclidean => 0,
			CellularDistanceFunction::EuclideanSq => 1,
			CellularDistanceFunction::Manhattan => 2,
			CellularDistanceFunction::Hybrid => 3,
		},
		cellular_return_type: match cellular_return_type {
			CellularReturnType::CellValue => 0,
			CellularReturnType::Nearest => 1,
			CellularReturnType::NextNearest => 2,
			CellularReturnType::Average => 3,
			CellularReturnType::Difference => 4,
			CellularReturnType::Product => 5,
			CellularReturnType::Division => 6,
		},
		cellular_jitter_mod: cellular_jitter as f32,
		domain_warp_type: domain_warp_type.unwrap_or(0),
		domain_warp_amp: domain_warp_amplitude as f32,
	};

	visitor.visit(&NoisePattern {
		corner,
		texel,
		domain_warp: domain_warp_type.is_some() && domain_warp_amplitude > 0.,
		noise,
	})
}

gpu_params! {
	#[derive(Clone)]
	struct NoiseState {
		seed: i32,
		frequency: f32,
		noise_type: i32,
		rotation_type_3d: i32,
		fractal_type: i32,
		octaves: i32,
		lacunarity: f32,
		gain: f32,
		weighted_strength: f32,
		ping_pong_strength: f32,
		cellular_distance_func: i32,
		cellular_return_type: i32,
		cellular_jitter_mod: f32,
		domain_warp_type: i32,
		domain_warp_amp: f32,
	}
}

gpu_params! {
	#[derive(Clone)]
	struct NoisePattern {
		corner: [f32; 2],
		texel: [f32; 2],
		domain_warp: bool,
		noise: NoiseState,
	}
}

impl RasterKernel for NoisePattern {
	const INPUTS: usize = 0;
	fn wgsl() -> Wgsl {
		Wgsl::function(concat!(include_str!("wgsl/fast_noise_lite.wgsl"), "\n", include_str!("wgsl/noise_pattern.wgsl")))
	}
}

gpu_params! {
	#[derive(Clone)]
	struct WhiteNoise {
		corner: [f32; 2],
		texel: [f32; 2],
		seed: u32,
	}
}

impl RasterKernel for WhiteNoise {
	const INPUTS: usize = 0;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/white_noise.wgsl"))
	}
}

gpu_params! {
	#[derive(Clone)]
	struct Mandelbrot {
		corner: [f32; 2],
		texel: [f32; 2],
	}
}

impl RasterKernel for Mandelbrot {
	const INPUTS: usize = 0;
	fn wgsl() -> Wgsl {
		Wgsl::body(
			r#"
			let c = vec2<f32>(coord) * vec2<f32>(texel[0], texel[1]) + vec2<f32>(corner[0], corner[1]);
			var z = vec2<f32>(0.);
			for (var iteration = 0u; iteration < 1024u; iteration++) {
				z = vec2<f32>(z.x * z.x - z.y * z.y, 2. * z.x * z.y) + c;
				let magnitude_squared = dot(z, z);
				if magnitude_squared > 65536. {
					let smooth_iteration = f32(iteration) + 1. - log2(0.5 * log2(magnitude_squared));
					return vec4<f32>(vec3<f32>(clamp(smooth_iteration / 255., 0., 1.)), 1.);
				}
			}
			return vec4<f32>(1.);
			"#,
		)
	}
}
