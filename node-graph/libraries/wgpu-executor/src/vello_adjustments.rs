//! The passes carrying out each run of adjustments. One interpreter shader loops over a pass's steps, calling the step function of each one's kind
//! from `vello_adjustments.wgsl`, while the kinds that compilers struggle to build inside its loop each get a shader and pass of their own. Steps read their
//! settings from a texture, so changing those recompiles nothing.

use crate::vello_effects::{WORKING_FORMAT, full_target_pipeline};
use core_types::graphene_hash::CacheHash;
use graphic_types::Adjustment;
use raster_kernels::adjustment_steps::{AdjustmentSteps, StepKind, shader_constants};
use std::cmp::Reverse;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Mutex, PoisonError};
use wgpu::util::DeviceExt;

/// The step functions that the generated shaders call.
const STEP_FUNCTIONS: &str = include_str!("vello_adjustments.wgsl");

/// Kinds of steps carried out by shaders of their own, one step per pass, since shader compilers struggle with their loops nested in the
/// interpreter's. Inside its loop and `switch`, Gradient Map's and Color Lookup's fail to build in FXC (behind a browser's ANGLE on Windows).
const STANDALONE_KINDS: [StepKind; 2] = [StepKind::GradientMap, StepKind::ColorLookup];

/// How many runs of adjustments keep their encoded and uploaded steps past the render that last used them. Their tables are kept apart.
const MAX_CACHED_RUNS: usize = 8;

/// How many bytes of tables stay uploaded past the render that last used them, the least recently used making way first.
const TABLE_CACHE_BYTES: u64 = 256 << 20;

/// The most texels a run's tables span (128 MiB), past which they are resampled to fit. That's half of what the cache keeps and of the largest buffer
/// wgpu allocates by default, which an upload passes through.
const MAX_TABLE_TEXELS: usize = 1 << 23;

/// The format each pass of a run split across several hands its result on to the next in, where the GPU can render into it, so the split rounds nothing.
/// The passes neither blend into it nor filter it, which 32-bit floats would need optional features for.
const CHAINED_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

/// The shader drawing a pass of adjustments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum AdjustmentShader {
	/// Carries out any number of steps of the kinds outside [`STANDALONE_KINDS`].
	Interpreter,
	/// Carries out one step of a kind in [`STANDALONE_KINDS`].
	Standalone(StepKind),
}

impl AdjustmentShader {
	/// The shader carrying out the steps of these `kinds` in one pass.
	fn for_pass(kinds: &[StepKind]) -> Self {
		match kinds {
			[kind] if STANDALONE_KINDS.contains(kind) => Self::Standalone(*kind),
			_ => Self::Interpreter,
		}
	}

	/// The shader's WGSL: the constants the step functions compare settings against, the step functions, and the entry point carrying out the steps.
	pub(crate) fn source(self) -> String {
		let entry_point = match self {
			Self::Interpreter => interpreter_entry_point(),
			Self::Standalone(kind) => standalone_entry_point(kind),
		};
		format!("{}\n{STEP_FUNCTIONS}\n{entry_point}", shader_constants())
	}
}

/// The steps of each pass carrying out a run with steps of these `kinds`: each step of a standalone kind alone, and the steps between them together. O(n).
pub(crate) fn pass_steps(kinds: &[StepKind]) -> Vec<Range<usize>> {
	let mut passes: Vec<Range<usize>> = Vec::new();

	for (index, kind) in kinds.iter().enumerate() {
		let interpreted = |kind: &StepKind| !STANDALONE_KINDS.contains(kind);
		match passes.last_mut() {
			Some(pass) if interpreted(kind) && interpreted(&kinds[pass.start]) => pass.end = index + 1,
			_ => passes.push(index..index + 1),
		}
	}

	passes
}

/// How a color reaching or leaving a step function is encoded.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Encoding {
	Linear,
	Gamma,
}

