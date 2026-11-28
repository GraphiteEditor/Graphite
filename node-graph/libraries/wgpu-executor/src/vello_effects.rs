//! The GPU passes that carry out an item's effects on its content, between rendering the content offscreen and drawing the result back.

use crate::vello_adjustments::{AdjustmentShaders, AdjustmentUploads, PassEncodings, PreparedPass};
use bytemuck::{Pod, Zeroable};
use core_types::Color;
use core_types::blending::BlendMode;
use core_types::transfer_curve::TransferCurve;
use glam::{DAffine2, DVec2, UVec2};
use graphic_types::effect_layers::{EffectLayers, EffectPiece, LayerPart, PiecePlace, is_pixel_filter};
use graphic_types::effects::{EffectCapture, GRADIENT_GLOW_FADE, GRADIENT_TABLE_SIZE, GlowFill, OutlineFill, bevel_light, contour_gradient_span, grow_and_tent, outline_extents, shadow_offset};
use graphic_types::masks::{mask_feather_sigma, masked_radius_blur_levels};
use graphic_types::{Adjustment, BevelDirection, BevelStyle, BevelTechnique, EffectOperation, Effects, GlowSource, GlowTechnique};
use raster_kernels::effect_kernels::{BlurKernel, MAX_DIRECT_BOX_RADIUS, NEGLIGIBLE_LENGTH, chamfer_passes};
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use wgpu::util::DeviceExt;

mod filters;
mod masks;
mod styles;

/// The format of the images the passes produce, with the precision to keep soft gradients smooth.
pub(crate) const WORKING_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The format of the silhouettes shadows and glows are cast from, which hold only coverage, or a distance.
const COVERAGE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

/// The format of sums carried between passes, which must keep full precision.
const SUMS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

/// How many samples of a contour the shaders interpolate between, enough to follow its spline to well within a level.
const CONTOUR_SAMPLES: u32 = 256;
/// How many samples of a glow's or outline's gradient the shaders read.
const GRADIENT_SAMPLES: u32 = GRADIENT_TABLE_SIZE as u32;

