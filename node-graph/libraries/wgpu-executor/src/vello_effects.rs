//! The GPU passes that carry out an item's effects on its content, between rendering the content offscreen and drawing the result back.

use crate::vello_adjustments::{AdjustmentShaders, AdjustmentUploads, PassEncodings, PreparedPass};
use bytemuck::{Pod, Zeroable};
use core_types::Color;
use core_types::transfer_curve::TransferCurve;
use glam::{DMat2, DVec2, UVec2};
use graphic_types::effects::{EffectCapture, GRADIENT_GLOW_FADE, GlowFill, grow_and_tent, shadow_offset};
use graphic_types::{Adjustment, EffectOperation, Effects, GlowTechnique};
use raster_kernels::effect_kernels::{BlurKernel, MAX_KERNEL_REACH, NEGLIGIBLE_LENGTH};
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use wgpu::util::DeviceExt;

/// The format of the images the passes produce, with the precision to keep soft gradients smooth.
pub(crate) const WORKING_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The format of the silhouettes shadows and glows are cast from, which hold only coverage, or a distance.
const COVERAGE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

/// How many samples of a contour the shaders interpolate between, enough to follow its spline to well within a level.
const CONTOUR_SAMPLES: u32 = 256;
/// How many samples of a glow's gradient the shaders interpolate between.
const GRADIENT_SAMPLES: u32 = 256;

/// The most taps one kernel pass weighs, matching the length of the shader's array, which holds a kernel of [`MAX_KERNEL_REACH`] texels to each side in pairs.
const MAX_TAPS: usize = 1 + 2 * MAX_KERNEL_REACH.div_ceil(2);

/// How many renders a pooled texture survives unused before it is freed.
const UNUSED_TEXTURE_RENDER_LIMIT: u64 = 3;

/// The shader's parameters, laid out to match its `Parameters` struct.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Parameters {
	target_to_uv: [f32; 2],
	step: [f32; 2],
	offset: [f32; 2],
	amount: f32,
	tap_count: u32,
	color: [f32; 4],
	values: [f32; 4],
	taps: [[f32; 4]; MAX_TAPS],
}

impl Default for Parameters {
	fn default() -> Self {
		Self::zeroed()
	}
}

/// The kernel's taps for a texture halved `levels` times, each the distance and weight of a symmetric pair of taps that sample between two texels.
fn kernel_taps(kernel: BlurKernel, levels: u32) -> Vec<[f32; 4]> {
	let weights = kernel.weights(levels);

	// Each tap between a pair of texels samples them in the ratio of their weights, through bilinear filtering
	let mut taps = vec![[0., weights[0] as f32, 0., 0.]];
	for (pair_index, pair) in weights[1..].chunks(2).enumerate() {
		let near_distance = (1 + 2 * pair_index) as f64;
		let (distance, weight) = match *pair {
			[near, far] => ((near_distance * near + (near_distance + 1.) * far) / (near + far), near + far),
			[near] => (near_distance, near),
			_ => continue,
		};
		taps.push([distance as f32, weight as f32, 0., 0.]);
		taps.push([-distance as f32, weight as f32, 0., 0.]);
	}

	taps
}

