//! The GPU pass that renders tiles of generators' patterns, matching the CPU's renders of them.

use crate::vello_render::TEXTURE_FORMAT;
use bytemuck::{Pod, Zeroable};
use glam::DVec2;
use graphic_types::generator::{CellularDistanceFunction, CellularReturnType, DomainWarpType, FractalType, GeneratorPattern, NoiseSettings, NoiseType};
use std::sync::OnceLock;
use wgpu::util::DeviceExt;

/// How many of a 32-bit float's steps one pixel must span for the Mandelbrot set to render on the GPU, past which its deep zooms render on the CPU in 64 bits.
const MIN_MANDELBROT_PIXEL_STEPS: f64 = 64.;

/// The shader's patterns, matching its `PATTERN_` constants.
const PATTERN_NOISE: u32 = 0;
const PATTERN_WHITE_NOISE: u32 = 1;
const PATTERN_MANDELBROT: u32 = 2;

/// FastNoiseLite's `fnl_state`, laid out to match the shader's.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
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

/// The shader's parameters, laid out to match its `Parameters` struct.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
struct Parameters {
	corner: [f32; 2],
	texel: [f32; 2],
	pattern: u32,
	domain_warp: u32,
	_padding: [u32; 2],
	noise: NoiseState,
	_end_padding: u32,
}

impl NoiseState {
	/// The state FastNoiseLite's CPU sampler is given for the same settings, with whether it warps its domain.
	fn new(settings: &NoiseSettings) -> (Self, bool) {
		let domain_warp_type = match settings.domain_warp_type {
			DomainWarpType::None => None,
			DomainWarpType::OpenSimplex2 => Some(0),
			DomainWarpType::OpenSimplex2Reduced => Some(1),
			DomainWarpType::BasicGrid => Some(2),
		};
		let state = Self {
			seed: settings.seed as i32,
			frequency: 1. / (settings.scale as f32).max(f32::EPSILON),
			noise_type: match settings.noise_type {
				NoiseType::OpenSimplex2 | NoiseType::WhiteNoise => 0,
				NoiseType::OpenSimplex2S => 1,
				NoiseType::Cellular => 2,
				NoiseType::Perlin => 3,
				NoiseType::ValueCubic => 4,
				NoiseType::Value => 5,
			},
			rotation_type_3d: 0,
			fractal_type: match settings.fractal_type {
				FractalType::None => 0,
				FractalType::FBm => 1,
				FractalType::Ridged => 2,
				FractalType::PingPong => 3,
				FractalType::DomainWarpProgressive => 4,
				FractalType::DomainWarpIndependent => 5,
			},
			octaves: settings.fractal_octaves as i32,
			lacunarity: settings.fractal_lacunarity as f32,
			gain: settings.fractal_gain as f32,
			weighted_strength: settings.fractal_weighted_strength as f32,
			ping_pong_strength: settings.fractal_ping_pong_strength as f32,
			cellular_distance_func: match settings.cellular_distance_function {
				CellularDistanceFunction::Euclidean => 0,
				CellularDistanceFunction::EuclideanSq => 1,
				CellularDistanceFunction::Manhattan => 2,
				CellularDistanceFunction::Hybrid => 3,
			},
			cellular_return_type: match settings.cellular_return_type {
				CellularReturnType::CellValue => 0,
				CellularReturnType::Nearest => 1,
				CellularReturnType::NextNearest => 2,
				CellularReturnType::Average => 3,
				CellularReturnType::Difference => 4,
				CellularReturnType::Product => 5,
				CellularReturnType::Division => 6,
			},
			cellular_jitter_mod: settings.cellular_jitter as f32,
			domain_warp_type: domain_warp_type.unwrap_or(0),
			domain_warp_amp: settings.domain_warp_amplitude as f32,
		};

		(state, domain_warp_type.is_some() && settings.domain_warp_amplitude > 0.)
	}
}

impl Parameters {
	/// The parameters of a tile of `pattern` whose top left corner lies at `corner` of the pattern's space, with pixels `texel` units across.
	/// Returns `None` if the tile's pixels are too fine for 32-bit floats to place.
	fn new(pattern: &GeneratorPattern, corner: DVec2, texel: f64, pixels: u32) -> Option<Self> {
		let (pattern, corner, texel, noise, domain_warp) = match pattern {
			GeneratorPattern::Noise(settings) => {
				let (noise, domain_warp) = NoiseState::new(settings);
				let pattern = if settings.noise_type == NoiseType::WhiteNoise { PATTERN_WHITE_NOISE } else { PATTERN_NOISE };
				(pattern, corner, texel, noise, domain_warp)
			}
			GeneratorPattern::Mandelbrot => {
				// Sampled in the complex plane rather than its unit square, so the shader's coordinates there round only once
				let (corner, texel) = (corner * 3. - DVec2::new(2., 1.5), texel * 3.);
				let farthest = corner.abs().max((corner + texel * pixels as f64).abs()).max_element();
				if texel < farthest * f32::EPSILON as f64 * MIN_MANDELBROT_PIXEL_STEPS {
					return None;
				}
				(PATTERN_MANDELBROT, corner, texel, NoiseState::default(), false)
			}
		};

		Some(Self {
			corner: corner.as_vec2().to_array(),
			texel: [texel as f32; 2],
			pattern,
			domain_warp: domain_warp as u32,
			noise,
			..Default::default()
		})
	}
}

