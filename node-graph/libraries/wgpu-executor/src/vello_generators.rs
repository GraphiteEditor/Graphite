//! The GPU pass that renders tiles of generators' patterns, matching the CPU's renders of them.

use crate::vello_render::TEXTURE_FORMAT;
use bytemuck::{Pod, Zeroable};
use glam::DVec2;
use graphic_types::generator::{
	CellularDistanceFunction, CellularReturnType, DomainWarpType, FractalType, GeneratorPattern, MandelbrotExterior, MandelbrotFractal, MandelbrotInterior, NoiseSettings, NoiseType,
};
use raster_kernels::mandelbrot::{MandelbrotSetup, ReferenceOrbit, resolves_in_32_bits};
use std::iter::zip;
use std::sync::OnceLock;
use wgpu::util::DeviceExt;

/// The reference orbit's values in each row of its texture, matching the Mandelbrot shader's `REFERENCE_ROW_LENGTH`.
const REFERENCE_ROW_LENGTH: usize = 1024;

/// The mip levels of a tile's texture. Tiles are drawn at most √2 times smaller than their pixels in an evenly scaled view,
/// so these reach views stretched up to 32 times more along one axis than the other.
const TILE_MIP_LEVELS: u32 = 4;

/// The shader of each kind of pattern, compiled the first time a generator with it is drawn, so a document compiles only those it uses.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Shader {
	Noise,
	WhiteNoise,
	Mandelbrot,
}

impl Shader {
	const ALL: [Self; 3] = [Self::Noise, Self::WhiteNoise, Self::Mandelbrot];

	fn source(self) -> String {
		let common = include_str!("vello_generators.wgsl");
		match self {
			Self::Noise => [fastnoise_lite::WGSL_SOURCE, common, include_str!("vello_generators_noise.wgsl")].concat(),
			Self::WhiteNoise => [common, include_str!("vello_generators_white_noise.wgsl")].concat(),
			Self::Mandelbrot => [common, include_str!("vello_generators_mandelbrot.wgsl")].concat(),
		}
	}
}

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

/// The noise shaders' parameters, laid out so each one's `Parameters` struct reads the part it needs.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
struct NoiseParameters {
	corner: [f32; 2],
	texel: [f32; 2],
	domain_warp: u32,
	_padding: [u32; 3],
	noise: NoiseState,
	_end_padding: u32,
}

/// The settings of a Mandelbrot-style fractal, laid out to match the Mandelbrot shader's `Mandelbrot` struct.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
struct MandelbrotState {
	delta_corner: [f32; 2],
	start_constant: [f32; 2],
	added_constant: [f32; 2],
	start_slope: f32,
	added_slope: f32,
	fractal: u32,
	exterior: u32,
	interior: u32,
	iterations: u32,
	power: f32,
	integer_power: u32,
	frequency: f32,
	bailout_squared: f32,
	log_bailout: f32,
	reference_length: u32,
	interior_test: u32,
	_padding: u32,
}

/// The Mandelbrot shader's parameters, laid out to match its `Parameters` struct.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
struct MandelbrotParameters {
	corner: [f32; 2],
	texel: [f32; 2],
	mandelbrot: MandelbrotState,
}