/// A pipeline running the `entry_point` of `shader` over one triangle covering a target of `format`.
pub(crate) fn full_target_pipeline(device: &wgpu::Device, layout: &wgpu::PipelineLayout, shader: &wgpu::ShaderModule, entry_point: &'static str, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
	device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: Some(entry_point),
		layout: Some(layout),
		vertex: wgpu::VertexState {
			module: shader,
			entry_point: Some("vertex"),
			compilation_options: Default::default(),
			buffers: &[],
		},
		fragment: Some(wgpu::FragmentState {
			module: shader,
			entry_point: Some(entry_point),
			compilation_options: Default::default(),
			targets: &[Some(wgpu::ColorTargetState {
				format,
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
	})
}

/// The pipelines of the effect passes, each built the first time it is used.
pub(crate) struct EffectPasses {
	shader: wgpu::ShaderModule,
	bind_group_layout: wgpu::BindGroupLayout,
	pipeline_layout: wgpu::PipelineLayout,
	sampler: wgpu::Sampler,
	pipelines: Mutex<HashMap<(&'static str, wgpu::TextureFormat), wgpu::RenderPipeline>>,
	adjustment_shaders: AdjustmentShaders,
	/// Whether the GPU can render into the passes' formats, which WebGL2 can't without `EXT_color_buffer_float`.
	pub(crate) renders_working_formats: bool,
}

impl EffectPasses {
	pub(crate) fn new(device: &wgpu::Device, adapter: &wgpu::Adapter) -> Self {
		let renders_working_formats = [WORKING_FORMAT, COVERAGE_FORMAT]
			.into_iter()
			.all(|format| adapter.get_texture_format_features(format).allowed_usages.contains(wgpu::TextureUsages::RENDER_ATTACHMENT));
		if !renders_working_formats {
			log::warn!("This GPU can't render into the formats layer effects need, so content draws without its effects");
		}

		let shader = device.create_shader_module(wgpu::include_wgsl!("vello_effects.wgsl"));

		let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
			binding,
			visibility: wgpu::ShaderStages::FRAGMENT,
			ty: wgpu::BindingType::Texture {
				sample_type: wgpu::TextureSampleType::Float { filterable: true },
				view_dimension: wgpu::TextureViewDimension::D2,
				multisampled: false,
			},
			count: None,
		};
		let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("vello_effects_bind_group_layout"),
			entries: &[
				texture_entry(0),
				texture_entry(1),
				texture_entry(2),
				wgpu::BindGroupLayoutEntry {
					binding: 3,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 4,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Uniform,
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				},
			],
		});
		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("vello_effects_pipeline_layout"),
			bind_group_layouts: &[Some(&bind_group_layout)],
			immediate_size: 0,
		});

		// Sampling past the edges repeats them, which is transparent since the content's offscreen area reaches past it by the effects' reach
		let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
			label: Some("vello_effects_sampler"),
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Linear,
			..Default::default()
		});

		Self {
			shader,
			bind_group_layout,
			pipeline_layout,
			sampler,
			pipelines: Mutex::new(HashMap::new()),
			adjustment_shaders: AdjustmentShaders::new(device, adapter),
			renders_working_formats,
		}
	}

	fn pipeline(&self, device: &wgpu::Device, entry_point: &'static str, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
		let mut pipelines = self.pipelines.lock().unwrap_or_else(PoisonError::into_inner);

		pipelines
			.entry((entry_point, format))
			.or_insert_with(|| full_target_pipeline(device, &self.pipeline_layout, &self.shader, entry_point, format))
			.clone()
	}

	/// Carries out `effects` on `content`, a texture of premultiplied pixels, returning the result in a pooled texture of [`WORKING_FORMAT`] the same size.
	/// `lengths_to_pixels` maps the effects' lengths onto the texture's pixels, and `fill` fades the content but not the shadows and glows cast from it.
	/// With a `capture`, the result is instead that effect's coverage, as black with it in alpha, and the effects after it are left out.
	#[expect(clippy::too_many_arguments)]
	pub(crate) fn apply(
		&self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		textures: &mut EffectTextures,
		content: &wgpu::Texture,
		effects: &Effects,
		fill: f32,
		lengths_to_pixels: DMat2,
		capture: Option<EffectCapture>,
	) -> wgpu::Texture {
		let mut recorder = PassRecorder {
			passes: self,
			device,
			queue,
			encoder: device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("vello_effects") }),
			textures,
			size: UVec2::new(content.width(), content.height()),
		};
		let length_scale = lengths_to_pixels.determinant().abs().sqrt();

		// The content at full strength, which casts every silhouette, read from the content itself (whatever its format) until a pass replaces it
		let mut full: Option<wgpu::Texture> = None;
		// The content faded by the fill, which shows in the result
		let mut filled = (fill < 1.).then(|| recorder.scale(content, fill));
		// The shadows and glows stacked beneath the content
		let mut beneath: Option<wgpu::Texture> = None;

		// Each operation with its index, for a capture to name, leaving out those that change nothing so they don't split the runs of adjustments around them
		let operations: Vec<(usize, EffectOperation)> = effects.operations().enumerate().filter(|(_, operation)| !changes_nothing(operation, length_scale)).collect();
		for (position, (index, operation)) in operations.iter().enumerate() {
			let index = *index;
			// A captured coverage ends the passes in place of the effects' result
			let mut captured = None;
			// A coverage is captured in black, and an already tinted image as it is
			let mut capture_plane = |recorder: &mut PassRecorder, plane: usize, coverage: &wgpu::Texture, tinted: bool| {
				if capture == Some(EffectCapture { index, plane }) {
					let black = Parameters {
						color: [0., 0., 0., 1.],
						..Default::default()
					};
					captured = Some(match tinted {
						true => recorder.scale(coverage, 1.),
						false => recorder.run("tint", WORKING_FORMAT, recorder.size, coverage, None, None, black),
					});
				}
			};

			match *operation {
				EffectOperation::Blur { radius, box_blur, gamma } => {
					let radius = radius * length_scale;
					recorder.merge_beneath(content, &mut full, &mut filled, beneath.take());

					// The blur gives its input back to the pool, so it takes a copy of the content rather than the content itself
					let kernel = if box_blur { BlurKernel::Box { radius } } else { BlurKernel::Gaussian { sigma: radius } };
					let unblurred = full.take().unwrap_or_else(|| recorder.scale(content, 1.));
					full = Some(recorder.blur(unblurred, kernel, !gamma));
					filled = filled.map(|filled| recorder.blur(filled, kernel, !gamma));
				}
				EffectOperation::DropShadow {
					color,
					angle,
					distance,
					spread,
					size,
					ref contour,
					knockout,
				} => {
					let full_content = full.as_ref().unwrap_or(content);
					let silhouette = recorder.silhouette(full_content, lengths_to_pixels * shadow_offset(angle, distance), false);
					let mut strength = recorder.cast(silhouette, size, spread, GlowTechnique::Softer, 100., contour, length_scale);
					if knockout {
						strength = recorder.knock_out(strength, full_content, fill);
					}
					capture_plane(&mut recorder, 0, &strength, false);

					beneath = Some(recorder.accumulate(beneath, &strength, color));
					recorder.textures.release(strength);
				}
				EffectOperation::OuterGlow {
					ref color,
					technique,
					spread,
					size,
					range,
					ref contour,
				} => {
					let Some(paint) = GlowFill::of(color) else { continue };
					let full_content = full.as_ref().unwrap_or(content);
					let silhouette = recorder.silhouette(full_content, DVec2::ZERO, false);
					let raw = recorder.cast_strength(silhouette, size, spread, technique, length_scale);
					let shaped = recorder.shaped(&raw, range, contour);
					let strength = shaped.as_ref().unwrap_or(&raw);

					// A solid color tints the coverage as it stacks, but a gradient paints each pixel its own color, which a map captures whole
					let (coverage, color) = match paint {
						GlowFill::Solid(color) => (recorder.scale(strength, 1.), Some(color)),
						GlowFill::Gradient { .. } => match recorder.gradient_glow_tint(&paint, strength, &raw, true) {
							Some(tinted) => (tinted, None),
							None => continue,
						},
					};
					[Some(raw), shaped].into_iter().flatten().for_each(|texture| recorder.textures.release(texture));

					let coverage = recorder.knock_out(coverage, full_content, fill);
					capture_plane(&mut recorder, 0, &coverage, color.is_none());
					beneath = Some(recorder.stack_beneath(beneath, &coverage, color));
					recorder.textures.release(coverage);
				}
				EffectOperation::InnerShadow {
					color,
					angle,
					distance,
					choke,
					size,
					ref contour,
				} => {
					let full_content = full.as_ref().unwrap_or(content);
					let outside = recorder.silhouette(full_content, lengths_to_pixels * shadow_offset(angle, distance), true);
					let strength = recorder.cast(outside, size, choke, GlowTechnique::Softer, 100., contour, length_scale);
					capture_plane(&mut recorder, 0, &strength, false);

					if let Some(filled) = &mut filled {
						let composited = recorder.composite_within(filled, &strength, full_content, color);
						recorder.replace(filled, composited);
					}
					let composited = recorder.composite_within(full_content, &strength, full_content, color);
					recorder.replace_or_fill(&mut full, composited);
					recorder.textures.release(strength);
				}
				EffectOperation::Adjustment(_) => {
					// A run of adjustments is carried out from its first row, in a pass for each standalone step and one for the steps between them
					if position > 0 && matches!(operations[position - 1].1, EffectOperation::Adjustment(_)) {
						continue;
					}
					let run: Vec<&Adjustment> = operations[position..]
						.iter()
						.map_while(|(_, operation)| match operation {
							EffectOperation::Adjustment(adjustment) => Some(adjustment),
							_ => None,
						})
						.collect();

					// The content at full strength is read only by an operation after the run, or shown as the result where no fill fades it
					let full_is_read = filled.is_none() || position + run.len() < operations.len();

					recorder.merge_beneath(content, &mut full, &mut filled, beneath.take());
					let prepared_passes = recorder.textures.prepare_adjustments(device, queue, &run);

					// Each pass but the last hands its result on in the chained format, so splitting the run rounds nothing
					for (index, prepared) in prepared_passes.iter().enumerate() {
						let format = if index + 1 < prepared_passes.len() {
							self.adjustment_shaders.chained_format
						} else {
							WORKING_FORMAT
						};
						let encodings = self.adjustment_shaders.pass_encodings(index, prepared_passes.len(), false);

						if full_is_read {
							let adjusted = recorder.adjust(full.as_ref().unwrap_or(content), prepared, format, encodings);
							recorder.replace_or_fill(&mut full, adjusted);
						}
						if let Some(filled) = &mut filled {
							let adjusted = recorder.adjust(filled, prepared, format, encodings);
							recorder.replace(filled, adjusted);
						}
					}
				}
			}

			if let Some(captured) = captured {
				[full, filled, beneath].into_iter().flatten().for_each(|texture| recorder.textures.release(texture));
				queue.submit([recorder.encoder.finish()]);
				return captured;
			}
		}

		let result = match (filled, beneath) {
			(filled, Some(beneath)) => {
				let result = recorder.composite_beneath(filled.as_ref().or(full.as_ref()).unwrap_or(content), &beneath);
				[Some(beneath), full, filled].into_iter().flatten().for_each(|texture| recorder.textures.release(texture));
				result
			}
			(Some(filled), None) => {
				if let Some(full) = full {
					recorder.textures.release(full);
				}
				filled
			}
			(None, None) => full.unwrap_or_else(|| recorder.scale(content, 1.)),
		};

		queue.submit([recorder.encoder.finish()]);
		result
	}

	/// Carries out a run of adjustments, in the passes `prepared` for it, on the top level of `image`, a texture of straight linear color if
	/// `straight_linear_image` or else of premultiplied pixels, into the top level of `target`, then averages each of `target`'s smaller levels
	/// from the one above it like the levels of an uploaded image.
	#[expect(clippy::too_many_arguments)]
	pub(crate) fn adjust_image(
		&self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		textures: &mut EffectTextures,
		image: &wgpu::Texture,
		straight_linear_image: bool,
		prepared: &[PreparedPass],
		target: &wgpu::Texture,
	) {
		let size = UVec2::new(image.width(), image.height());
		let mut recorder = PassRecorder {
			passes: self,
			device,
			queue,
			encoder: device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
				label: Some("vello_effects_adjust_image"),
			}),
			textures,
			size,
		};
		let level = |texture: &wgpu::Texture, level: u32| {
			texture.create_view(&wgpu::TextureViewDescriptor {
				base_mip_level: level,
				mip_level_count: Some(1),
				..Default::default()
			})
		};

		// Each pass but the last hands its result on in the chained format, so only the last one's result rounds to the target's format
		let mut chained: Option<wgpu::Texture> = None;
		for (index, pass) in prepared.iter().enumerate() {
			let source = chained.as_ref().unwrap_or(image);
			let encodings = self.adjustment_shaders.pass_encodings(index, prepared.len(), straight_linear_image);
			if index + 1 < prepared.len() {
				let adjusted = recorder.adjust(source, pass, self.adjustment_shaders.chained_format, encodings);
				recorder.replace_or_fill(&mut chained, adjusted);
			} else {
				let source = source.create_view(&wgpu::TextureViewDescriptor::default());
				recorder.adjust_into(&level(target, 0), target.format(), &source, pass, encodings);
			}
		}
		chained.into_iter().for_each(|texture| recorder.textures.release(texture));

		self.record_mipmaps(device, &mut recorder.encoder, target);
		queue.submit([recorder.encoder.finish()]);
	}

	/// Records the averaging of each of `texture`'s smaller mip levels from the one above it, like the levels of an uploaded image.
	pub(crate) fn record_mipmaps(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, texture: &wgpu::Texture) {
		let level = |level: u32| {
			texture.create_view(&wgpu::TextureViewDescriptor {
				base_mip_level: level,
				mip_level_count: Some(1),
				..Default::default()
			})
		};

		for index in 1..texture.mip_level_count() {
			let level_size = UVec2::new(texture.width() >> index, texture.height() >> index).max(UVec2::ONE);
			self.record(
				device,
				encoder,
				"halve",
				texture.format(),
				&level(index),
				level_size,
				&level(index - 1),
				None,
				None,
				Parameters::default(),
			);
		}
	}

	/// Records the shader's `entry_point` into `target`, a view of `format` and `size`, reading the `source` view and optionally the `secondary` and `content` views.
	#[expect(clippy::too_many_arguments)]
	fn record(
		&self,
		device: &wgpu::Device,
		encoder: &mut wgpu::CommandEncoder,
		entry_point: &'static str,
		format: wgpu::TextureFormat,
		target: &wgpu::TextureView,
		size: UVec2,
		source: &wgpu::TextureView,
		secondary: Option<&wgpu::TextureView>,
		content: Option<&wgpu::TextureView>,
		parameters: Parameters,
	) {
		let parameters = Parameters {
			target_to_uv: [1. / size.x as f32, 1. / size.y as f32],
			..parameters
		};

		let (secondary, content) = (secondary.unwrap_or(source), content.unwrap_or(source));
		let parameters_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("vello_effects_parameters"),
			contents: bytemuck::bytes_of(&parameters),
			usage: wgpu::BufferUsages::UNIFORM,
		});
		let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("vello_effects_bind_group"),
			layout: &self.bind_group_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(source),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::TextureView(secondary),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::TextureView(content),
				},
				wgpu::BindGroupEntry {
					binding: 3,
					resource: wgpu::BindingResource::Sampler(&self.sampler),
				},
				wgpu::BindGroupEntry {
					binding: 4,
					resource: parameters_buffer.as_entire_binding(),
				},
			],
		});

		let pipeline = self.pipeline(device, entry_point, format);
		draw_covering(encoder, entry_point, &pipeline, &bind_group, target);
	}
}