/// The step function carrying out a kind of step, with the encodings of the color it reads and returns, or `None` for one that reads either and returns it unchanged.
fn step_function(kind: StepKind) -> (&'static str, Option<[Encoding; 2]>) {
	use Encoding::{Gamma, Linear};

	match kind {
		StepKind::Desaturate => ("desaturate", Some([Linear, Linear])),
		StepKind::GammaCorrection => ("gamma_correction", Some([Linear, Linear])),
		StepKind::MakeOpaque => ("make_opaque", None),
		StepKind::BrightnessContrast => ("brightness_contrast", Some([Gamma, Gamma])),
		StepKind::Levels => ("levels", Some([Gamma, Gamma])),
		StepKind::Curves => ("curves", Some([Gamma, Gamma])),
		StepKind::BlackAndWhite => ("black_and_white", Some([Gamma, Gamma])),
		StepKind::HueSaturation => ("hue_saturation", Some([Gamma, Gamma])),
		StepKind::Invert => ("invert", Some([Gamma, Gamma])),
		StepKind::Threshold => ("threshold", Some([Gamma, Gamma])),
		StepKind::GradientMap => ("gradient_map", Some([Gamma, Linear])),
		StepKind::Vibrance => ("vibrance", Some([Linear, Linear])),
		StepKind::ChannelMixer => ("channel_mixer", Some([Gamma, Gamma])),
		StepKind::SelectiveColor => ("selective_color", Some([Gamma, Gamma])),
		StepKind::Posterize => ("posterize", Some([Gamma, Gamma])),
		StepKind::Exposure => ("exposure", Some([Gamma, Gamma])),
		StepKind::ColorBalance => ("color_balance", Some([Gamma, Gamma])),
		StepKind::PhotoFilter => ("photo_filter", Some([Gamma, Gamma])),
		StepKind::ColorLookup => ("color_lookup", Some([Gamma, Gamma])),
		StepKind::ColorOverlay => ("color_overlay", Some([Gamma, Gamma])),
		StepKind::Solarize => ("solarize", Some([Gamma, Gamma])),
	}
}

/// The interpreter's entry point, with the cases of its `switch` over the kinds of steps in place of `CASES`.
const INTERPRETER: &str = "
@fragment
fn adjust(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	var color = source_color(position);
	if color.a <= 0. {
		return vec4<f32>(0.);
	}

	// Whether the color is in linear light rather than gamma-encoded, starting in the source's encoding
	var linear = encodings.straight_linear_source != 0u;

	// The step count comes first, then each step's header followed by its parameters
	let step_count = u32(step_texel(0u).x);
	var header = 1u;
	for (var index = 0u; index < step_count; index++) {
		let at = header + 1u;
		switch u32(step_texel(header).z) {
CASES
			default: {}
		}
		header += step_length(header);
	}

	return result_pixel(color, linear);
}
";

/// The entry point of a standalone kind's shader, with its step in place of `STEP`.
const STANDALONE: &str = "
@fragment
fn adjust(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	var color = source_color(position);
	if color.a <= 0. {
		return vec4<f32>(0.);
	}

	// Whether the color is in linear light rather than gamma-encoded, starting in the source's encoding
	var linear = encodings.straight_linear_source != 0u;

	// The step count comes first, then the step's header followed by its parameters
STEP
	return result_pixel(color, linear);
}
";

/// The interpreter's entry point, with a case for each kind of step it carries out.
fn interpreter_entry_point() -> String {
	let mut cases = String::new();

	for kind in StepKind::ALL.into_iter().filter(|kind| !STANDALONE_KINDS.contains(kind)) {
		cases += &format!("\t\t\tcase {}u: {{\n", kind.code());
		cases += &step_call(kind, "at", "\t\t\t\t");
		cases += "\t\t\t}\n";
	}

	INTERPRETER.replace("CASES\n", &cases)
}

/// The entry point of the shader carrying out a step of the standalone `kind`.
fn standalone_entry_point(kind: StepKind) -> String {
	STANDALONE.replace("STEP\n", &step_call(kind, "2u", "\t"))
}

/// The lines calling the function of a step of `kind` with its parameters `at`: first converting the color into the encoding the function reads
/// if it's in the other one, then ending the pass if that left the pixel fully transparent, which holds no color for the steps after to adjust.
fn step_call(kind: StepKind, at: &str, indent: &str) -> String {
	let (function, encodings) = step_function(kind);
	let mut lines = String::new();

	if let Some([reads, _]) = encodings {
		let (condition, conversion) = match reads {
			Encoding::Linear => ("!linear", "from_gamma"),
			Encoding::Gamma => ("linear", "to_gamma"),
		};
		lines += &format!("{indent}if {condition} {{\n{indent}\tcolor = {conversion}(color);\n{indent}}}\n");
	}
	lines += &format!("{indent}color = {function}(color, {at});\n");

	// Only these kinds can lower alpha
	if matches!(kind, StepKind::Levels | StepKind::Curves) {
		lines += &format!("{indent}if color.a <= 0. {{\n{indent}\treturn vec4<f32>(0.);\n{indent}}}\n");
	}

	if let Some([_, returns]) = encodings {
		lines += &format!("{indent}linear = {};\n", returns == Encoding::Linear);
	}

	lines
}