impl MandelbrotState {
	/// The state of a tile with its top left corner at `corner` of the complex plane, following `reference` if there is one, and testing for the cardioid and its
	/// left bulb with `interior_test`.
	fn new(setup: &MandelbrotSetup, corner: DVec2, reference: Option<&ReferenceOrbit>, interior_test: bool) -> Self {
		Self {
			delta_corner: reference.map_or(DVec2::ZERO, |reference| corner - reference.point).as_vec2().to_array(),
			start_constant: setup.start_constant.as_vec2().to_array(),
			added_constant: setup.added_constant.as_vec2().to_array(),
			start_slope: setup.start_slope as f32,
			added_slope: setup.added_slope as f32,
			fractal: match setup.fractal {
				MandelbrotFractal::ClassicMandelbrot => 0,
				MandelbrotFractal::BurningShip => 1,
				MandelbrotFractal::Tricorn => 2,
			},
			exterior: match setup.exterior {
				MandelbrotExterior::SmoothEscape => 0,
				MandelbrotExterior::DiscreteEscape => 4,
				MandelbrotExterior::Proximity => 1,
				MandelbrotExterior::Tendrils => 2,
				MandelbrotExterior::Rays => 3,
				MandelbrotExterior::ClosestApproach => 5,
				MandelbrotExterior::Flat => 6,
				MandelbrotExterior::Angle => 7,
				MandelbrotExterior::ClosestIteration => 8,
			},
			interior: match setup.interior {
				MandelbrotInterior::Flat => 0,
				MandelbrotInterior::FinalDistance => 1,
				MandelbrotInterior::Period => 2,
				MandelbrotInterior::ClosestApproach => 3,
				MandelbrotInterior::ClosestIteration => 4,
				MandelbrotInterior::FinalAngle => 5,
				MandelbrotInterior::Radius => 6,
				MandelbrotInterior::Angle => 7,
				MandelbrotInterior::Rays => 9,
				MandelbrotInterior::SmoothSettling => 8,
				MandelbrotInterior::DiscreteSettling => 10,
				MandelbrotInterior::Proximity => 11,
			},
			iterations: setup.iterations,
			power: setup.power as f32,
			integer_power: setup.integer_power.unwrap_or(0),
			frequency: setup.frequency,
			bailout_squared: (setup.bailout * setup.bailout) as f32,
			log_bailout: setup.bailout.ln() as f32,
			reference_length: reference.map_or(0, |reference| reference.values.len() as u32),
			interior_test: interior_test as u32,
			_padding: 0,
		}
	}
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

/// What a tile's draw needs: the shader drawing it, its parameters laid out for that shader, and the Mandelbrot reference orbit its points follow, if any.
struct Tile {
	shader: Shader,
	parameters: Vec<u8>,
	reference: Option<ReferenceOrbit>,
}

impl Tile {
	/// The tile of `pattern` with its top left corner at `corner` of the pattern's space and pixels `texel` units across.
	fn new(pattern: &GeneratorPattern, corner: DVec2, texel: f64, pixels: u32) -> Self {
		match pattern {
			GeneratorPattern::Noise(settings) => {
				let (noise, domain_warp) = NoiseState::new(settings);
				let parameters = NoiseParameters {
					corner: corner.as_vec2().to_array(),
					texel: [texel as f32; 2],
					domain_warp: domain_warp as u32,
					noise,
					..Default::default()
				};

				let shader = if settings.noise_type == NoiseType::WhiteNoise { Shader::WhiteNoise } else { Shader::Noise };
				Self {
					shader,
					parameters: bytemuck::bytes_of(&parameters).to_vec(),
					reference: None,
				}
			}
			GeneratorPattern::Mandelbrot(settings) => {
				let setup = MandelbrotSetup::new(settings);

				// Sampled in the complex plane rather than the pattern's space, so the shader's coordinates there round only once
				let (corner, texel) = (setup.plane_point(corner), texel / setup.scale);
				let area = [corner, corner + texel * pixels as f64];

				// A reference orbit escaping at once leaves points to escape at once too, however imprecisely 32-bit floats place them
				let reference = setup.reference_orbit(area, texel).filter(|reference| reference.values.len() >= 2);
				let interior_test = setup.has_interior_test() && resolves_in_32_bits(area, texel);

				let parameters = MandelbrotParameters {
					corner: corner.as_vec2().to_array(),
					texel: [texel as f32; 2],
					mandelbrot: MandelbrotState::new(&setup, corner, reference.as_ref(), interior_test),
				};
				Self {
					shader: Shader::Mandelbrot,
					parameters: bytemuck::bytes_of(&parameters).to_vec(),
					reference,
				}
			}
		}
	}
}

/// The texture holding a reference orbit's values and their raisings for the Mandelbrot shader, in rows of `REFERENCE_ROW_LENGTH`, or one empty texel without one.
fn reference_texture(device: &wgpu::Device, queue: &wgpu::Queue, reference: Option<&ReferenceOrbit>) -> wgpu::Texture {
	let (values, raised_values) = reference.map_or((&[][..], &[][..]), |reference| (&reference.values[..], &reference.raised_values[..]));
	let (width, height) = (values.len().clamp(1, REFERENCE_ROW_LENGTH), values.len().div_ceil(REFERENCE_ROW_LENGTH).max(1));

	let mut texels = vec![[0_f32; 4]; width * height];
	for (texel, (value, raised)) in texels.iter_mut().zip(zip(values, raised_values)) {
		*texel = [value.x, value.y, raised.x, raised.y];
	}

	device.create_texture_with_data(
		queue,
		&wgpu::TextureDescriptor {
			label: Some("vello_generator_reference_orbit"),
			size: wgpu::Extent3d {
				width: width as u32,
				height: height as u32,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba32Float,
			usage: wgpu::TextureUsages::TEXTURE_BINDING,
			view_formats: &[],
		},
		wgpu::util::TextureDataOrder::LayerMajor,
		bytemuck::cast_slice(&texels),
	)
}

/// The pipelines of the pass, one for each shader.
#[derive(Default)]
pub(crate) struct GeneratorPasses {
	pipelines: [OnceLock<(wgpu::RenderPipeline, wgpu::BindGroupLayout)>; Shader::ALL.len()],
}

impl GeneratorPasses {
	fn pipeline(&self, device: &wgpu::Device, shader: Shader) -> &(wgpu::RenderPipeline, wgpu::BindGroupLayout) {
		self.pipelines[shader as usize].get_or_init(|| {
			// The vertex stage gets a module of its own, since GLSL translation writes all of a module's FastNoise Lite into every stage
			let vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
				label: Some("vello_generators_vertex"),
				source: wgpu::ShaderSource::Wgsl(include_str!("vello_generators.wgsl").into()),
			});
			let fragment_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
				label: Some("vello_generators"),
				source: wgpu::ShaderSource::Wgsl(shader.source().into()),
			});

			// Every shader reads its parameters, and the Mandelbrot shader its reference orbit too
			let parameters = wgpu::BindGroupLayoutEntry {
				binding: 0,
				visibility: wgpu::ShaderStages::FRAGMENT,
				ty: wgpu::BindingType::Buffer {
					ty: wgpu::BufferBindingType::Uniform,
					has_dynamic_offset: false,
					min_binding_size: None,
				},
				count: None,
			};
			let reference_orbit = wgpu::BindGroupLayoutEntry {
				binding: 1,
				visibility: wgpu::ShaderStages::FRAGMENT,
				ty: wgpu::BindingType::Texture {
					sample_type: wgpu::TextureSampleType::Float { filterable: false },
					view_dimension: wgpu::TextureViewDimension::D2,
					multisampled: false,
				},
				count: None,
			};
			let entries: &[_] = if shader == Shader::Mandelbrot { &[parameters, reference_orbit] } else { &[parameters] };

			let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
				label: Some("vello_generators_bind_group_layout"),
				entries,
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
					module: &vertex_module,
					entry_point: Some("vertex"),
					compilation_options: Default::default(),
					buffers: &[],
				},
				fragment: Some(wgpu::FragmentState {
					module: &fragment_module,
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

	/// A texture for square tiles `pixels` across to be rendered into, with the mip levels they're drawn from.
	pub(crate) fn tile_texture(device: &wgpu::Device, pixels: u32) -> wgpu::Texture {
		device.create_texture(&wgpu::TextureDescriptor {
			label: Some("vello_generator_tile"),
			size: wgpu::Extent3d {
				width: pixels,
				height: pixels,
				depth_or_array_layers: 1,
			},
			mip_level_count: TILE_MIP_LEVELS,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: TEXTURE_FORMAT,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
			view_formats: &[],
		})
	}

	/// Records the rendering of a square tile of `pattern` into the top mip level of `texture`, with its top left corner at `corner` of the pattern's space and pixels `texel` units across.
	#[allow(clippy::too_many_arguments)]
	pub(crate) fn render_tile(&self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, texture: &wgpu::Texture, pattern: &GeneratorPattern, corner: DVec2, texel: f64) {
		let tile = Tile::new(pattern, corner, texel, texture.width());
		let (pipeline, bind_group_layout) = self.pipeline(device, tile.shader);

		let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("vello_generator_parameters"),
			contents: &tile.parameters,
			usage: wgpu::BufferUsages::UNIFORM,
		});
		let reference_view = (tile.shader == Shader::Mandelbrot).then(|| reference_texture(device, queue, tile.reference.as_ref()).create_view(&wgpu::TextureViewDescriptor::default()));
		let mut entries = vec![wgpu::BindGroupEntry {
			binding: 0,
			resource: buffer.as_entire_binding(),
		}];
		if let Some(reference_view) = &reference_view {
			entries.push(wgpu::BindGroupEntry {
				binding: 1,
				resource: wgpu::BindingResource::TextureView(reference_view),
			});
		}
		let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("vello_generator_bind_group"),
			layout: bind_group_layout,
			entries: &entries,
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
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Translates `source` to the shading language of every backend, with the GLSL ES the web build's WebGL2 backend runs written for each entry point.
	fn translate(source: &str, entry_points: &[(naga::ShaderStage, &str)]) {
		let module = naga::front::wgsl::parse_str(source).unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
		let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
			.validate(&module)
			.unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));