/// How many taps the shader's parameters hold, enough for a blur kernel of [`raster_kernels::effect_kernels::MAX_KERNEL_REACH`] texels to each side in pairs, and few enough that a pass reading them all stays quick.
/// A filter kernel with more takes a pass for each part of them.
const PARAMETER_TAPS: usize = 81;
const _: () = assert!(PARAMETER_TAPS > 2 * raster_kernels::effect_kernels::MAX_KERNEL_REACH.div_ceil(2));

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
	taps: [[f32; 4]; PARAMETER_TAPS],
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
		let renders_working_formats = [WORKING_FORMAT, COVERAGE_FORMAT, SUMS_FORMAT]
			.into_iter()
			.all(|format| adapter.get_texture_format_features(format).allowed_usages.contains(wgpu::TextureUsages::RENDER_ATTACHMENT));
		if !renders_working_formats {
			log::warn!("This GPU can't render into the formats layer effects need, so content draws without its effects");
		}

		// The filters', styles', and masks' passes live in files of their own, sharing this one's bindings and helpers
		let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: Some("vello_effects.wgsl"),
			source: wgpu::ShaderSource::Wgsl(
				concat!(
					include_str!("vello_effects.wgsl"),
					include_str!("vello_effects/filters.wgsl"),
					include_str!("vello_styles.wgsl"),
					include_str!("vello_masks.wgsl")
				)
				.into(),
			),
		});

		// The `content` input is only loaded rather than sampled, so it can also take a 32-bit float texture, which can't be filtered
		let texture_entry = |binding, filterable| wgpu::BindGroupLayoutEntry {
			binding,
			visibility: wgpu::ShaderStages::FRAGMENT,
			ty: wgpu::BindingType::Texture {
				sample_type: wgpu::TextureSampleType::Float { filterable },
				view_dimension: wgpu::TextureViewDimension::D2,
				multisampled: false,
			},
			count: None,
		};
		let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("vello_effects_bind_group_layout"),
			entries: &[
				texture_entry(0, true),
				texture_entry(1, true),
				texture_entry(2, false),
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

	/// Carries out `effects` on `content`, a texture of premultiplied pixels, returning each of the `layers` they draw in a pooled texture of [`WORKING_FORMAT`] the same size, with the mode it blends by.
	/// `effect_to_pixels` places the effects' space on the texture's pixels, `fill` fades the content but not the shadows and glows cast from it, and `opacity` fades the layers.
	/// `paints` holds the effects' drawn paints in order, each rendered over the same pixels as the content.
	/// With a `capture`, the only result is instead that effect's coverage, as black with it in alpha, or the share [`EffectCapture::THROUGH_MASK`] names.
	#[expect(clippy::too_many_arguments)]
	pub(crate) fn apply(
		&self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		textures: &mut EffectTextures,
		content: &wgpu::Texture,
		effects: &Effects,
		layers: &EffectLayers,
		fill: f32,
		opacity: f32,
		effect_to_pixels: DAffine2,
		paints: &[wgpu::Texture],
		capture: Option<EffectCapture>,
	) -> Vec<(wgpu::Texture, BlendMode)> {
		let mut recorder = PassRecorder {
			passes: self,
			device,
			queue,
			encoder: device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("vello_effects") }),
			textures,
			size: UVec2::new(content.width(), content.height()),
		};
		let lengths_to_pixels = effect_to_pixels.matrix2;
		let length_scale = lengths_to_pixels.determinant().abs().sqrt();

		// The content at full strength, which casts every silhouette, read from the content itself (whatever its format) until a pass replaces it
		let mut full: Option<wgpu::Texture> = None;
		// The content faded by the fill, which shows in the result
		let mut filled = (fill < 1.).then(|| recorder.scale(content, fill));
		// The shadows and glows stacked beneath the content
		let mut beneath: Option<wgpu::Texture> = None;
		// The layered effects' pieces, each tinted
		let mut pieces: HashMap<EffectCapture, wgpu::Texture> = HashMap::new();
		// How much the trailing masks together show of each finished layer
		let mut layer_fade: Option<wgpu::Texture> = None;

		// A captured coverage ends the passes in place of the effects' result, once the content's layer is finished if its knockout or share depends on that
		let mut captured: Option<wgpu::Texture> = None;
		let captured_piece = capture.and_then(|capture| layers.piece(capture)).map(|(_, piece)| piece);
		// At a full fill, the content's own layer has exactly the full-strength content's alpha, so knocking out a piece within it again changes nothing
		let knocks_out_by_layer = |piece: &EffectPiece| piece.knocks_out && (fill < 1. || layers.knockout_cover(piece.capture, opacity) < 1. || layers.masks_fade_knockout_cover(piece.capture));
		let capture_needs_content_layer = capture == Some(EffectCapture::THROUGH_MASK) || captured_piece.as_ref().is_some_and(knocks_out_by_layer);

		// Each operation with its index, for a capture to name, leaving out those that change nothing so they don't split the runs of adjustments around them.
		// A blur whose result is captured runs however slight, so its map still holds the content.
		let captured_blur =
			|index: usize, operation: &EffectOperation| matches!(operation, EffectOperation::Blur { .. }) && !operation.changes_nothing() && capture.is_some_and(|capture| capture.index == index);
		let operations: Vec<(usize, EffectOperation)> = (effects.operations().enumerate())
			.filter(|(index, operation)| captured_blur(*index, operation) || !changes_nothing(operation, length_scale))
			.collect();
		let mut paints = paints.iter();
		for (position, (index, operation)) in operations.iter().enumerate() {
			let index = *index;
			// A layered effect's coverages become pieces, which the content knocks out once its layer is finished, and a flattened one is knocked out by the faded content now
			let layered = index >= layers.flattened;
			let piece = |plane: usize| EffectCapture { index, plane };
			let knockout_fill = if layered { 1. } else { fill };

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
				EffectOperation::Blur { radius, box_blur, gamma, .. } if !operation.varies_blur_radius() => {
					let radius = radius * length_scale;
					recorder.merge_beneath(content, &mut full, &mut filled, beneath.take());

					if box_blur && radius > MAX_DIRECT_BOX_RADIUS {
						// Past the widest box blurred directly, a box sums running sums as a masked box does with every pixel at the full radius, exact rather than blurred on a halved image
						let every_pixel = Parameters { color: [1.; 4], ..Default::default() };
						let full_radius = recorder.run("flood", COVERAGE_FORMAT, recorder.size, content, None, None, every_pixel);
						recorder.masked_box_blur(content, &mut full, &mut filled, &full_radius, DVec2::splat(radius), !gamma, true);
						recorder.textures.release(full_radius);
					} else {
						// The blur gives its input back to the pool, so it takes a copy of the content rather than the content itself
						let kernel = if box_blur { BlurKernel::Box { radius } } else { BlurKernel::Gaussian { sigma: radius } };
						let unblurred = full.take().unwrap_or_else(|| recorder.scale(content, 1.));
						full = Some(recorder.blur(unblurred, kernel, !gamma));
						filled = filled.map(|filled| recorder.blur(filled, kernel, !gamma));
					}

					// SVG takes the results of a box blur it can't run natively as maps
					if let Some(full) = &full {
						capture_plane(&mut recorder, 0, full, true);
					}
					if let Some(filled) = &filled {
						capture_plane(&mut recorder, 1, filled, true);
					}
				}
				// A blur varying its radius by its radius mask
				EffectOperation::Blur { radius, box_blur, gamma, .. } => {
					let Some(map) = paints.next() else { continue };
					if radius * length_scale < NEGLIGIBLE_LENGTH && !captured_blur(index, operation) {
						continue;
					}
					recorder.merge_beneath(content, &mut full, &mut filled, beneath.take());

					// The map's luminance picks each pixel's blur, which a map gives SVG where it can't draw the radius mask as markup.
					// A box takes it unrounded, since SVG reads the box's result rather than its luminance.
					let unrounded = Parameters {
						values: [if box_blur { 1. } else { 0. }, 0., 0., 0.],
						..Default::default()
					};
					let luminance = recorder.run("masked_radius_blur_luminance", COVERAGE_FORMAT, recorder.size, map, None, None, unrounded);
					if !box_blur {
						capture_plane(&mut recorder, 0, &luminance, false);
						if capture == Some(piece(0)) && !capture_needs_content_layer {
							recorder.textures.release(luminance);
							break;
						}
					}

					// The full-strength content is read by a later operation, as the unfaded result, or as a box blur's map, and capturing one plane ends the passes
					let captures_only = |plane: usize| capture == Some(piece(plane)) && !capture_needs_content_layer;
					let full_is_read = !captures_only(1) && (filled.is_none() || position + 1 < operations.len() || (box_blur && capture == Some(piece(0))));
					match box_blur {
						true => {
							if captures_only(0)
								&& let Some(filled) = filled.take()
							{
								recorder.textures.release(filled);
							}
							let radius = DVec2::splat(radius * length_scale);
							recorder.masked_box_blur(content, &mut full, &mut filled, &luminance, radius, !gamma, full_is_read);

							// SVG has no box of each pixel's own width, so it takes the results as maps
							if let Some(full) = &full {
								capture_plane(&mut recorder, 0, full, true);
							}
							if let Some(filled) = &filled {
								capture_plane(&mut recorder, 1, filled, true);
							}
						}
						false => {
							let levels: Vec<f64> = masked_radius_blur_levels(radius, length_scale).into_iter().map(|level| level * length_scale).collect();
							recorder.masked_radius_blur(content, &mut full, &mut filled, &luminance, &levels, !gamma, full_is_read);
						}
					}
					recorder.textures.release(luminance);
				}
				EffectOperation::DropShadow {
					color,
					angle,
					distance,
					spread,
					size,
					ref contour,
					knockout,
					..
				} => {
					let full_content = full.as_ref().unwrap_or(content);
					let silhouette = recorder.silhouette(full_content, lengths_to_pixels * shadow_offset(angle, distance), false);
					let mut strength = recorder.cast(silhouette, size, spread, GlowTechnique::Softer, 100., contour, length_scale);
					if knockout {
						strength = recorder.knock_out(strength, full_content, knockout_fill);
					}
					capture_plane(&mut recorder, 0, &strength, false);
					recorder.place_beneath(layered.then_some((&mut pieces, piece(0))), &mut beneath, strength, Some(color));
				}
				EffectOperation::OuterGlow {
					ref color,
					technique,
					spread,
					size,
					range,
					ref contour,
					..
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

					let coverage = recorder.knock_out(coverage, full_content, knockout_fill);
					capture_plane(&mut recorder, 0, &coverage, color.is_none());
					recorder.place_beneath(layered.then_some((&mut pieces, piece(0))), &mut beneath, coverage, color);
				}
				EffectOperation::InnerShadow {
					color,
					blend_mode,
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
					recorder.place_within(layered.then_some((&mut pieces, piece(0))), content, &mut full, &mut filled, strength, Some(color), blend_mode);
				}
				EffectOperation::InnerGlow {
					ref color,
					blend_mode,
					technique,
					source,
					choke,
					size,
					range,
					ref contour,
				} => {
					let Some(paint) = GlowFill::of(color) else { continue };

					// A glow from the center is strongest where the glow from the edges is weakest
					let outside = recorder.silhouette(full.as_ref().unwrap_or(content), DVec2::ZERO, true);
					let mut strength = recorder.cast(outside, size, choke, technique, range, contour, length_scale);
					if source == GlowSource::Center {
						let inverted = recorder.run("invert_coverage", COVERAGE_FORMAT, recorder.size, &strength, None, None, Parameters::default());
						recorder.replace(&mut strength, inverted);
					}

					// A gradient paints each pixel its own color, which a map captures whole, and doesn't fade since the content bounds it
					let destination = layered.then_some((&mut pieces, piece(0)));
					match paint {
						GlowFill::Solid(color) => {
							capture_plane(&mut recorder, 0, &strength, false);
							recorder.place_within(destination, content, &mut full, &mut filled, strength, Some(color), blend_mode);
						}
						GlowFill::Gradient { .. } => {
							let tinted = recorder.gradient_glow_tint(&paint, &strength, &strength, false);
							recorder.textures.release(strength);
							let Some(tinted) = tinted else { continue };
							capture_plane(&mut recorder, 0, &tinted, true);
							recorder.place_within(destination, content, &mut full, &mut filled, tinted, None, blend_mode);
						}
					}
				}
				EffectOperation::Satin {
					color,
					blend_mode,
					angle,
					distance,
					size,
					invert,
					ref contour,
				} => {
					// The difference between the silhouette cast both ways along the offset, softened by the size
					let offset = lengths_to_pixels * shadow_offset(angle, distance);
					let full_content = full.as_ref().unwrap_or(content);
					let cast_toward = |recorder: &mut PassRecorder, offset: DVec2| {
						let silhouette = recorder.silhouette(full_content, offset, false);
						recorder.cast(silhouette, size, 0., GlowTechnique::Softer, 100., &TransferCurve::default(), length_scale)
					};
					let (forward, backward) = (cast_toward(&mut recorder, offset), cast_toward(&mut recorder, -offset));
					let parameters = Parameters {
						amount: if invert { 1. } else { 0. },
						..Default::default()
					};
					let difference = recorder.run("satin", COVERAGE_FORMAT, recorder.size, &forward, Some(&backward), None, parameters);
					recorder.textures.release(forward);
					recorder.textures.release(backward);

					let strength = recorder.shape(difference, 100., contour);
					capture_plane(&mut recorder, 0, &strength, false);
					recorder.place_within(layered.then_some((&mut pieces, piece(0))), content, &mut full, &mut filled, strength, Some(color), blend_mode);
				}
				EffectOperation::Outline {
					ref paint,
					blend_mode,
					weight,
					align,
					contour_gradient,
				} => {
					let Some(outline_fill) = OutlineFill::of(paint, contour_gradient) else { continue };
					let drawn_paint = match outline_fill {
						OutlineFill::Drawn(_) => match paints.next() {
							Some(paint) => Some(paint),
							None => continue,
						},
						_ => None,
					};

					let (outside, inside) = outline_extents(weight * length_scale, align);
					let contour_span = matches!(outline_fill, OutlineFill::ContourGradient(_))
						.then(|| contour_gradient_span(weight, align))
						.map(|(start, span)| (start * length_scale, span * length_scale));
					let bands = recorder.outline_bands(full.as_ref().unwrap_or(content), outside, inside, contour_span);
					if let Some(positions) = &bands.positions {
						capture_plane(&mut recorder, 2, positions, false);
					}

					// A solid color tints each band as it's composited, and other paints color it beforehand
					let color = match outline_fill {
						OutlineFill::Solid(color) => Some(color),
						_ => None,
					};
					let painted = |recorder: &mut PassRecorder, band: wgpu::Texture| match (&outline_fill, drawn_paint, &bands.positions) {
						(OutlineFill::Drawn(_), Some(paint), _) => recorder.paint_band(band, paint),
						(OutlineFill::ContourGradient(gradient), _, Some(positions)) => recorder.contour_gradient_band(band, positions, gradient),
						_ => band,
					};

					// The band outside lies beneath the content like a shadow, and the band inside over it like an inner shadow
					if let Some(band) = bands.outside {
						let band = recorder.knock_out(band, full.as_ref().unwrap_or(content), knockout_fill);
						capture_plane(&mut recorder, 0, &band, false);

						let band = painted(&mut recorder, band);
						recorder.place_beneath(layered.then_some((&mut pieces, piece(0))), &mut beneath, band, color);
					}
					if let Some(band) = bands.inside {
						capture_plane(&mut recorder, 1, &band, false);
						let band = painted(&mut recorder, band);
						recorder.place_within(layered.then_some((&mut pieces, piece(1))), content, &mut full, &mut filled, band, color, blend_mode);
					}
					if let Some(positions) = bands.positions {
						recorder.textures.release(positions);
					}
				}
				EffectOperation::BevelAndEmboss {
					style,
					technique,
					depth,
					direction,
					size,
					soften,
					angle,
					altitude,
					highlight_color,
					highlight_blend_mode,
					shadow_color,
					shadow_blend_mode,
					ref gloss_contour,
					gloss_antialiased,
					ref slope_contour,
					slope_antialiased,
					slope_range,
					texture_depth,
					..
				} => {
					let texture = match operation.drawn_paint() {
						Some(_) => match paints.next() {
							Some(texture) => Some(texture),
							None => continue,
						},
						None => None,
					};
					let size = size * length_scale;
					if size < NEGLIGIBLE_LENGTH && gloss_contour.is_identity() {
						continue;
					}

					// The surface's height, reshaped by the contour and sunk by the texture, lit from the light's direction into its highlights and shadows
					let (height, rise) = recorder.bevel_height(full.as_ref().unwrap_or(content), style, technique, size);
					let rise = rise * depth / 100. * if direction == BevelDirection::Down { -1. } else { 1. };

					// Under a gloss contour or texture, the shading beneath the content fades out across the slopes' foot
					let fades_at_foot = !gloss_contour.is_identity() || texture.is_some();

					let surface = styles::BevelSurface {
						style,
						slope_contour,
						slope_range,
						slope_antialiased,
						texture,
						texture_depth,
					};
					let gloss = (!gloss_contour.is_identity()).then(|| (recorder.contour_samples(gloss_contour), gloss_antialiased));
					let light = bevel_light(angle, altitude, lengths_to_pixels);
					let (_, soften) = grow_and_tent(soften * length_scale, 0.);
					// How much brighter the slopes are lit, needed only by a gloss contour or the fade at the foot, without which the highlight and shadow are lit directly
					let lit = |recorder: &mut PassRecorder, rise: f64| (gloss.is_some() || fades_at_foot).then(|| recorder.bevel_lit(&height, &surface, rise, light, None));
					let amounts = |recorder: &mut PassRecorder, brighter: Option<&wgpu::Texture>, rise: f64, fades_at_foot: bool| {
						[false, true].map(|shadow| {
							let amount = match brighter {
								Some(brighter) => recorder.bevel_amount(brighter, &height, gloss.as_ref(), light.z, shadow, fades_at_foot),
								None => recorder.bevel_lit(&height, &surface, rise, light, Some(shadow)),
							};
							match soften >= NEGLIGIBLE_LENGTH {
								true => recorder.blur(amount, BlurKernel::Tent { radius: soften }, false),
								false => amount,
							}
						})
					};

					// The slopes inside the content are lit over it, and those outside lie beneath it, mirrored for a pillow so they rise away from the edge
					let brighter = lit(&mut recorder, rise);
					let inside = (style != BevelStyle::OuterBevel).then(|| amounts(&mut recorder, brighter.as_ref(), rise, false));
					let colors = [(highlight_color, highlight_blend_mode), (shadow_color, shadow_blend_mode)];
					if let Some(inside) = &inside {
						for (plane, (amount, (color, blend_mode))) in inside.iter().zip(colors).enumerate() {
							capture_plane(&mut recorder, plane, amount, false);
							match layered {
								true => {
									let tinted = recorder.tinted(amount, color);
									pieces.insert(piece(plane), tinted);
								}
								false => recorder.shade_within_by_mode(content, &mut full, &mut filled, amount, Some(color), blend_mode),
							}
						}
					}
					let outside = match style {
						BevelStyle::InnerBevel => None,
						BevelStyle::Emboss if !fades_at_foot => inside,
						BevelStyle::OuterBevel | BevelStyle::Emboss => {
							inside.into_iter().flatten().for_each(|amount| recorder.textures.release(amount));
							Some(amounts(&mut recorder, brighter.as_ref(), rise, fades_at_foot))
						}
						BevelStyle::PillowEmboss => {
							inside.into_iter().flatten().for_each(|amount| recorder.textures.release(amount));
							let mirrored = lit(&mut recorder, -rise);
							let outside = amounts(&mut recorder, mirrored.as_ref(), -rise, fades_at_foot);
							mirrored.into_iter().for_each(|mirrored| recorder.textures.release(mirrored));
							Some(outside)
						}
					};
					brighter.into_iter().for_each(|brighter| recorder.textures.release(brighter));
					for (plane, (amount, (color, _))) in outside.into_iter().flatten().zip(colors).enumerate() {
						let amount = recorder.knock_out(amount, full.as_ref().unwrap_or(content), knockout_fill);
						capture_plane(&mut recorder, plane + 2, &amount, false);
						recorder.place_beneath(layered.then_some((&mut pieces, piece(plane + 2))), &mut beneath, amount, Some(color));
					}
					recorder.textures.release(height);
				}
				EffectOperation::PaintOverlay { blend_mode, opacity, .. } => {
					let Some(paint) = paints.next() else { continue };
					let painted = recorder.scale(paint, (opacity / 100.) as f32);
					recorder.place_within(layered.then_some((&mut pieces, piece(0))), content, &mut full, &mut filled, painted, None, blend_mode);
				}
				// A solid paint overlay paints within the content alone, like any other paint overlay
				EffectOperation::Adjustment(Adjustment::ColorOverlay { color, blend_mode, opacity }) => {
					let parameters = Parameters {
						color: premultiplied_gamma_channels(color.with_alpha(color.a() * (opacity / 100.) as f32)),
						..Default::default()
					};
					let painted = recorder.run("flood", WORKING_FORMAT, recorder.size, full.as_ref().unwrap_or(content), None, None, parameters);
					recorder.place_within(layered.then_some((&mut pieces, piece(0))), content, &mut full, &mut filled, painted, None, blend_mode);
				}
				EffectOperation::Mask { mode, density, feather, invert, .. } => {
					let Some(mask) = paints.next() else { continue };

					// SVG takes the strength as a map where the mask graphic's own effects need the GPU
					let strength = recorder.mask_strength(mask, mode, density, mask_feather_sigma(feather) * length_scale, invert);
					capture_plane(&mut recorder, 0, &strength, false);

					if layers.trailing_masks.contains(&index) {
						layer_fade = Some(recorder.faded_together(layer_fade.take(), strength));
					} else {
						recorder.merge_beneath(content, &mut full, &mut filled, beneath.take());
						recorder.fade_by(content, &mut full, &mut filled, &strength);
						recorder.textures.release(strength);
					}
				}
				EffectOperation::Feather { distance } => {
					recorder.merge_beneath(content, &mut full, &mut filled, beneath.take());

					let strength = recorder.feather_strength(full.as_ref().unwrap_or(content), distance * length_scale);
					capture_plane(&mut recorder, 0, &strength, false);
					recorder.fade_by(content, &mut full, &mut filled, &strength);
					recorder.textures.release(strength);
				}
				EffectOperation::ConvolutionKernel { .. }
				| EffectOperation::Emboss { .. }
				| EffectOperation::FindEdges
				| EffectOperation::MinimumMaximum { .. }
				| EffectOperation::Sharpen { .. }
				| EffectOperation::HighPass { .. }
				| EffectOperation::AddNoise { .. }
				| EffectOperation::Dissolve { .. } => {
					recorder.merge_beneath(content, &mut full, &mut filled, beneath.take());
					let captured_plane = capture.filter(|capture| capture.index == index).map(|capture| capture.plane);
					recorder.filter(content, &mut full, &mut filled, operation, effect_to_pixels, captured_plane, &mut |recorder, plane, map| {
						capture_plane(recorder, plane, map, true)
					});
				}
				EffectOperation::Adjustment(_) => {
					// A run of adjustments is carried out from its first row, in a pass for each standalone step and one for the steps between them
					let is_adjustment = |position: usize| is_pixel_filter(&operations[position].1) && matches!(operations[position].1, EffectOperation::Adjustment(_));
					if position > 0 && is_adjustment(position - 1) {
						continue;
					}

					recorder.merge_beneath(content, &mut full, &mut filled, beneath.take());
					let run: Vec<&Adjustment> = (position..operations.len())
						.take_while(|&position| is_adjustment(position))
						.filter_map(|position| match &operations[position].1 {
							EffectOperation::Adjustment(adjustment) => Some(adjustment),
							_ => None,
						})
						.collect();

					// The content at full strength is read only by an operation after the run, or shown as the result where no fill fades it
					let full_is_read = filled.is_none() || position + run.len() < operations.len();

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

			if captured.is_some() && !capture_needs_content_layer {
				break;
			}
		}
		if let Some(captured) = captured.take_if(|_| !capture_needs_content_layer) {
			let textures = [full, filled, beneath, layer_fade].into_iter().flatten().chain(pieces.into_values());
			textures.for_each(|texture| recorder.textures.release(texture));
			queue.submit([recorder.encoder.finish()]);
			return vec![(captured, BlendMode::Normal)];
		}

		// A coverage the passes never reached, like that of a band too thin to draw at this resolution, covers nothing
		if capture.is_some_and(|capture| capture != EffectCapture::THROUGH_MASK) && captured.is_none() {
			let empty = recorder.scale(full.as_ref().unwrap_or(content), 0.);
			let textures = [full, filled, beneath, layer_fade].into_iter().flatten().chain(pieces.into_values());
			textures.for_each(|texture| recorder.textures.release(texture));
			queue.submit([recorder.encoder.finish()]);
			return vec![(empty, BlendMode::Normal)];
		}

		// The content's layer is finished first, since the knockouts and the parts showing through its fill read its alpha
		let content_layer = layers.content_layer();
		// The full-strength content becomes the content's image itself where no piece or capture reads it beside that image
		let mut content_image = match filled {
			Some(filled) => filled,
			None if pieces.is_empty() && !capture_needs_content_layer => full.take().unwrap_or_else(|| recorder.scale(content, 1.)),
			None => recorder.scale(full.as_ref().unwrap_or(content), 1.),
		};
		let full_content = full.as_ref().unwrap_or(content);

		// The passes working from the opacity read the trailing masks' fade of it beside the content's alpha, flagged in `values.w`
		let content_with_fade = layer_fade.as_ref().map(|fade| recorder.content_with_layer_fade(full_content, fade));
		let (fading_content, fades) = match &content_with_fade {
			Some(content_with_fade) => (content_with_fade, 1.),
			None => (full_content, 0.),
		};
		let cover_fades = |capture: EffectCapture| if layers.masks_fade_knockout_cover(capture) { fades } else { 0. };

		let content_parts = content_layer.into_iter().flat_map(|layer| &layers.layers[layer].parts);
		for piece in content_parts.filter_map(|part| match part {
			LayerPart::Piece(piece) => Some(piece),
			LayerPart::Content => None,
		}) {
			let part = match piece.place {
				PiecePlace::Within => 0.,
				PiecePlace::IntoContent { over: false } => 1.,
				PiecePlace::IntoContent { over: true } => 2.,
				PiecePlace::Beneath | PiecePlace::Through { .. } => continue,
			};
			let Some(tinted) = pieces.get(&piece.capture) else { continue };
			let parameters = Parameters {
				amount: opacity,
				values: [piece.blend_mode as u32 as f32, 1., part, fades],
				..Default::default()
			};
			let composited = recorder.run("composite_within", WORKING_FORMAT, recorder.size, &content_image, Some(tinted), Some(fading_content), parameters);
			recorder.replace(&mut content_image, composited);
		}

		// A capture that waited for the content's layer is a knocked-out coverage, or the share showing through the content's fill
		if capture_needs_content_layer {
			let output = match (captured, capture) {
				(Some(captured), Some(capture)) => {
					let parameters = Parameters {
						amount: layers.knockout_cover(capture, opacity),
						values: [0., 0., 0., cover_fades(capture)],
						..Default::default()
					};
					let knocked_out = recorder.run("knock_out_by_layer", WORKING_FORMAT, recorder.size, &captured, Some(&content_image), Some(fading_content), parameters);
					recorder.textures.release(captured);
					knocked_out
				}
				(None, Some(EffectCapture::THROUGH_MASK)) => {
					let parameters = Parameters {
						amount: opacity,
						values: [1., 0., 0., fades],
						..Default::default()
					};
					recorder.run("through_mask", WORKING_FORMAT, recorder.size, full_content, Some(&content_image), Some(fading_content), parameters)
				}
				// A coverage too small to draw, like an outline's band at a far zoom, covers nothing
				_ => recorder.run("flood", WORKING_FORMAT, recorder.size, full_content, None, None, Parameters::default()),
			};
			let textures = [full, Some(content_image), beneath, content_with_fade, layer_fade].into_iter().flatten().chain(pieces.into_values());
			textures.for_each(|texture| recorder.textures.release(texture));
			queue.submit([recorder.encoder.finish()]);
			return vec![(output, BlendMode::Normal)];
		}

		// The content's image moves into its layer, copied only when a layer over it still reads its alpha
		let reads_content_layer = |part: &LayerPart| {
			matches!(
				part,
				LayerPart::Piece(
					EffectPiece {
						place: PiecePlace::Through { .. },
						..
					} | EffectPiece { knocks_out: true, .. }
				)
			)
		};
		let read_after = (layers.layers.iter().skip(content_layer.map_or(usize::MAX, |layer| layer + 1))).any(|layer| layer.parts.iter().any(reads_content_layer));

		// Each layer stacks its parts in order: what lies beneath the content, then the content with what lies within it
		let mut content_image = Some(content_image);
		let mut results = Vec::new();
		for (index, layer) in layers.layers.iter().enumerate() {
			let holds_content = Some(index) == content_layer;
			let mut stack = if holds_content { beneath.take() } else { None };

			for part in &layer.parts {
				let LayerPart::Piece(part_piece) = part else { continue };
				// Only the knockouts and the parts through the fill read the content's layer, which is kept for them while a later layer holds one
				let with_content_layer = || pieces.get(&part_piece.capture).zip(content_image.as_ref());
				let upper = match part_piece.place {
					PiecePlace::Beneath if !knocks_out_by_layer(part_piece) => pieces.remove(&part_piece.capture),
					PiecePlace::Beneath => with_content_layer().map(|(tinted, content_image)| {
						let parameters = Parameters {
							amount: layers.knockout_cover(part_piece.capture, opacity),
							values: [0., 0., 0., cover_fades(part_piece.capture)],
							..Default::default()
						};
						recorder.run("knock_out_by_layer", WORKING_FORMAT, recorder.size, tinted, Some(content_image), Some(fading_content), parameters)
					}),
					PiecePlace::Within if !holds_content => {
						(pieces.get(&part_piece.capture)).map(|tinted| recorder.run("mask_within", WORKING_FORMAT, recorder.size, tinted, None, Some(full_content), Parameters::default()))
					}
					PiecePlace::Through { .. } => with_content_layer().map(|(tinted, content_image)| {
						let parameters = Parameters {
							amount: opacity,
							values: [0., 0., 0., fades],
							..Default::default()
						};
						recorder.run("through_mask", WORKING_FORMAT, recorder.size, tinted, Some(content_image), Some(fading_content), parameters)
					}),
					PiecePlace::Within | PiecePlace::IntoContent { .. } => continue,
				};
				if let Some(upper) = upper {
					stack = Some(recorder.stack_over(stack, upper));
				}
			}

			let image = match (holds_content, read_after) {
				(true, true) => content_image.as_ref().map(|image| recorder.scale(image, 1.)),
				(true, false) => content_image.take(),
				(false, _) => None,
			};
			let Some(mut result) = image.map(|image| recorder.stack_over(stack.take(), image)).or(stack) else {
				continue;
			};
			if let Some(fade) = &layer_fade {
				let faded = recorder.run("apply_mask", WORKING_FORMAT, recorder.size, &result, Some(fade), None, Parameters::default());
				recorder.replace(&mut result, faded);
			}
			results.push((result, layer.blend_mode));
		}

		let textures = [full, content_image, beneath, content_with_fade, layer_fade].into_iter().flatten().chain(pieces.into_values());
		textures.for_each(|texture| recorder.textures.release(texture));
		queue.submit([recorder.encoder.finish()]);
		results
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

		self.record_mipmaps(device, &mut recorder.encoder, recorder.textures, target);
		queue.submit([recorder.encoder.finish()]);
	}

	/// Records the averaging of each of `texture`'s smaller mip levels from the one above it, like the levels of an uploaded image.
	/// Each is halved into a pooled texture and copied into place, since WebGL sets the levels read for a whole texture, so none is drawn into while another is read.
	pub(crate) fn record_mipmaps(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, textures: &mut EffectTextures, texture: &wgpu::Texture) {
		let format = texture.format();
		let mut above: Option<wgpu::Texture> = None;

		for index in 1..texture.mip_level_count() {
			let level_size = UVec2::new(texture.width() >> index, texture.height() >> index).max(UVec2::ONE);
			let level = textures.acquire(device, level_size, format);

			// The first level reads the top one through a view of every level
			let source = above.as_ref().unwrap_or(texture).create_view(&wgpu::TextureViewDescriptor::default());
			let target = level.create_view(&wgpu::TextureViewDescriptor::default());
			self.record(device, encoder, "halve", format, &target, level_size, &source, None, None, Parameters::default());

			let extent = wgpu::Extent3d {
				width: level_size.x,
				height: level_size.y,
				depth_or_array_layers: 1,
			};
			let destination = wgpu::TexelCopyTextureInfo {
				mip_level: index,
				..texture.as_image_copy()
			};
			encoder.copy_texture_to_texture(level.as_image_copy(), destination, extent);

			if let Some(above) = above.replace(level) {
				textures.release(above);
			}
		}
		above.into_iter().for_each(|texture| textures.release(texture));
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

/// Whether an operation leaves the content as it is, by its settings alone or as a blur or bevel too slight to see at this scale.
fn changes_nothing(operation: &EffectOperation, length_scale: f64) -> bool {
	// Each drawn paint is matched to its operation by order, so only those without one are left out for their size here
	let too_slight = match operation {
		EffectOperation::Blur { radius, .. } => radius * length_scale < NEGLIGIBLE_LENGTH,
		EffectOperation::BevelAndEmboss { size, gloss_contour, .. } => size * length_scale < NEGLIGIBLE_LENGTH && gloss_contour.is_identity(),
		_ => false,
	};
	operation.changes_nothing() || (too_slight && operation.drawn_paint().is_none())
}

/// Which pixels seed a distance pass, and how far from each one its edge lies.
#[derive(Clone, Copy)]
enum Seeds {
	/// The covered pixels, with their antialiased edges placed by coverage, matching `distances_to_silhouette`.
	Covered,
	/// The pixels of a distance texture lying up to 3 past a grow of so many pixels, matching `distances_through_seeds` of how far short of the grown edge they lie.
	PastGrow { grow: f64 },
	/// Every pixel, with its value as a head start, matching `distances_through_seeds` of values that are all finite.
	Weighted,
	/// Like `Covered`, but measured in straight lines rather than by `chamfer_length`.
	CoveredStraight,
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
		self.shape(strength, range, contour)
	}

	/// The strength from 0 to 1 of a shadow or glow cast from a `silhouette`'s coverage before its range and contour shape it, as [`Self::cast`] describes.
	fn cast_strength(&mut self, silhouette: wgpu::Texture, size: f64, spread: f64, technique: GlowTechnique, length_scale: f64) -> wgpu::Texture {
		let (grow, tent) = grow_and_tent(size, spread);
		let grow_pixels = grow * length_scale;

		match technique {
			GlowTechnique::Softer => {
				let mut coverage = silhouette;
				if grow_pixels >= NEGLIGIBLE_LENGTH {
					coverage = self.grown(coverage, grow_pixels);
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

	/// The `silhouette` grown outward by `distance` pixels, matching the CPU's `grown`.
	fn grown(&mut self, silhouette: wgpu::Texture, distance: f64) -> wgpu::Texture {
		let to_silhouette = self.distances(&silhouette, distance + 1., Seeds::Covered);
		self.textures.release(silhouette);

		let parameters = Parameters {
			values: [distance as f32, 0., 0., 0.],
			..Default::default()
		};
		let grown = self.run("grow", COVERAGE_FORMAT, self.size, &to_silhouette, None, None, parameters);
		self.textures.release(to_silhouette);
		grown
	}

	/// The `strength` shaped as [`Self::shaped`] describes, in place of the unshaped texture.
	fn shape(&mut self, strength: wgpu::Texture, range: f64, contour: &TransferCurve) -> wgpu::Texture {
		match self.shaped(&strength, range, contour) {
			Some(shaped) => {
				self.textures.release(strength);
				shaped
			}
			None => strength,
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
	/// Jump flooding finds them in a pass for each halving of a step that starts past `reach`, then two more halvings and those of [`chamfer_passes`] along the chamfer steps. O(width × height × log(reach)).
	fn distances(&mut self, seed_values: &wgpu::Texture, reach: f64, seeds: Seeds) -> wgpu::Texture {
		let (mode, grow, straight) = match seeds {
			Seeds::Covered => (0., 0., 0.),
			Seeds::PastGrow { grow } => (1., grow as f32, 0.),
			Seeds::Weighted => (2., 0., 0.),
			Seeds::CoveredStraight => (0., 0., 1.),
		};
		let parameters = |step: u32| Parameters {
			values: [step as f32, mode, grow, 0.],
			amount: straight,
			..Default::default()
		};

		let mut nearest = self.run("jump_flood_seed", WORKING_FORMAT, self.size, seed_values, None, Some(seed_values), parameters(0));
		let first_step = (reach.clamp(0., self.size.max_element() as f64).ceil() as u32 + 1).next_power_of_two();
		let halvings = std::iter::successors(Some(first_step), |&step| (step > 1).then_some(step / 2));
		for step in halvings.chain([2, 1]) {
			let flooded = self.run("jump_flood_step", WORKING_FORMAT, self.size, &nearest, None, Some(seed_values), parameters(step));
			self.replace(&mut nearest, flooded);
		}
		let chamfer_parameters = Parameters {
			values: [1., mode, grow, 1.],
			amount: straight,
			..Default::default()
		};
		for _ in 0..chamfer_passes(reach) {
			let flooded = self.run("jump_flood_step", WORKING_FORMAT, self.size, &nearest, None, Some(seed_values), chamfer_parameters);
			self.replace(&mut nearest, flooded);
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

	/// Places what lies `beneath` under the full-strength content (`content` itself until a pass replaces it) and the faded fill,
	/// so a pixel filter after it (a blur, an adjustment, a mask, or a feather) processes them together.
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

	/// A coverage tinted `color`.
	fn tinted(&mut self, coverage: &wgpu::Texture, color: Color) -> wgpu::Texture {
		let parameters = Parameters {
			color: premultiplied_gamma_channels(color),
			..Default::default()
		};
		self.run("tint", WORKING_FORMAT, self.size, coverage, None, None, parameters)
	}

	/// A coverage tinted `color`, freeing the coverage, or with no color the already tinted image itself.
	fn tint_consuming(&mut self, coverage: wgpu::Texture, color: Option<Color>) -> wgpu::Texture {
		let Some(color) = color else { return coverage };
		let tinted = self.tinted(&coverage, color);
		self.textures.release(coverage);
		tinted
	}

	/// The `upper` image stacked over the `stack` if there is one, freeing both.
	fn stack_over(&mut self, stack: Option<wgpu::Texture>, upper: wgpu::Texture) -> wgpu::Texture {
		let Some(stack) = stack else { return upper };
		let stacked = self.composite_beneath(&upper, &stack);
		self.textures.release(upper);
		self.textures.release(stack);
		stacked
	}

	/// Places a coverage tinted `color` (or with no color, already tinted) beneath the content: as the piece `destination` names among the layered pieces, or else stacked beneath the content's image. Frees the coverage.
	fn place_beneath(&mut self, destination: Option<(&mut HashMap<EffectCapture, wgpu::Texture>, EffectCapture)>, beneath: &mut Option<wgpu::Texture>, coverage: wgpu::Texture, color: Option<Color>) {
		match destination {
			Some((pieces, capture)) => {
				let tinted = self.tint_consuming(coverage, color);
				pieces.insert(capture, tinted);
			}
			None => {
				*beneath = Some(self.stack_beneath(beneath.take(), &coverage, color));
				self.textures.release(coverage);
			}
		}
	}

	/// Places a coverage tinted `color` (or with no color, already tinted) within the content: as the piece `destination` names among the layered pieces,
	/// or else over the content's image, mixed with its colors by the blend mode. Frees the coverage.
	#[expect(clippy::too_many_arguments)]
	fn place_within(
		&mut self,
		destination: Option<(&mut HashMap<EffectCapture, wgpu::Texture>, EffectCapture)>,
		content: &wgpu::Texture,
		full: &mut Option<wgpu::Texture>,
		filled: &mut Option<wgpu::Texture>,
		coverage: wgpu::Texture,
		color: Option<Color>,
		blend_mode: BlendMode,
	) {
		match destination {
			Some((pieces, capture)) => {
				let tinted = self.tint_consuming(coverage, color);
				pieces.insert(capture, tinted);
			}
			None => {
				self.shade_within_by_mode(content, full, filled, &coverage, color, blend_mode);
				self.textures.release(coverage);
			}
		}
	}

	/// Places a coverage over both the full-strength content (`content` itself until a pass replaces it) and the faded fill, within the content,
	/// tinted by `color` mixed with the content's colors by the blend mode in gamma (or with no color, already tinted).
	fn shade_within_by_mode(
		&mut self,
		content: &wgpu::Texture,
		full: &mut Option<wgpu::Texture>,
		filled: &mut Option<wgpu::Texture>,
		coverage: &wgpu::Texture,
		color: Option<Color>,
		blend_mode: BlendMode,
	) {
		let parameters = Parameters {
			color: color.map_or([0.; 4], premultiplied_gamma_channels),
			values: [blend_mode as u32 as f32, if color.is_none() { 1. } else { 0. }, 0., 0.],
			..Default::default()
		};
		let full_content = full.as_ref().unwrap_or(content);
		if let Some(filled) = filled {
			let composited = self.run("composite_within", WORKING_FORMAT, self.size, filled, Some(coverage), Some(full_content), parameters);
			self.replace(filled, composited);
		}
		let composited = self.run("composite_within", WORKING_FORMAT, self.size, full_content, Some(coverage), Some(full_content), parameters);
		self.replace_or_fill(full, composited);
	}

	/// A bevel's height over the `style`'s slopes `size` pixels wide, shaped by the `technique`, from -0.5 at their foot to 0.5 at their top, with how many pixels they rise.
	/// It's cast from the `content`'s silhouette and centered on the edge, where half floats are finest. Matches the CPU's `bevel_height`.
	fn bevel_height(&mut self, content: &wgpu::Texture, style: BevelStyle, technique: BevelTechnique, size: f64) -> (wgpu::Texture, f64) {
		let span = match style {
			BevelStyle::InnerBevel | BevelStyle::OuterBevel => size,
			BevelStyle::Emboss | BevelStyle::PillowEmboss => size / 2.,
		};

		// Slopes too narrow to see leave a flat surface, of the coverage less a half, which only a gloss contour shades
		if span < NEGLIGIBLE_LENGTH {
			let parameters = Parameters { amount: 0.5, ..Default::default() };
			return (self.run("cast_silhouette", COVERAGE_FORMAT, self.size, content, None, None, parameters), 0.);
		}

		// A smooth bevel blurs the coverage less a half, cast that way
		if technique == BevelTechnique::Smooth {
			let (_, tent) = grow_and_tent(span, 0.);
			let parameters = Parameters { amount: 0.5, ..Default::default() };
			let centered = self.run("cast_silhouette", COVERAGE_FORMAT, self.size, content, None, None, parameters);
			return (self.blur(centered, BlurKernel::Tent { radius: tent }, false), span);
		}
		let silhouette = self.silhouette(content, DVec2::ZERO, false);

		// The height spans only the part across the edge the style lights
		let near = if technique == BevelTechnique::ChiselSoft { 3. } else { 2. };
		let (low, high) = match style {
			BevelStyle::InnerBevel => (-near, span + 1.),
			BevelStyle::OuterBevel => (-span - 1., near),
			BevelStyle::Emboss | BevelStyle::PillowEmboss => (-span - 1., span + 1.),
		};

		let (inward, outward) = match technique {
			// A soft chisel measures from a slope-limited matte, each pixel's coverage away from half giving its distance across the edge a head start
			BevelTechnique::ChiselSoft => {
				let reach = span + 1.;
				let distances = |recorder: &mut Self, sign: f64, distance_reach: f64| {
					let parameters = Parameters {
						amount: (sign * 2. * reach) as f32,
						..Default::default()
					};
					let head_starts = recorder.run("slope_limited_matte", COVERAGE_FORMAT, recorder.size, &silhouette, None, None, parameters);
					let distances = recorder.distances(&head_starts, distance_reach, Seeds::Weighted);
					recorder.textures.release(head_starts);
					distances
				};
				(distances(self, 1., high + 1.), distances(self, -1., -low + 1.))
			}
			_ => {
				let outside = self.run("invert_coverage", COVERAGE_FORMAT, self.size, &silhouette, None, None, Parameters::default());
				let inward = self.distances(&outside, high + 1., Seeds::Covered);
				self.textures.release(outside);
				(inward, self.distances(&silhouette, -low + 1., Seeds::Covered))
			}
		};

		let parameters = Parameters {
			values: [low as f32, high as f32, if technique == BevelTechnique::ChiselSoft { 1. } else { 0. }, (span + 1.) as f32],
			..Default::default()
		};
		let height = self.run("chisel_height", COVERAGE_FORMAT, self.size, &inward, Some(&outward), None, parameters);
		[inward, outward, silhouette].into_iter().for_each(|texture| self.textures.release(texture));
		(height, span)
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
		let mut tap_array = [[0.; 4]; PARAMETER_TAPS];
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
	fn the_shaders_parameters_hold_as_many_taps_as_the_passes_write() {
		assert!(include_str!("vello_effects.wgsl").contains(&format!("taps: array<vec4<f32>, {PARAMETER_TAPS}>")));
	}

	#[test]
	fn kernels_keep_brightness_and_fit_one_pass() {
		let kernels = [0.3, 1., 2.5, 7.9, 8., 12., 100., 4000.]
			.into_iter()
			.flat_map(|length| [BlurKernel::Gaussian { sigma: length }, BlurKernel::Box { radius: length }]);

		for kernel in kernels {
			let taps = kernel_taps(kernel, kernel.levels());

			assert!(
				taps.len() <= 1 + 2 * raster_kernels::effect_kernels::MAX_KERNEL_REACH.div_ceil(2),
				"{kernel:?} needs {} taps",
				taps.len()
			);
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
			radius_mask: Default::default(),
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
		let apply = |textures: &mut EffectTextures, effects: &Effects| {
			textures.drawn_passes.clear();
			let layers = EffectLayers::plan(effects, BlendMode::Normal, 1., false);
			let results = passes.apply(&context.device, &queue, textures, &content, effects, &layers, 1., 1., DAffine2::IDENTITY, &[], None);
			results.into_iter().for_each(|(result, _)| textures.release(result));
		};
		let draw = |textures: &mut EffectTextures| {
			apply(textures, &effects);
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
			let effects = Effects::from_operations(operations);
			let layers = EffectLayers::plan(&effects, BlendMode::Normal, fill, false);
			let results = passes.apply(&context.device, &queue, &mut textures, &content, &effects, &layers, fill, 1., DAffine2::IDENTITY, &[], None);
			results.into_iter().for_each(|(result, _)| textures.release(result));
			textures.drawn_passes.clone()
		};
		let second_run: Vec<EffectOperation> = second_run.iter().cloned().map(EffectOperation::Adjustment).collect();
		assert_eq!(drawn(second_run.clone(), 1.), ["adjust"]);

		// With a fill, the run adjusts the faded content alone unless an operation after it reads the content at full strength
		assert_eq!(drawn(second_run.clone(), 0.5), ["scale", "adjust"]);
		let shadow = EffectOperation::DropShadow {
			color: Color::BLACK,
			blend_mode: BlendMode::Normal,
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
	fn effects_blending_normally_at_a_full_fill_take_no_more_passes_when_layered() {
		use graphic_types::effects::EffectKind;

		let Some(context) = futures::executor::block_on(crate::WgpuContext::for_tests()) else { return };
		let passes = EffectPasses::new(&context.device, &context.adapter);
		if !passes.renders_working_formats {
			return;
		}

		let effects = Effects::from_operations([EffectOperation::default_for(EffectKind::DropShadow), EffectOperation::default_for(EffectKind::InnerShadow)]);
		let mut textures = EffectTextures::default();
		let content = textures.acquire(&context.device, UVec2::splat(16), wgpu::TextureFormat::Rgba8Unorm);
		let queue = context.queue.lock();
		let mut drawn = |layered: bool| {
			textures.drawn_passes.clear();
			let layers = EffectLayers::plan(&effects, BlendMode::Normal, 1., layered);
			let results = passes.apply(&context.device, &queue, &mut textures, &content, &effects, &layers, 1., 1., DAffine2::IDENTITY, &[], None);
			results.into_iter().for_each(|(result, _)| textures.release(result));
			textures.drawn_passes.clone()
		};
		assert_eq!(drawn(true), drawn(false));
	}

	#[test]
	fn a_bevel_without_a_gloss_contour_or_texture_lights_its_highlight_and_shadow_directly() {
		use graphic_types::effects::EffectKind;

		let Some(context) = futures::executor::block_on(crate::WgpuContext::for_tests()) else { return };
		let passes = EffectPasses::new(&context.device, &context.adapter);
		if !passes.renders_working_formats {
			return;
		}

		let effects = Effects::from_operations([EffectOperation::default_for(EffectKind::BevelAndEmboss)]);
		let mut textures = EffectTextures::default();
		let content = textures.acquire(&context.device, UVec2::splat(16), wgpu::TextureFormat::Rgba8Unorm);
		let queue = context.queue.lock();
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 1., false);
		let results = passes.apply(&context.device, &queue, &mut textures, &content, &effects, &layers, 1., 1., DAffine2::IDENTITY, &[], None);
		results.into_iter().for_each(|(result, _)| textures.release(result));

		let drawn = |label: &str| textures.drawn_passes.iter().filter(|&&drawn| drawn == label).count();
		assert_eq!((drawn("bevel_lit"), drawn("bevel_amount")), (2, 0));
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