/// Records a pass clearing `target` and drawing one triangle covering it with the `pipeline` reading the `bind_group`.
fn draw_covering(encoder: &mut wgpu::CommandEncoder, label: &str, pipeline: &wgpu::RenderPipeline, bind_group: &wgpu::BindGroup, target: &wgpu::TextureView) {
	let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
		label: Some(label),
		color_attachments: &[Some(wgpu::RenderPassColorAttachment {
			view: target,
			resolve_target: None,
			ops: wgpu::Operations {
				load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
				store: wgpu::StoreOp::Store,
			},
			depth_slice: None,
		})],
		depth_stencil_attachment: None,
		timestamp_writes: None,
		occlusion_query_set: None,
		multiview_mask: None,
	});
	pass.set_pipeline(pipeline);
	pass.set_bind_group(0, bind_group, &[]);
	pass.draw(0..3, 0..1);
}

/// Whether an operation leaves the content as it is: a blur too slight to see, or a glow with nothing to paint.
fn changes_nothing(operation: &EffectOperation, length_scale: f64) -> bool {
	match operation {
		EffectOperation::Blur { radius, .. } => radius * length_scale < NEGLIGIBLE_LENGTH,
		EffectOperation::OuterGlow { color, .. } => GlowFill::of(color).is_none(),
		_ => false,
	}
}