		// GLSL ES 3.0
		let options = naga::back::glsl::Options {
			version: naga::back::glsl::Version::Embedded { version: 300, is_webgl: true },
			..Default::default()
		};
		for &(shader_stage, entry_point) in entry_points {
			let pipeline_options = naga::back::glsl::PipelineOptions {
				shader_stage,
				entry_point: entry_point.to_string(),
				multiview: None,
			};
			let mut glsl = String::new();
			let written = naga::back::glsl::Writer::new(&mut glsl, &module, &info, &options, &pipeline_options, naga::proc::BoundsCheckPolicies::default()).and_then(|mut writer| writer.write());
			assert!(written.is_ok(), "the {entry_point} entry point doesn't translate to GLSL: {written:?}");
		}

		// HLSL, which Direct3D compiles natively and ANGLE's D3D11 backend compiles by way of GLSL
		let mut hlsl = String::new();
		let written = naga::back::hlsl::Writer::new(&mut hlsl, &Default::default(), &Default::default()).write(&module, &info, None);
		assert!(written.is_ok(), "the shader doesn't translate to HLSL: {written:?}");

		// MSL for Metal, without the bindings a pipeline's layout would map, and SPIR-V for Vulkan
		let options = naga::back::msl::Options {
			fake_missing_bindings: true,
			..Default::default()
		};
		let written = naga::back::msl::write_string(&module, &info, &options, &Default::default());
		assert!(written.is_ok(), "the shader doesn't translate to MSL: {:?}", written.err());