/// The pipeline of the pass, built the first time a generator is drawn so documents without one don't compile its shader.
#[derive(Default)]
pub(crate) struct GeneratorPasses {
	pipeline: OnceLock<(wgpu::RenderPipeline, wgpu::BindGroupLayout)>,
}

impl GeneratorPasses {
	fn pipeline(&self, device: &wgpu::Device) -> &(wgpu::RenderPipeline, wgpu::BindGroupLayout) {
		self.pipeline.get_or_init(|| {
			let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
				label: Some("vello_generators"),
				source: wgpu::ShaderSource::Wgsl(concat!(include_str!("fast_noise_lite.wgsl"), include_str!("vello_generators.wgsl")).into()),
			});

			let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
				label: Some("vello_generators_bind_group_layout"),
				entries: &[wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Uniform,
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				}],
			});
			let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
				label: Some("vello_generators_pipeline_layout"),
				bind_group_layouts: &[Some(&bind_group_layout)],
				immediate_size: 0,
			});

			let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
				label: Some("vello_generators"),
				layout: Some(&pipeline_layout),
				vertex: wgpu::VertexState {
					module: &shader,
					entry_point: Some("vertex"),
					compilation_options: Default::default(),
					buffers: &[],
				},
				fragment: Some(wgpu::FragmentState {
					module: &shader,
					entry_point: Some("generate"),
					compilation_options: Default::default(),
					targets: &[Some(wgpu::ColorTargetState {
						format: TEXTURE_FORMAT,
						blend: None,
						write_mask: wgpu::ColorWrites::ALL,
					})],
				}),
				primitive: wgpu::PrimitiveState {
					topology: wgpu::PrimitiveTopology::TriangleList,
					..Default::default()
				},
				depth_stencil: None,
				multisample: wgpu::MultisampleState::default(),
				multiview_mask: None,
				cache: None,
			});

			(pipeline, bind_group_layout)
		})
	}

	/// Records the rendering of the top mip level of a square tile of `pattern`, `pixels` across, whose top left corner lies at `corner` of the pattern's space with pixels `texel` units across.
	/// Returns `None` if the GPU can't render the tile as precisely as the CPU, leaving it to the CPU.
	pub(crate) fn render_tile(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, pattern: &GeneratorPattern, corner: DVec2, texel: f64, pixels: u32) -> Option<wgpu::Texture> {
		let parameters = Parameters::new(pattern, corner, texel, pixels)?;
		let (pipeline, bind_group_layout) = self.pipeline(device);

		let texture = device.create_texture(&wgpu::TextureDescriptor {
			label: Some("vello_generator_tile"),
			size: wgpu::Extent3d {
				width: pixels,
				height: pixels,
				depth_or_array_layers: 1,
			},
			mip_level_count: pixels.ilog2() + 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: TEXTURE_FORMAT,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
			view_formats: &[],
		});

		let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("vello_generator_parameters"),
			contents: bytemuck::bytes_of(&parameters),
			usage: wgpu::BufferUsages::UNIFORM,
		});
		let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("vello_generator_bind_group"),
			layout: bind_group_layout,
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: buffer.as_entire_binding(),
			}],
		});

		let view = texture.create_view(&wgpu::TextureViewDescriptor {
			mip_level_count: Some(1),
			..Default::default()
		});
		let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some("vello_generator_tile"),
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view: &view,
				depth_slice: None,
				resolve_target: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
					store: wgpu::StoreOp::Store,
				},
			})],
			depth_stencil_attachment: None,
			timestamp_writes: None,
			occlusion_query_set: None,
			multiview_mask: None,
		});
		pass.set_pipeline(pipeline);
		pass.set_bind_group(0, &bind_group, &[]);
		pass.draw(0..3, 0..1);
		drop(pass);

		Some(texture)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn mandelbrot_tiles_too_fine_for_32_bit_floats_are_left_to_the_cpu() {
		let corner = DVec2::new(0.25, 0.5);
		assert!(Parameters::new(&GeneratorPattern::Mandelbrot, corner, 2_f64.powi(-8), 260).is_some());
		assert!(Parameters::new(&GeneratorPattern::Mandelbrot, corner, 2_f64.powi(-30), 260).is_none());
	}
}