/// Which pixels seed a distance pass, and how far from each one its edge lies.
#[derive(Clone, Copy)]
enum Seeds {
	/// The covered pixels, with their antialiased edges placed by coverage, matching `distances_to_silhouette`.
	Covered,
	/// The pixels of a distance texture lying up to 3 past a grow of so many pixels, matching `distances_through_seeds` of how far short of the grown edge they lie.
	PastGrow { grow: f64 },
}

/// A color as the premultiplied, gamma-encoded channels the passes work with.
fn premultiplied_gamma_channels(color: Color) -> [f32; 4] {
	let [red, green, blue, alpha] = color.to_gamma_srgb_channels();
	[red * alpha, green * alpha, blue * alpha, alpha]
}

/// Records the passes of one effects group, whose images all share one size.
struct PassRecorder<'a> {
	passes: &'a EffectPasses,
	device: &'a wgpu::Device,
	queue: &'a wgpu::Queue,
	encoder: wgpu::CommandEncoder,
	textures: &'a mut EffectTextures,
	size: UVec2,
}

impl PassRecorder<'_> {
	/// Runs the shader's `entry_point` into a new pooled texture of `format` and `size`, reading `source` and optionally the `secondary` and `content` inputs.
	#[expect(clippy::too_many_arguments)]
	fn run(
		&mut self,
		entry_point: &'static str,
		format: wgpu::TextureFormat,
		size: UVec2,
		source: &wgpu::Texture,
		secondary: Option<&wgpu::Texture>,
		content: Option<&wgpu::Texture>,
		parameters: Parameters,
	) -> wgpu::Texture {
		let target = self.textures.acquire(self.device, size, format);

		let view = |texture: &wgpu::Texture| texture.create_view(&wgpu::TextureViewDescriptor::default());
		let (source, secondary, content) = (view(source), secondary.map(view), content.map(view));
		self.run_into(entry_point, format, &view(&target), size, &source, secondary.as_ref(), content.as_ref(), parameters);

		target
	}

	/// Runs the shader's `entry_point` into `target`, a view of `format` and `size`, reading the `source` view and optionally the `secondary` and `content` views.
	#[expect(clippy::too_many_arguments)]
	fn run_into(
		&mut self,
		entry_point: &'static str,
		format: wgpu::TextureFormat,
		target: &wgpu::TextureView,
		size: UVec2,
		source: &wgpu::TextureView,
		secondary: Option<&wgpu::TextureView>,
		content: Option<&wgpu::TextureView>,
		parameters: Parameters,
	) {
		self.passes
			.record(self.device, &mut self.encoder, entry_point, format, target, size, source, secondary, content, parameters);

		#[cfg(test)]
		self.textures.drawn_passes.push(entry_point);
	}

	/// Puts `next` in place of the texture in `slot`, returning the old one to the pool.
	fn replace(&mut self, slot: &mut wgpu::Texture, next: wgpu::Texture) {
		self.textures.release(std::mem::replace(slot, next));
	}

	/// Puts `next` in `slot`, returning any texture it held to the pool.
	fn replace_or_fill(&mut self, slot: &mut Option<wgpu::Texture>, next: wgpu::Texture) {
		if let Some(previous) = slot.replace(next) {
			self.textures.release(previous);
		}
	}

	/// A copy of `source` in the working format, multiplied by `amount`.
	fn scale(&mut self, source: &wgpu::Texture, amount: f32) -> wgpu::Texture {
		self.run("scale", WORKING_FORMAT, self.size, source, None, None, Parameters { amount, ..Default::default() })
	}

	/// The coverage of the content's silhouette (or of the area outside it, if `inverse`), cast by `offset` in pixels.
	fn silhouette(&mut self, content: &wgpu::Texture, offset: DVec2, inverse: bool) -> wgpu::Texture {
		let offset = offset / self.size.as_dvec2();
		let parameters = Parameters {
			offset: [offset.x as f32, offset.y as f32],
			..Default::default()
		};
		let entry_point = if inverse { "cast_inverse_silhouette" } else { "cast_silhouette" };
		self.run(entry_point, COVERAGE_FORMAT, self.size, content, None, None, parameters)
	}

	/// The strength from 0 to 1 of a shadow or glow cast from a `silhouette`'s coverage, shaped for PSD interop, matching the CPU's `cast`:
	/// grown by the `spread` percentage of its `size` (in document pixels, `length_scale` pixels each), softened over the rest by the `technique`, then strengthened by its `range` percentage and reshaped by its `contour`.
	#[expect(clippy::too_many_arguments)]
	fn cast(&mut self, silhouette: wgpu::Texture, size: f64, spread: f64, technique: GlowTechnique, range: f64, contour: &TransferCurve, length_scale: f64) -> wgpu::Texture {
		let strength = self.cast_strength(silhouette, size, spread, technique, length_scale);
		match self.shaped(&strength, range, contour) {
			Some(shaped) => {
				self.textures.release(strength);
				shaped
			}
			None => strength,
		}
	}

	/// The strength from 0 to 1 of a shadow or glow cast from a `silhouette`'s coverage before its range and contour shape it, as [`Self::cast`] describes.
	fn cast_strength(&mut self, silhouette: wgpu::Texture, size: f64, spread: f64, technique: GlowTechnique, length_scale: f64) -> wgpu::Texture {
		let (grow, tent) = grow_and_tent(size, spread);
		let grow_pixels = grow * length_scale;

		match technique {
			GlowTechnique::Softer => {
				let mut coverage = silhouette;
				if grow_pixels >= NEGLIGIBLE_LENGTH {
					let to_silhouette = self.distances(&coverage, grow_pixels + 1., Seeds::Covered);
					let parameters = Parameters {
						values: [grow_pixels as f32, 0., 0., 0.],
						..Default::default()
					};
					let grown = self.run("grow", COVERAGE_FORMAT, self.size, &to_silhouette, None, None, parameters);
					self.textures.release(to_silhouette);
					self.replace(&mut coverage, grown);
				}

				let radius = tent * length_scale;
				if radius >= NEGLIGIBLE_LENGTH {
					coverage = self.blur(coverage, BlurKernel::Tent { radius }, false);
				}
				coverage
			}
			GlowTechnique::Precise => {
				let to_content = self.distances(&silhouette, ((size + 1.) * length_scale).max(grow_pixels + 3.), Seeds::Covered);
				self.textures.release(silhouette);

				// Within the grown shape, the falloff measures how far it lies in from the grown shape's edge
				let within_grown = (grow_pixels >= NEGLIGIBLE_LENGTH).then(|| self.distances(&to_content, grow_pixels + 1., Seeds::PastGrow { grow: grow_pixels }));
				let parameters = Parameters {
					values: [grow_pixels as f32, size as f32, length_scale as f32, if within_grown.is_some() { 1. } else { 0. }],
					..Default::default()
				};
				let falloff = self.run("precise_falloff", COVERAGE_FORMAT, self.size, &to_content, within_grown.as_ref(), None, parameters);

				self.textures.release(to_content);
				if let Some(within_grown) = within_grown {
					self.textures.release(within_grown);
				}
				falloff
			}
		}
	}

	/// The `strength` strengthened by the `range` percentage and reshaped by the `contour`, matching the CPU's `shaped`, or `None` where they leave it as it is.
	fn shaped(&mut self, strength: &wgpu::Texture, range: f64, contour: &TransferCurve) -> Option<wgpu::Texture> {
		if range >= 100. && contour.is_identity() {
			return None;
		}
		let contour_samples = (!contour.is_identity()).then(|| self.contour_samples(contour));
		let parameters = Parameters {
			values: [(range.clamp(1., 100.) / 100.) as f32, if contour_samples.is_some() { 1. } else { 0. }, 0., 0.],
			..Default::default()
		};
		Some(self.run("shape_strength", COVERAGE_FORMAT, self.size, strength, contour_samples.as_ref(), None, parameters))
	}

	/// The premultiplied color a glow filled with the gradient in `paint` paints at each pixel, from its `strength` after range and contour and its `raw` strength before them.
	/// An outer glow `fades` out with its raw strength, matching the CPU's `glow_tint`.
	fn gradient_glow_tint(&mut self, paint: &GlowFill, strength: &wgpu::Texture, raw: &wgpu::Texture, fades: bool) -> Option<wgpu::Texture> {
		let (samples, opacity) = paint.gradient_samples(GRADIENT_SAMPLES as usize)?;
		let samples: Vec<half::f16> = samples.into_iter().flatten().map(half::f16::from_f32).collect();
		let samples = self.row_texture("vello_effects_gradient", wgpu::TextureFormat::Rgba16Float, GRADIENT_SAMPLES, bytemuck::cast_slice(&samples));

		let parameters = Parameters {
			values: [if fades { 1. } else { 0. }, GRADIENT_GLOW_FADE as f32, opacity, 0.],
			..Default::default()
		};
		Some(self.run("gradient_glow_tint", WORKING_FORMAT, self.size, strength, Some(&samples), Some(raw), parameters))
	}

	/// Each pixel's distance from the `seeds` in `seed_values`, matching the CPU's distance passes, or far past `reach` pixels.
	/// Jump flooding finds them in a pass for each halving of a step that starts past `reach`. O(width × height × log(reach)).
	fn distances(&mut self, seed_values: &wgpu::Texture, reach: f64, seeds: Seeds) -> wgpu::Texture {
		let (mode, grow) = match seeds {
			Seeds::Covered => (0., 0.),
			Seeds::PastGrow { grow } => (1., grow as f32),
		};
		let parameters = |step: u32| Parameters {
			values: [step as f32, mode, grow, 0.],
			..Default::default()
		};

		let mut nearest = self.run("jump_flood_seed", WORKING_FORMAT, self.size, seed_values, None, Some(seed_values), parameters(0));
		let mut step = (reach.clamp(0., self.size.max_element() as f64).ceil() as u32 + 1).next_power_of_two();
		while step >= 1 {
			let flooded = self.run("jump_flood_step", WORKING_FORMAT, self.size, &nearest, None, Some(seed_values), parameters(step));
			self.replace(&mut nearest, flooded);
			step /= 2;
		}

		let distances = self.run("jump_flood_distance", COVERAGE_FORMAT, self.size, &nearest, None, Some(seed_values), parameters(0));
		self.textures.release(nearest);
		distances
	}

	/// A row of samples of the contour across 0 to 1, which shaders interpolate between.
	/// It's made fresh rather than pooled, since its upload runs ahead of the passes recorded before it that a pooled texture may still serve.
	fn contour_samples(&mut self, contour: &TransferCurve) -> wgpu::Texture {
		let contour = contour.evaluator();
		let samples: Vec<half::f16> = (0..CONTOUR_SAMPLES)
			.map(|index| half::f16::from_f64(contour.evaluate(index as f64 / (CONTOUR_SAMPLES - 1) as f64).clamp(0., 1.)))
			.collect();
		self.row_texture("vello_effects_contour", COVERAGE_FORMAT, CONTOUR_SAMPLES, bytemuck::cast_slice(&samples))
	}

	/// A texture one row of `width` texels tall holding `data` in the `format`, for shaders to interpolate between its samples.
	/// It's made fresh rather than pooled, since its upload runs ahead of the passes recorded before it that a pooled texture may still serve.
	fn row_texture(&mut self, label: &str, format: wgpu::TextureFormat, width: u32, data: &[u8]) -> wgpu::Texture {
		let extent = wgpu::Extent3d {
			width,
			height: 1,
			depth_or_array_layers: 1,
		};
		let texture = self.device.create_texture(&wgpu::TextureDescriptor {
			label: Some(label),
			size: extent,
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			view_formats: &[],
		});
		self.queue.write_texture(
			wgpu::TexelCopyTextureInfo {
				texture: &texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
				aspect: wgpu::TextureAspect::All,
			},
			data,
			wgpu::TexelCopyBufferLayout {
				offset: 0,
				bytes_per_row: Some(data.len() as u32),
				rows_per_image: Some(1),
			},
			extent,
		);
		texture
	}

	/// The coverage (or tinted image) of what lies beneath, hidden behind the full-strength content where a `fill` below 1 fades the content that shows over it.
	fn knock_out(&mut self, coverage: wgpu::Texture, full_content: &wgpu::Texture, fill: f32) -> wgpu::Texture {
		if fill >= 1. {
			return coverage;
		}
		let parameters = Parameters { amount: fill, ..Default::default() };
		let knocked_out = self.run("knock_out", coverage.format(), self.size, &coverage, None, Some(full_content), parameters);
		self.textures.release(coverage);
		knocked_out
	}

	/// Stacks a silhouette's coverage, tinted `color`, over what lies `beneath` the content so far.
	fn accumulate(&mut self, beneath: Option<wgpu::Texture>, coverage: &wgpu::Texture, color: Color) -> wgpu::Texture {
		self.stack_beneath(beneath, coverage, Some(color))
	}

	/// Stacks a coverage tinted `color`, or with no color an already tinted premultiplied image, over what lies `beneath` the content so far.
	fn stack_beneath(&mut self, beneath: Option<wgpu::Texture>, coverage: &wgpu::Texture, color: Option<Color>) -> wgpu::Texture {
		let parameters = Parameters {
			color: color.map_or([0.; 4], premultiplied_gamma_channels),
			values: [if beneath.is_some() { 1. } else { 0. }, if color.is_none() { 1. } else { 0. }, 0., 0.],
			..Default::default()
		};

		let accumulated = self.run("accumulate", WORKING_FORMAT, self.size, beneath.as_ref().unwrap_or(coverage), Some(coverage), None, parameters);
		if let Some(beneath) = beneath {
			self.textures.release(beneath);
		}
		accumulated
	}

	/// The image with a silhouette's coverage, tinted `color`, placed over it within the full-strength content.
	fn composite_within(&mut self, image: &wgpu::Texture, coverage: &wgpu::Texture, full_content: &wgpu::Texture, color: Color) -> wgpu::Texture {
		let parameters = Parameters {
			color: premultiplied_gamma_channels(color),
			..Default::default()
		};
		self.run("composite_within", WORKING_FORMAT, self.size, image, Some(coverage), Some(full_content), parameters)
	}

	/// Places what lies `beneath` under the full-strength content (`content` itself until a pass replaces it) and the faded fill,
	/// so a pixel filter after it (a blur or an adjustment) processes them together.
	fn merge_beneath(&mut self, content: &wgpu::Texture, full: &mut Option<wgpu::Texture>, filled: &mut Option<wgpu::Texture>, beneath: Option<wgpu::Texture>) {
		let Some(beneath) = beneath else { return };

		if let Some(filled) = filled {
			let composited = self.composite_beneath(filled, &beneath);
			self.replace(filled, composited);
		}
		let composited = self.composite_beneath(full.as_ref().unwrap_or(content), &beneath);
		self.replace_or_fill(full, composited);
		self.textures.release(beneath);
	}

	/// The image with the shadows and glows `beneath` placed under it.
	fn composite_beneath(&mut self, image: &wgpu::Texture, beneath: &wgpu::Texture) -> wgpu::Texture {
		self.run("composite_beneath", WORKING_FORMAT, self.size, image, Some(beneath), None, Parameters::default())
	}

	/// The image (of any format a shader can load from) with a pass of adjustments carried out on it into a texture of `format`, reading what was `prepared` for the pass,
	/// in the `encodings` of its source and result.
	fn adjust(&mut self, image: &wgpu::Texture, prepared: &PreparedPass, format: wgpu::TextureFormat, encodings: PassEncodings) -> wgpu::Texture {
		let target = self.textures.acquire(self.device, self.size, format);

		let view = |texture: &wgpu::Texture| texture.create_view(&wgpu::TextureViewDescriptor::default());
		self.adjust_into(&view(&target), format, &view(image), prepared, encodings);

		target
	}

	/// Carries out a pass of adjustments on the `image` view, reading what was `prepared` for the pass, into `target`, a view of `format`, in the `encodings` of its source and result.
	fn adjust_into(&mut self, target: &wgpu::TextureView, format: wgpu::TextureFormat, image: &wgpu::TextureView, prepared: &PreparedPass, encodings: PassEncodings) {
		let passes = self.passes;
		let shaders = &passes.adjustment_shaders;

		// The pixels, the steps, the tables, and the encodings, bound in that order
		let view = |texture: &wgpu::Texture| texture.create_view(&wgpu::TextureViewDescriptor::default());
		let (steps, tables) = (view(&prepared.steps), view(prepared.tables.as_ref().unwrap_or(&shaders.no_tables)));
		let mut entries: Vec<wgpu::BindGroupEntry> = ([image, &steps, &tables].into_iter().enumerate())
			.map(|(binding, view)| wgpu::BindGroupEntry {
				binding: binding as u32,
				resource: wgpu::BindingResource::TextureView(view),
			})
			.collect();
		entries.push(wgpu::BindGroupEntry {
			binding: 3,
			resource: shaders.encodings_buffer(encodings).as_entire_binding(),
		});
		let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("vello_adjustments_bind_group"),
			layout: &shaders.bind_group_layout,
			entries: &entries,
		});

		let pipeline = shaders.pipeline(self.device, prepared.shader, format);
		draw_covering(&mut self.encoder, "adjust", &pipeline, &bind_group, target);

		#[cfg(test)]
		self.textures.drawn_passes.push("adjust");
	}

	/// Blurs `texture` by `kernel` along both axes, in linear light if `linear`.
	/// Kernels too wide for one pass run on a texture halved enough times for them to fit, then doubled back to full size. O(width × height) per pass.
	fn blur(&mut self, texture: wgpu::Texture, kernel: BlurKernel, linear: bool) -> wgpu::Texture {
		let format = texture.format();
		let mut texture = texture;

		if linear {
			let decoded = self.run("decode_to_linear", format, self.size, &texture, None, None, Parameters::default());
			self.replace(&mut texture, decoded);
		}

		let levels = kernel.levels();
		let size = self.size;
		let level_size = |level: u32| UVec2::new(size.x.div_ceil(1 << level), size.y.div_ceil(1 << level));

		for level in 1..=levels {
			let halved = self.run("resample", format, level_size(level), &texture, None, None, Parameters::default());
			self.replace(&mut texture, halved);
		}

		let taps = kernel_taps(kernel, levels);
		let mut tap_array = [[0.; 4]; MAX_TAPS];
		tap_array[..taps.len()].copy_from_slice(&taps);
		for axis in [DVec2::X, DVec2::Y] {
			let step = axis / level_size(levels).as_dvec2();
			let parameters = Parameters {
				step: [step.x as f32, step.y as f32],
				tap_count: taps.len() as u32,
				taps: tap_array,
				..Default::default()
			};
			let convolved = self.run("convolve", format, level_size(levels), &texture, None, None, parameters);
			self.replace(&mut texture, convolved);
		}

		for level in (0..levels).rev() {
			let doubled = self.run("resample", format, level_size(level), &texture, None, None, Parameters::default());
			self.replace(&mut texture, doubled);
		}

		if linear {
			let encoded = self.run("encode_to_gamma", format, self.size, &texture, None, None, Parameters::default());
			self.replace(&mut texture, encoded);
		}

		texture
	}
}