		let written = naga::back::spv::write_vec(&module, &info, &Default::default(), None);
		assert!(written.is_ok(), "the shader doesn't translate to SPIR-V: {:?}", written.err());
	}

	// Each module on its own, as a pipeline compiles it
	#[test]
	fn every_shader_translates_for_every_backend() {
		translate(include_str!("vello_generators.wgsl"), &[(naga::ShaderStage::Vertex, "vertex")]);
		for shader in Shader::ALL {
			translate(&shader.source(), &[(naga::ShaderStage::Fragment, "generate")]);
		}
	}

	/// The Mandelbrot node's defaults, with `power`, at a scale placing the pattern's space on the complex plane.
	fn mandelbrot(power: f64) -> GeneratorPattern {
		match GeneratorPattern::decode(graphic_types::generator::Generator::Mandelbrot, &|_| None) {
			GeneratorPattern::Mandelbrot(settings) => GeneratorPattern::Mandelbrot(graphic_types::generator::MandelbrotSettings { power, scale: 1., ..settings }),
			GeneratorPattern::Noise(_) => unreachable!(),
		}
	}

	#[test]
	fn tiles_too_fine_for_32_bit_floats_follow_a_reference_orbit_at_any_power() {
		let corner = DVec2::new(-1.25, 0.);
		let (shallow, deep) = (2_f64.powi(-8), 2_f64.powi(-30));

		// Integer powers follow a reference orbit at any depth, while other powers are iterated directly where they can be
		assert!(Tile::new(&mandelbrot(2.), corner, shallow, 260).reference.is_some());
		assert!(Tile::new(&mandelbrot(2.), corner, deep, 260).reference.is_some());
		assert!(Tile::new(&mandelbrot(2.5), corner, shallow, 260).reference.is_none());
		assert!(Tile::new(&mandelbrot(2.5), corner, deep, 260).reference.is_some());
	}
}
