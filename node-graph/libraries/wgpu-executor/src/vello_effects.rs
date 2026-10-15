//! The GPU passes that carry out an item's effects on its content, between rendering the content offscreen and drawing the result back.

use bytemuck::{Pod, Zeroable};
use core_types::Color;
use glam::{DMat2, DVec2, UVec2};
use graphic_types::effects::outline_extents;
use graphic_types::{EffectOperation, Effects, GlowSource};
use raster_kernels::effect_kernels::{BlurKernel, MAX_KERNEL_REACH, NEGLIGIBLE_LENGTH};
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

/// The format of the images the passes produce, with the precision to keep soft gradients smooth.
pub(crate) const WORKING_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The format of the silhouettes shadows and glows are cast from, which hold only coverage.
const COVERAGE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

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

/// The pipelines of the effect passes, each built the first time it is used.
pub(crate) struct EffectPasses {
	shader: wgpu::ShaderModule,
	bind_group_layout: wgpu::BindGroupLayout,
	pipeline_layout: wgpu::PipelineLayout,
	sampler: wgpu::Sampler,
	pipelines: Mutex<HashMap<(&'static str, wgpu::TextureFormat), wgpu::RenderPipeline>>,
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
			renders_working_formats,
		}
	}

	fn pipeline(&self, device: &wgpu::Device, entry_point: &'static str, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
		let mut pipelines = self.pipelines.lock().unwrap_or_else(PoisonError::into_inner);

		pipelines
			.entry((entry_point, format))
			.or_insert_with(|| {
				device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
					label: Some(entry_point),
					layout: Some(&self.pipeline_layout),
					vertex: wgpu::VertexState {
						module: &self.shader,
						entry_point: Some("vertex"),
						compilation_options: Default::default(),
						buffers: &[],
					},
					fragment: Some(wgpu::FragmentState {
						module: &self.shader,
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
			})
			.clone()
	}

	/// Carries out `effects` on `content`, a texture of premultiplied pixels, returning the result in a pooled texture of [`WORKING_FORMAT`] the same size.
	/// `lengths_to_pixels` maps the effects' lengths onto the texture's pixels, and `fill` fades the content but not the shadows and glows cast from it.
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
	) -> wgpu::Texture {
		let mut recorder = PassRecorder {
			passes: self,
			device,
			encoder: device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("vello_effects") }),
			textures,
			size: UVec2::new(content.width(), content.height()),
		};
		let length_scale = lengths_to_pixels.determinant().abs().sqrt();

		// The content at full strength, which casts every silhouette, and the content faded by the fill, which shows in the result
		let mut full = recorder.scale(content, 1.);
		let mut filled = (fill < 1.).then(|| recorder.scale(content, fill));
		// The shadows and glows stacked beneath the content
		let mut beneath: Option<wgpu::Texture> = None;

		for effect in effects.operations() {
			match effect {
				EffectOperation::Blur { radius, box_blur, gamma } => {
					let radius = radius * length_scale;
					if radius < NEGLIGIBLE_LENGTH {
						continue;
					}

					// What lies beneath blurs together with the content
					if let Some(beneath) = beneath.take() {
						if let Some(filled) = &mut filled {
							let composited = recorder.composite_beneath(filled, &beneath, &full);
							recorder.replace(filled, composited);
						}
						let composited = recorder.composite_beneath(&full, &beneath, &full);
						recorder.replace(&mut full, composited);
						recorder.textures.release(beneath);
					}

					let kernel = if box_blur { BlurKernel::Box { radius } } else { BlurKernel::Gaussian { sigma: radius } };
					full = recorder.blur(full, kernel, !gamma);
					filled = filled.map(|filled| recorder.blur(filled, kernel, !gamma));
				}
				EffectOperation::DropShadow { color, offset, blur, spread } => {
					let coverage = recorder.cast(&full, lengths_to_pixels * offset, blur * length_scale, spread * length_scale, false);
					beneath = Some(recorder.accumulate(beneath, &coverage, color));
					recorder.textures.release(coverage);
				}
				EffectOperation::OuterGlow { color, blur, spread } => {
					let coverage = recorder.cast(&full, DVec2::ZERO, blur * length_scale, spread * length_scale, false);
					beneath = Some(recorder.accumulate(beneath, &coverage, color));
					recorder.textures.release(coverage);
				}
				EffectOperation::InnerShadow { color, offset, blur, spread } => {
					let coverage = recorder.cast(&full, lengths_to_pixels * offset, blur * length_scale, spread * length_scale, true);
					recorder.shade_within(&mut full, &mut filled, coverage, color);
				}
				EffectOperation::InnerGlow { color, blur, choke, source } => {
					let mut coverage = recorder.cast(&full, DVec2::ZERO, blur * length_scale, choke * length_scale, true);
					if source == GlowSource::Center {
						let inverted = recorder.run("invert_coverage", COVERAGE_FORMAT, recorder.size, &coverage, None, None, Parameters::default());
						recorder.replace(&mut coverage, inverted);
					}
					recorder.shade_within(&mut full, &mut filled, coverage, color);
				}
				EffectOperation::Satin { color, offset, blur, invert } => {
					let offset = lengths_to_pixels * offset;
					let forward = recorder.cast(&full, offset, blur * length_scale, 0., false);
					let backward = recorder.cast(&full, -offset, blur * length_scale, 0., false);
					let parameters = Parameters {
						amount: if invert { 1. } else { 0. },
						..Default::default()
					};
					let coverage = recorder.run("satin", COVERAGE_FORMAT, recorder.size, &forward, Some(&backward), None, parameters);
					recorder.textures.release(forward);
					recorder.textures.release(backward);

					recorder.shade_within(&mut full, &mut filled, coverage, color);
				}
				EffectOperation::Outline { color, weight, align } => {
					let (outside, inside) = outline_extents(weight * length_scale, align);
					if outside >= NEGLIGIBLE_LENGTH {
						let coverage = recorder.edge_band(&full, outside, false);
						beneath = Some(recorder.accumulate(beneath, &coverage, color));
						recorder.textures.release(coverage);
					}
					if inside >= NEGLIGIBLE_LENGTH {
						let coverage = recorder.edge_band(&full, inside, true);
						recorder.shade_within(&mut full, &mut filled, coverage, color);
					}
				}
			}
		}

		let result = match (filled, beneath) {
			(filled, Some(beneath)) => {
				let result = recorder.composite_beneath(filled.as_ref().unwrap_or(&full), &beneath, &full);
				recorder.textures.release(beneath);
				recorder.textures.release(full);
				if let Some(filled) = filled {
					recorder.textures.release(filled);
				}
				result
			}
			(Some(filled), None) => {
				recorder.textures.release(full);
				filled
			}
			(None, None) => full,
		};

		queue.submit([recorder.encoder.finish()]);
		result
	}
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
		use wgpu::util::DeviceExt;

		let target = self.textures.acquire(self.device, size, format);
		let parameters = Parameters {
			target_to_uv: [1. / size.x as f32, 1. / size.y as f32],
			..parameters
		};

		let view = |texture: &wgpu::Texture| texture.create_view(&wgpu::TextureViewDescriptor::default());
		let (source_view, secondary_view, content_view) = (view(source), view(secondary.unwrap_or(source)), view(content.unwrap_or(source)));
		let parameters_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("vello_effects_parameters"),
			contents: bytemuck::bytes_of(&parameters),
			usage: wgpu::BufferUsages::UNIFORM,
		});
		let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("vello_effects_bind_group"),
			layout: &self.passes.bind_group_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&source_view),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::TextureView(&secondary_view),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::TextureView(&content_view),
				},
				wgpu::BindGroupEntry {
					binding: 3,
					resource: wgpu::BindingResource::Sampler(&self.passes.sampler),
				},
				wgpu::BindGroupEntry {
					binding: 4,
					resource: parameters_buffer.as_entire_binding(),
				},
			],
		});

		let pipeline = self.passes.pipeline(self.device, entry_point, format);
		let target_view = view(&target);
		let mut pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some(entry_point),
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view: &target_view,
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
		pass.set_pipeline(&pipeline);
		pass.set_bind_group(0, &bind_group, &[]);
		pass.draw(0..3, 0..1);
		drop(pass);

		target
	}

	/// Puts `next` in place of the texture in `slot`, returning the old one to the pool.
	fn replace(&mut self, slot: &mut wgpu::Texture, next: wgpu::Texture) {
		self.textures.release(std::mem::replace(slot, next));
	}

	/// A copy of `source` in the working format, multiplied by `amount`.
	fn scale(&mut self, source: &wgpu::Texture, amount: f32) -> wgpu::Texture {
		self.run("scale", WORKING_FORMAT, self.size, source, None, None, Parameters { amount, ..Default::default() })
	}

	/// The coverage of the content's silhouette (or of the area outside it, if `inverse`), cast by `offset`, grown by `spread` and softened by `blur`, all in pixels.
	fn cast(&mut self, content: &wgpu::Texture, offset: DVec2, blur: f64, spread: f64, inverse: bool) -> wgpu::Texture {
		let offset = offset / self.size.as_dvec2();
		let parameters = Parameters {
			offset: [offset.x as f32, offset.y as f32],
			..Default::default()
		};
		let entry_point = if inverse { "cast_inverse_silhouette" } else { "cast_silhouette" };
		let mut coverage = self.run(entry_point, COVERAGE_FORMAT, self.size, content, None, None, parameters);

		if spread >= NEGLIGIBLE_LENGTH {
			coverage = self.dilate(coverage, spread);
		}
		if blur >= NEGLIGIBLE_LENGTH {
			coverage = self.blur(coverage, BlurKernel::Gaussian { sigma: blur }, false);
		}

		coverage
	}

	/// Stacks a silhouette's coverage, tinted `color`, over what lies `beneath` the content so far.
	fn accumulate(&mut self, beneath: Option<wgpu::Texture>, coverage: &wgpu::Texture, color: Color) -> wgpu::Texture {
		let parameters = Parameters {
			color: premultiplied_gamma_channels(color),
			..Default::default()
		};

		let Some(beneath) = beneath else {
			return self.run("tint", WORKING_FORMAT, self.size, coverage, None, None, parameters);
		};
		let accumulated = self.run("accumulate", WORKING_FORMAT, self.size, &beneath, Some(coverage), None, parameters);
		self.textures.release(beneath);

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

	/// Places a silhouette's coverage, tinted `color`, over both the full-strength content and the faded fill, within the content, then frees the coverage.
	fn shade_within(&mut self, full: &mut wgpu::Texture, filled: &mut Option<wgpu::Texture>, coverage: wgpu::Texture, color: Color) {
		if let Some(filled) = filled {
			let composited = self.composite_within(filled, &coverage, full, color);
			self.replace(filled, composited);
		}
		let composited = self.composite_within(full, &coverage, full, color);
		self.replace(full, composited);
		self.textures.release(coverage);
	}

	/// The coverage of the band reaching `width` pixels from the content's silhouette edge, outward or (if `inward`) inward, measured in straight lines so it rounds corners.
	/// Jump flooding finds each texel's nearest texel across the edge, in a pass for each halving of a step that starts past the width.
	fn edge_band(&mut self, content: &wgpu::Texture, width: f64, inward: bool) -> wgpu::Texture {
		let parameters = Parameters {
			amount: if inward { 1. } else { 0. },
			..Default::default()
		};
		let mut nearest = self.run("jump_flood_seed", WORKING_FORMAT, self.size, content, None, None, parameters);

		let mut step = (width.max(0.).ceil() as u32 + 1).next_power_of_two();
		while step >= 1 {
			let parameters = Parameters {
				values: [step as f32, 0., 0., 0.],
				..Default::default()
			};
			let flooded = self.run("jump_flood_step", WORKING_FORMAT, self.size, &nearest, None, None, parameters);
			self.replace(&mut nearest, flooded);
			step /= 2;
		}

		let parameters = Parameters {
			values: [width as f32, 0., 0., 0.],
			..Default::default()
		};
		let band = self.run("edge_band", COVERAGE_FORMAT, self.size, &nearest, None, None, parameters);
		self.textures.release(nearest);
		band
	}

	/// The image with the shadows and glows `beneath` placed under it, knocked out by the full-strength content.
	fn composite_beneath(&mut self, image: &wgpu::Texture, beneath: &wgpu::Texture, full_content: &wgpu::Texture) -> wgpu::Texture {
		self.run("composite_beneath", WORKING_FORMAT, self.size, image, Some(beneath), Some(full_content), Parameters::default())
	}

	/// Grows the shapes in `texture` by `distance` pixels along each axis, in a square like SVG's `feMorphology`.
	fn dilate(&mut self, texture: wgpu::Texture, distance: f64) -> wgpu::Texture {
		// Steps that double, then the remainder, so each pass grows the shapes by up to as much as all the passes before it
		let mut steps = Vec::new();
		let (mut covered, mut step) = (0., 1.);
		while covered + step <= distance {
			steps.push(step);
			covered += step;
			step *= 2.;
		}
		if distance - covered >= NEGLIGIBLE_LENGTH {
			steps.push(distance - covered);
		}

		let format = texture.format();
		let mut texture = texture;
		for step in steps {
			for axis in [DVec2::X, DVec2::Y] {
				let step = axis * step / self.size.as_dvec2();
				let parameters = Parameters {
					step: [step.x as f32, step.y as f32],
					..Default::default()
				};
				let dilated = self.run("dilate", format, self.size, &texture, None, None, parameters);
				self.replace(&mut texture, dilated);
			}
		}

		texture
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
	render_count: u64,
}

impl EffectTextures {
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
	fn merged_taps_sample_between_the_texels_they_weigh() {
		// A box of half width two weighs five texels evenly, so its outer pair merges into taps one and a half texels out
		let taps = kernel_taps(BlurKernel::Box { radius: 2. }, 0);

		assert_eq!(taps.len(), 3);
		assert!((taps[0][1] - 0.2).abs() < 1e-6);
		assert!((taps[1][0] - 1.5).abs() < 1e-6 && (taps[1][1] - 0.4).abs() < 1e-6);
		assert!((taps[2][0] + 1.5).abs() < 1e-6);
	}
}