/// Textures for the effect passes and the content they work on, kept between renders for reuse at the same sizes.
#[derive(Default)]
pub(crate) struct EffectTextures {
	/// Each unused texture with the render it was last used in.
	free: Vec<(wgpu::Texture, u64)>,
	/// The uploads of recent runs of adjustments.
	adjustments: AdjustmentUploads,
	render_count: u64,
	/// The label of each pass drawn, in order.
	#[cfg(test)]
	pub(crate) drawn_passes: Vec<&'static str>,
}

impl EffectTextures {
	/// What each pass carrying out a run of adjustments reads, as [`AdjustmentUploads::prepare`] gives it.
	pub(crate) fn prepare_adjustments(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, run: &[&Adjustment]) -> Vec<PreparedPass> {
		self.adjustments.prepare(device, queue, run, self.render_count)
	}

	pub(crate) fn acquire(&mut self, device: &wgpu::Device, size: UVec2, format: wgpu::TextureFormat) -> wgpu::Texture {
		let size = size.max(UVec2::ONE);
		let matches = |texture: &wgpu::Texture| texture.width() == size.x && texture.height() == size.y && texture.format() == format;
		if let Some(index) = self.free.iter().position(|(texture, _)| matches(texture)) {
			return self.free.swap_remove(index).0;
		}

		device.create_texture(&wgpu::TextureDescriptor {
			label: Some("vello_effects_texture"),
			size: wgpu::Extent3d {
				width: size.x,
				height: size.y,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
			view_formats: &[],
		})
	}

	/// Returns a texture to the pool once the passes recorded so far are done with it, since later passes are recorded and submitted after them.
	pub(crate) fn release(&mut self, texture: wgpu::Texture) {
		self.free.push((texture, self.render_count));
	}

	/// Frees the textures no recent render has used.
	pub(crate) fn finish_render(&mut self) {
		let render_count = self.render_count;
		self.free.retain(|(_, last_used)| render_count - last_used < UNUSED_TEXTURE_RENDER_LIMIT);
		self.adjustments.finish_render(render_count);

		self.render_count += 1;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// The sum of the weights of all taps, which must be one so blurring keeps the total brightness.
	fn total_weight(taps: &[[f32; 4]]) -> f32 {
		taps.iter().map(|tap| tap[1]).sum()
	}

	#[test]
	fn kernels_keep_brightness_and_fit_one_pass() {
		let kernels = [0.3, 1., 2.5, 7.9, 8., 12., 100., 4000.]
			.into_iter()
			.flat_map(|length| [BlurKernel::Gaussian { sigma: length }, BlurKernel::Box { radius: length }]);

		for kernel in kernels {
			let taps = kernel_taps(kernel, kernel.levels());

			assert!(taps.len() <= MAX_TAPS, "{kernel:?} needs {} taps", taps.len());
			assert!((total_weight(&taps) - 1.).abs() < 1e-5, "{kernel:?} weighs {}", total_weight(&taps));
		}
	}

	#[test]
	fn the_steps_between_standalone_ones_share_a_pass() {
		use core_types::list::Item;
		use graphic_types::vector_types::Gradient;

		let Some(context) = futures::executor::block_on(crate::WgpuContext::for_tests()) else { return };
		let passes = EffectPasses::new(&context.device, &context.adapter);
		if !passes.renders_working_formats {
			return;
		}

		// Kinds of every sort in one run, which a blur ends, then a run of two more that a blur too slight to see doesn't split
		let first_run = [
			Adjustment::Vibrance { vibrance: 30., saturation: 10. },
			Adjustment::Threshold {
				min_luminance: 40.,
				max_luminance: 90.,
			},
			Adjustment::GradientMap {
				gradient: Item::new_from_element(Gradient::black_to_white()),
				reverse: true,
			},
			Adjustment::Levels(Default::default()),
			Adjustment::ColorLookup {
				lut_file: graphene_resource::Resource::new(b"LUT_1D_SIZE 2\n0 0 0\n1 0.5 1\n".to_vec()),
			},
			Adjustment::Invert,
		];
		let second_run = [Adjustment::Invert, Adjustment::Posterize { levels: 3 }];
		let blur = |radius| EffectOperation::Blur {
			radius,
			box_blur: false,
			gamma: false,
		};
		let operations = (first_run.iter().cloned().map(EffectOperation::Adjustment)).chain([blur(2.)]).chain([
			EffectOperation::Adjustment(second_run[0].clone()),
			blur(NEGLIGIBLE_LENGTH / 2.),
			EffectOperation::Adjustment(second_run[1].clone()),
		]);
		let effects = Effects::from_operations(operations);

		let mut textures = EffectTextures::default();
		let content = textures.acquire(&context.device, UVec2::splat(16), wgpu::TextureFormat::Rgba8Unorm);
		let queue = context.queue.lock();
		let draw = |textures: &mut EffectTextures| {
			textures.drawn_passes.clear();
			let result = passes.apply(&context.device, &queue, textures, &content, &effects, 1., DMat2::IDENTITY, None);
			textures.release(result);
			textures.drawn_passes.iter().filter(|&&label| label == "adjust").count()
		};

		// Five passes for the first run (its Gradient Map and Color Lookup one each) and one for the second. Drawing again builds no more pipelines
		// than one for each shader and format, the interpreter's for both the format chained between passes and the one each run ends in.
		let built = if passes.adjustment_shaders.chained_format == WORKING_FORMAT { 3 } else { 4 };
		assert_eq!(draw(&mut textures), 6);
		assert_eq!(passes.adjustment_shaders.cached_pipelines(), built);
		assert_eq!(draw(&mut textures), 6);
		assert_eq!(passes.adjustment_shaders.cached_pipelines(), built);
		assert_eq!(textures.adjustments.cached_counts(), (2, 1));

		// A run reading the content first does so straight from the content's own format, without copying it beforehand
		let mut drawn = |operations: Vec<EffectOperation>, fill| {
			textures.drawn_passes.clear();
			let result = passes.apply(&context.device, &queue, &mut textures, &content, &Effects::from_operations(operations), fill, DMat2::IDENTITY, None);
			textures.release(result);
			textures.drawn_passes.clone()
		};
		let second_run: Vec<EffectOperation> = second_run.iter().cloned().map(EffectOperation::Adjustment).collect();
		assert_eq!(drawn(second_run.clone(), 1.), ["adjust"]);

		// With a fill, the run adjusts the faded content alone unless an operation after it reads the content at full strength
		assert_eq!(drawn(second_run.clone(), 0.5), ["scale", "adjust"]);
		let shadow = EffectOperation::DropShadow {
			color: Color::BLACK,
			angle: 0.,
			distance: 2.,
			spread: 0.,
			size: 0.,
			contour: Default::default(),
			knockout: false,
		};
		let shadowed = drawn([second_run, vec![shadow]].concat(), 0.5);
		assert_eq!(shadowed.iter().filter(|&&label| label == "adjust").count(), 2);

		// Each standalone step takes a pass of its own, even beside another of its kind
		let gradient_map = EffectOperation::Adjustment(first_run[2].clone());
		assert_eq!(drawn(vec![gradient_map; 5], 1.), ["adjust"; 5]);
	}

	#[test]
	fn a_table_stays_uploaded_while_the_settings_beside_it_change() {
		let Some(context) = futures::executor::block_on(crate::WgpuContext::for_tests()) else { return };
		let queue = context.queue.lock();

		let lookup = Adjustment::ColorLookup {
			lut_file: graphene_resource::Resource::new(b"LUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n".to_vec()),
		};
		let mut textures = EffectTextures::default();
		let prepare = |textures: &mut EffectTextures, posterize_levels| {
			let posterize = Adjustment::Posterize { levels: posterize_levels };
			// Color Lookup's pass and then Posterize's, which share the run's tables
			let mut prepared = textures.prepare_adjustments(&context.device, &queue, &[&lookup, &posterize]);
			prepared.pop().map(|prepared| (prepared.steps, prepared.tables))
		};

		// Drawing the same run again reuses its steps, and a slider dragged beside the table reuses the table
		let Some((first_steps, Some(first_table))) = prepare(&mut textures, 4) else {
			panic!("The run reads its table")
		};
		let Some((again_steps, Some(again_table))) = prepare(&mut textures, 4) else {
			panic!("The run reads its table")
		};
		assert!(first_steps == again_steps && first_table == again_table);
		for levels in 5..20 {
			let Some((steps, Some(table))) = prepare(&mut textures, levels) else {
				panic!("The run reads its table")
			};
			assert!(steps != first_steps && table == first_table);
			textures.finish_render();
		}

		// Of the runs drawn while dragging, only the latest few are kept, but the table they share stays
		assert_eq!(textures.adjustments.cached_counts(), (8, 1));
	}

	#[test]
	fn merged_taps_sample_between_the_texels_they_weigh() {
		// A box of half width two weighs five texels evenly, so its outer pair merges into taps one and a half texels out
		let taps = kernel_taps(BlurKernel::Box { radius: 2. }, 0);

		assert_eq!(taps.len(), 3);
		assert!((taps[0][1] - 0.2).abs() < 1e-6);
		assert!((taps[1][0] - 1.5).abs() < 1e-6 && (taps[1][1] - 0.4).abs() < 1e-6);
		assert!((taps[2][0] + 1.5).abs() < 1e-6);
	}
}