/// How a pass of adjustments reads its source and writes its result: as straight linear color, or else as premultiplied gamma-encoded color.
/// Straight linear color in 32-bit floats holds the CPU's values exactly, which premultiplying would round away at partial alpha.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PassEncodings {
	pub(crate) straight_linear_source: bool,
	pub(crate) straight_linear_result: bool,
}

impl PassEncodings {
	/// The index of these encodings among every combination of them.
	fn index(self) -> usize {
		usize::from(self.straight_linear_source) | (usize::from(self.straight_linear_result) << 1)
	}
}

/// The bindings of the adjustment pass and its pipelines, each built the first time a pass draws with its shader into its format.
pub(crate) struct AdjustmentShaders {
	pub(crate) bind_group_layout: wgpu::BindGroupLayout,
	pipeline_layout: wgpu::PipelineLayout,
	/// Bound in place of the tables of a run that reads none.
	pub(crate) no_tables: wgpu::Texture,
	/// The format the passes of a run split across several hand their results on in: [`CHAINED_FORMAT`] where the GPU renders into it, or else [`WORKING_FORMAT`], or else 8 bits.
	pub(crate) chained_format: wgpu::TextureFormat,
	/// The uniform buffer for each combination of [`PassEncodings`], by its index.
	encodings: [wgpu::Buffer; 4],
	pipelines: Mutex<HashMap<(AdjustmentShader, wgpu::TextureFormat), wgpu::RenderPipeline>>,
}

impl AdjustmentShaders {
	pub(crate) fn new(device: &wgpu::Device, adapter: &wgpu::Adapter) -> Self {
		// The pixels to adjust, then the pass's steps and the run's tables, which hold 32-bit floats that can be loaded but not filtered, then the encodings
		let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
			binding,
			visibility: wgpu::ShaderStages::FRAGMENT,
			ty: wgpu::BindingType::Texture {
				sample_type: wgpu::TextureSampleType::Float { filterable: false },
				view_dimension: wgpu::TextureViewDimension::D2,
				multisampled: false,
			},
			count: None,
		};
		let encodings_entry = wgpu::BindGroupLayoutEntry {
			binding: 3,
			visibility: wgpu::ShaderStages::FRAGMENT,
			ty: wgpu::BindingType::Buffer {
				ty: wgpu::BufferBindingType::Uniform,
				has_dynamic_offset: false,
				min_binding_size: None,
			},
			count: None,
		};
		let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("vello_adjustments_bind_group_layout"),
			entries: &[texture_entry(0), texture_entry(1), texture_entry(2), encodings_entry],
		});
		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("vello_adjustments_pipeline_layout"),
			bind_group_layouts: &[Some(&bind_group_layout)],
			immediate_size: 0,
		});

		let no_tables = device.create_texture(&texels_texture_descriptor("vello_adjustments_no_tables", 1, 1));

		// Adjusted images chain their passes even on GPUs that can't render into either float format, where effects are left out
		let chained_usages = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
		let renders = |format: wgpu::TextureFormat| adapter.get_texture_format_features(format).allowed_usages.contains(chained_usages);
		let chained_format = [CHAINED_FORMAT, WORKING_FORMAT].into_iter().find(|&format| renders(format)).unwrap_or(wgpu::TextureFormat::Rgba8Unorm);

		let encodings = std::array::from_fn(|index| {
			let flags = [(index & 1) as u32, (index >> 1) as u32, 0, 0];
			device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some("vello_adjustments_encodings"),
				contents: bytemuck::bytes_of(&flags),
				usage: wgpu::BufferUsages::UNIFORM,
			})
		});

		Self {
			bind_group_layout,
			pipeline_layout,
			no_tables,
			chained_format,
			encodings,
			pipelines: Mutex::default(),
		}
	}

	/// The uniform buffer telling a pass these `encodings`.
	pub(crate) fn encodings_buffer(&self, encodings: PassEncodings) -> &wgpu::Buffer {
		&self.encodings[encodings.index()]
	}

	/// The encodings for the pass at `index` of `count` handing on what it adjusts, reading a straight linear source first if `straight_linear_source`.
	/// The passes between hand on straight linear color where the chained format holds it in 32-bit floats, and the last writes premultiplied gamma-encoded color.
	pub(crate) fn pass_encodings(&self, index: usize, count: usize, straight_linear_source: bool) -> PassEncodings {
		let chained_straight_linear = self.chained_format == CHAINED_FORMAT;
		PassEncodings {
			straight_linear_source: if index == 0 { straight_linear_source } else { chained_straight_linear },
			straight_linear_result: index + 1 < count && chained_straight_linear,
		}
	}

	/// The pipeline drawing passes with the `shader` into a texture of `format`, built if none has yet.
	pub(crate) fn pipeline(&self, device: &wgpu::Device, shader: AdjustmentShader, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
		let mut pipelines = self.pipelines.lock().unwrap_or_else(PoisonError::into_inner);

		(pipelines.entry((shader, format)).or_insert_with(|| {
			let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
				label: Some("vello_adjustments"),
				source: wgpu::ShaderSource::Wgsl(shader.source().into()),
			});
			full_target_pipeline(device, &self.pipeline_layout, &module, "adjust", format)
		}))
		.clone()
	}

	#[cfg(test)]
	pub(crate) fn cached_pipelines(&self) -> usize {
		self.pipelines.lock().unwrap_or_else(PoisonError::into_inner).len()
	}
}

/// A texture of 32-bit float texels that shaders read by index, filling its rows in order.
fn texels_texture_descriptor(label: &'static str, width: u32, height: u32) -> wgpu::TextureDescriptor<'static> {
	wgpu::TextureDescriptor {
		label: Some(label),
		size: wgpu::Extent3d {
			width,
			height,
			depth_or_array_layers: 1,
		},
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: wgpu::TextureFormat::Rgba32Float,
		usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
		view_formats: &[],
	}
}

/// A texture holding `texels` in rows as wide as the GPU allows, the last padded out, or `None` if they need more rows than it allows too.
fn upload_texels(device: &wgpu::Device, queue: &wgpu::Queue, label: &'static str, texels: &[[f32; 4]]) -> Option<wgpu::Texture> {
	let max_dimension = device.limits().max_texture_dimension_2d as usize;
	let width = texels.len().clamp(1, max_dimension);
	let height = texels.len().div_ceil(width).max(1);
	if height > max_dimension {
		return None;
	}

	let texture = device.create_texture(&texels_texture_descriptor(label, width as u32, height as u32));
	let write_rows = |first_row: usize, rows: usize, data: &[[f32; 4]]| {
		let texture_rows = wgpu::TexelCopyTextureInfo {
			texture: &texture,
			mip_level: 0,
			origin: wgpu::Origin3d { x: 0, y: first_row as u32, z: 0 },
			aspect: wgpu::TextureAspect::All,
		};
		let layout = wgpu::TexelCopyBufferLayout {
			offset: 0,
			bytes_per_row: Some((width * size_of::<[f32; 4]>()) as u32),
			rows_per_image: Some(rows as u32),
		};
		let extent = wgpu::Extent3d {
			width: width as u32,
			height: rows as u32,
			depth_or_array_layers: 1,
		};
		queue.write_texture(texture_rows, bytemuck::cast_slice(data), layout, extent);
	};

	// The full rows straight from the texels, then a copy of only the last row's, padded out
	let full_rows = texels.len() / width;
	if full_rows > 0 {
		write_rows(0, full_rows, &texels[..full_rows * width]);
	}
	let last_row = &texels[full_rows * width..];
	if !last_row.is_empty() {
		let mut padded = last_row.to_vec();
		padded.resize(width, [0.; 4]);
		write_rows(full_rows, 1, &padded);
	}

	Some(texture)
}

/// What a pass carrying out adjustments reads: its steps and any tables they read, with the shader carrying them out.
pub(crate) struct PreparedPass {
	pub(crate) shader: AdjustmentShader,
	pub(crate) steps: wgpu::Texture,
	pub(crate) tables: Option<wgpu::Texture>,
}

/// The shader and the uploaded steps of each pass carrying out a run with these `steps`, each pass's texels starting with its step count,
/// or `None` if any holds more than the GPU can read.
fn upload_passes(device: &wgpu::Device, queue: &wgpu::Queue, steps: &AdjustmentSteps) -> Option<Vec<(AdjustmentShader, wgpu::Texture)>> {
	(pass_steps(steps.kinds()).into_iter())
		.map(|pass| {
			let shader = AdjustmentShader::for_pass(&steps.kinds()[pass.clone()]);
			let texels: Vec<[f32; 4]> = std::iter::once([pass.len() as f32, 0., 0., 0.]).chain(steps.step_texels(pass).iter().copied()).collect();
			upload_texels(device, queue, "vello_adjustments_steps", &texels).map(|texture| (shader, texture))
		})
		.collect()
}

/// The encoded and uploaded runs of adjustments that recent renders drew, kept so a run drawn again skips encoding and uploading,
/// and the uploaded tables, kept apart from the runs so a run with only its other settings changed reads its tables as they are.
#[derive(Default)]
pub(crate) struct AdjustmentUploads {
	runs: Vec<CachedRun>,
	tables: Vec<CachedTables>,
}

/// A run of adjustments as encoded, with the shader and uploaded steps of each pass carrying it out, or `None` for a run that holds more than the GPU can read.
struct CachedRun {
	hash: u64,
	steps: AdjustmentSteps,
	passes: Option<Vec<(AdjustmentShader, wgpu::Texture)>>,
	last_used: u64,
}

/// The tables of a run as uploaded, shared by every run reading the same ones, or `None` in place of the upload for tables too large for the GPU even resampled.
struct CachedTables {
	key: u64,
	uploaded: Option<wgpu::Texture>,
	bytes: u64,
	last_used: u64,
}

/// Identifies a run of adjustments by its settings.
pub(crate) fn adjustment_run_hash(run: &[&Adjustment]) -> u64 {
	let mut hasher = std::hash::DefaultHasher::new();
	run.iter().for_each(|adjustment| adjustment.cache_hash(&mut hasher));
	std::hash::Hasher::finish(&hasher)
}

impl AdjustmentUploads {
	/// What each pass carrying out `run` reads in turn, encoding and uploading only what no recent render has. A run that changes no color
	/// or that holds more than the GPU can read takes no passes. Only the 64-bit hashes of the runs and of the tables' content tell them apart.
	pub(crate) fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, run: &[&Adjustment], render_count: u64) -> Vec<PreparedPass> {
		let hash = adjustment_run_hash(run);
		let index = self.runs.iter().position(|cached| cached.hash == hash).unwrap_or_else(|| {
			let steps = AdjustmentSteps::encode(run);
			let passes = upload_passes(device, queue, &steps);
			if passes.is_none() {
				log::error!("Adjustments hold more settings than the GPU can read, so they're left out");
			}

			self.runs.push(CachedRun {
				hash,
				steps,
				passes,
				last_used: render_count,
			});
			self.runs.len() - 1
		});
		let cached_run = &mut self.runs[index];
		cached_run.last_used = render_count;
		let Some(passes) = cached_run.passes.clone() else { return Vec::new() };
		if passes.is_empty() {
			return Vec::new();
		}

		let tables = match cached_run.steps.table_key() {
			Some(key) => {
				let index = self.tables.iter().position(|cached| cached.key == key).unwrap_or_else(|| {
					// As many texels as one texture holds and one upload carries
					let limits = device.limits();
					let max_dimension = limits.max_texture_dimension_2d as usize;
					let max_uploaded = (limits.max_buffer_size / size_of::<[f32; 4]>() as u64) as usize;
					let texels = cached_run.steps.table_texels(MAX_TABLE_TEXELS.min(max_dimension * max_dimension).min(max_uploaded));

					let uploaded = upload_texels(device, queue, "vello_adjustments_tables", &texels);
					if uploaded.is_none() {
						log::error!("Adjustments read lookup tables holding more data than the GPU can read, so they're left out");
					}

					self.tables.push(CachedTables {
						key,
						bytes: uploaded
							.as_ref()
							.map_or(0, |texture| u64::from(texture.width()) * u64::from(texture.height()) * size_of::<[f32; 4]>() as u64),
						uploaded,
						last_used: render_count,
					});
					self.tables.len() - 1
				});
				let cached_tables = &mut self.tables[index];
				cached_tables.last_used = render_count;
				let Some(uploaded) = cached_tables.uploaded.clone() else { return Vec::new() };
				Some(uploaded)
			}
			None => None,
		};

		(passes.into_iter())
			.map(|(shader, steps)| PreparedPass {
				shader,
				steps,
				tables: tables.clone(),
			})
			.collect()
	}

	/// Lets go of what no recent render used, keeping all that this render used. O(n log n).
	pub(crate) fn finish_render(&mut self, render_count: u64) {
		// Dragging a slider makes a new run every frame, so the runs this render didn't use make way rather than piling up, least recent first
		self.runs.sort_by_key(|cached| Reverse(cached.last_used));
		let used_this_render = self.runs.iter().take_while(|cached| cached.last_used == render_count).count();
		self.runs.truncate(used_this_render.max(MAX_CACHED_RUNS));

		// Tables are kept within a byte budget instead, since one can take up hundreds of megabytes
		self.tables.sort_by_key(|cached| Reverse(cached.last_used));
		let mut kept_bytes = 0;
		self.tables.retain(|cached| {
			kept_bytes += cached.bytes;
			cached.last_used == render_count || kept_bytes <= TABLE_CACHE_BYTES
		});
	}

	#[cfg(test)]
	pub(crate) fn cached_counts(&self) -> (usize, usize) {
		(self.runs.len(), self.tables.len())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use wgpu::naga;

	/// The interpreter, then the shader of each standalone kind.
	fn every_shader() -> impl Iterator<Item = AdjustmentShader> {
		std::iter::once(AdjustmentShader::Interpreter).chain(STANDALONE_KINDS.map(AdjustmentShader::Standalone))
	}

	#[test]
	fn every_shader_translates_for_every_backend() {
		for shader in every_shader() {
			let source = shader.source();
			let module = naga::front::wgsl::parse_str(&source).unwrap_or_else(|error| panic!("{shader:?}: {}", error.emit_to_string(&source)));
			let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
				.validate(&module)
				.unwrap_or_else(|error| panic!("{shader:?}: {}", error.emit_to_string(&source)));

			// GLSL ES 3.0, which the web build's WebGL2 backend translates each entry point into
			let options = naga::back::glsl::Options {
				version: naga::back::glsl::Version::Embedded { version: 300, is_webgl: true },
				..Default::default()
			};
			for (shader_stage, entry_point) in [(naga::ShaderStage::Vertex, "vertex"), (naga::ShaderStage::Fragment, "adjust")] {
				let pipeline_options = naga::back::glsl::PipelineOptions {
					shader_stage,
					entry_point: entry_point.to_string(),
					multiview: None,
				};

				let mut glsl = String::new();
				let written = naga::back::glsl::Writer::new(&mut glsl, &module, &info, &options, &pipeline_options, naga::proc::BoundsCheckPolicies::default()).and_then(|mut writer| writer.write());
				assert!(written.is_ok(), "{shader:?}: the {entry_point} entry point doesn't translate to GLSL: {written:?}");
			}

			// HLSL, which Direct3D compiles natively and ANGLE's D3D11 backend compiles by way of GLSL
			let mut hlsl = String::new();
			let written = naga::back::hlsl::Writer::new(&mut hlsl, &Default::default(), &Default::default()).write(&module, &info, None);
			assert!(written.is_ok(), "{shader:?}: the shader doesn't translate to HLSL: {written:?}");

			// MSL for Metal, without the bindings a pipeline's layout would map, and SPIR-V for Vulkan
			let options = naga::back::msl::Options {
				fake_missing_bindings: true,
				..Default::default()
			};
			let written = naga::back::msl::write_string(&module, &info, &options, &Default::default());
			assert!(written.is_ok(), "{shader:?}: the shader doesn't translate to MSL: {:?}", written.err());

			let written = naga::back::spv::write_vec(&module, &info, &Default::default(), None);
			assert!(written.is_ok(), "{shader:?}: the shader doesn't translate to SPIR-V: {:?}", written.err());
		}
	}

	#[test]
	fn the_interpreter_carries_out_every_kind_but_the_standalone_ones() {
		let source = AdjustmentShader::Interpreter.source();

		for kind in StepKind::ALL {
			let (function, _) = step_function(kind);
			let case = format!("case {}u: {{\n\t\t\t\t", kind.code());
			assert_eq!(source.contains(&case), !STANDALONE_KINDS.contains(&kind), "{kind:?}");
			assert_eq!(source.contains(&format!("color = {function}(color, at);")), !STANDALONE_KINDS.contains(&kind), "{kind:?}");
		}
	}

	#[test]
	fn a_pixel_left_fully_transparent_ends_its_pass() {
		let empty = "return vec4<f32>(0.);";

		// Once for the source pixel before any step, then right after each of the two kinds that can lower alpha
		let interpreter = AdjustmentShader::Interpreter.source();
		let entry_point = &interpreter[interpreter.find("fn adjust(").unwrap_or_default()..];
		assert_eq!(entry_point.matches(empty).count(), 3, "{entry_point}");
		assert!(entry_point.find(empty) < entry_point.find("for ("), "{entry_point}");
		for function in ["levels", "curves"] {
			let call = format!("color = {function}(color, at);\n\t\t\t\tif color.a <= 0. {{\n\t\t\t\t\t{empty}");
			assert!(entry_point.contains(&call), "{function} is followed by the check in {entry_point}");
		}

		let standalone = AdjustmentShader::Standalone(StepKind::GradientMap).source();
		let entry_point = &standalone[standalone.find("fn adjust(").unwrap_or_default()..];
		assert_eq!(entry_point.matches(empty).count(), 1, "{entry_point}");
	}

	#[test]
	fn a_run_splits_into_passes_around_its_standalone_steps() {
		use StepKind::{ColorLookup, Curves, GradientMap, Invert, Levels};

		// The passes cover the steps in order, so their lengths tell where each one starts
		let lengths = |kinds: &[StepKind]| {
			let passes = pass_steps(kinds);
			assert!(passes.windows(2).all(|pair| pair[0].end == pair[1].start), "{passes:?}");
			passes.iter().map(|pass| pass.len()).collect::<Vec<_>>()
		};

		// Steps of the interpreted kinds share a pass however many there are
		assert_eq!(lengths(&[Levels, Curves, Invert, Invert]), [4]);
		assert_eq!(lengths(&[]), [0; 0]);

		// Each standalone step takes a pass of its own, even beside another of its kind
		assert_eq!(lengths(&[Levels, GradientMap, Invert, Curves, ColorLookup]), [1, 1, 2, 1]);
		assert_eq!(lengths(&[GradientMap, GradientMap, Invert]), [1, 1, 1]);

		assert_eq!(AdjustmentShader::for_pass(&[GradientMap]), AdjustmentShader::Standalone(GradientMap));
		assert_eq!(AdjustmentShader::for_pass(&[Levels, Curves]), AdjustmentShader::Interpreter);
	}

	#[test]
	fn conversions_between_encodings_come_only_where_a_step_reads_the_other_one() {
		let entry_point = |shader: AdjustmentShader| {
			let source = shader.source();
			source[source.find("fn adjust(").unwrap_or_default()..].to_string()
		};
		let count = |source: &str, conversion: &str| source.matches(conversion).count();

		// Gradient Map and Color Lookup read gamma-encoded color, so their shaders encode a linear source first, and the result's encoding is left to `result_pixel`
		for kind in STANDALONE_KINDS {
			let standalone = entry_point(AdjustmentShader::Standalone(kind));
			assert_eq!(
				(count(&standalone, "if linear {\n\t\tcolor = to_gamma(color);"), count(&standalone, "from_gamma(")),
				(1, 0),
				"{standalone}"
			);
			assert!(standalone.contains("return result_pixel(color, linear);"), "{standalone}");
		}

		// The case of each interpreted kind but Make Opaque, which keeps whichever encoding it's given, converts only a color in the other encoding
		let interpreter = entry_point(AdjustmentShader::Interpreter);
		let conditional_conversions = count(&interpreter, "\t\t\t\tif !linear {") + count(&interpreter, "\t\t\t\tif linear {");
		assert_eq!(conditional_conversions, StepKind::ALL.len() - STANDALONE_KINDS.len() - 1, "{interpreter}");

		// Both start in the source's encoding
		for shader in every_shader() {
			assert!(entry_point(shader).contains("var linear = encodings.straight_linear_source != 0u;"), "{shader:?}");
		}
	}
}
