//! Carries out an item's effects on a GPU raster of its content with the kernels of `raster_kernels::effects`,
//! for the renderer drawing an effects group and for the nodes and the Eager node carrying out a raster's effects into its pixels.

use crate::WgpuPipelineCache;
use crate::raster_kernel::{MAX_FRAME_SIDE, adjust_raster, blur_raster, run_raster_kernel};
use crate::shader_runtime::raster_kernel_runtime::RasterKernelInput;
use core_types::list::{ATTR_EFFECTS, Item};
use core_types::math::quad::Quad;
use core_types::ops::Convert;
use core_types::transform::Footprint;
use core_types::{ATTR_OPACITY_FILL, ATTR_TRANSFORM, Color, NodeError};
use glam::{DAffine2, DMat2, DVec2, UVec2};
use graphic_types::effects::{bevel_light, bevel_smoothing, fitted_paint, outline_extents};
use graphic_types::{Adjustment, BevelDirection, EffectOperation, Effects, GlowSource, Graphic};
use raster_kernels::RasterKernel;
use raster_kernels::adjustments::{leading_adjustments, preserves_alpha};
use raster_kernels::blur::BlurSpace;
use raster_kernels::effects::*;
use raster_types::{CPU, GPU, Raster, Storage};
use rendering::vello_scene_builder::VelloSceneBuilder;
use rendering::{Render, RenderParams};

/// Blurs and spreads shorter than this, in pixels, leave no visible trace.
pub const NEGLIGIBLE_LENGTH: f64 = 0.01;

/// The most pixels a raster may grow to with the margin its effects cast into, beyond which the margin is cut short.
const MAX_PIXELS_WITH_MARGIN: f64 = (1 << 24) as f64;

/// The raster with its effects carried out into its pixels, its coverage grown to hold what they cast past its edges, and with the effects and the fill opacity they used removed.
/// A raster without the effects attribute is returned as it is.
pub fn evaluate_raster_effects(item: Item<Raster<GPU>>, pipeline: &WgpuPipelineCache) -> Result<Item<Raster<GPU>>, NodeError> {
	let Some(operations) = item.attribute::<Effects>(ATTR_EFFECTS).map(|effects| effects.operations().collect::<Vec<_>>()) else {
		return Ok(item);
	};
	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let fill = item.attribute_cloned_or::<f64>(ATTR_OPACITY_FILL, 1.) as f32;

	let (raster, attributes) = item.into_parts();
	let raster = evaluate(&raster, transform, &operations, fill, pipeline)?.unwrap_or(raster);

	// Which attributes come out depends only on whether the effects attribute came in, not on what it holds
	let mut item = Item::from_parts(raster, attributes);
	item.remove_attribute::<Effects>(ATTR_EFFECTS);
	item.remove_attribute::<f64>(ATTR_OPACITY_FILL);
	item.set_attribute(ATTR_TRANSFORM, transform);
	Ok(item)
}

/// The CPU raster uploaded with its effects carried out into its pixels, for the Eager node and the input adapter that feeds a CPU raster to a node taking only GPU rasters.
pub fn upload_raster(image: Item<Raster<CPU>>, pipeline: &WgpuPipelineCache) -> Result<Item<Raster<GPU>>, NodeError> {
	let executor = pipeline.executor().cloned().ok_or_else(|| NodeError::new("A GPU is required to upload a raster"))?;

	let (image, attributes) = image.into_parts();
	let uploaded: Raster<GPU> = image.convert(Footprint::DEFAULT, executor);

	evaluate_raster_effects(Item::from_parts(uploaded, attributes), pipeline)
}

/// The larger raster of the effects carried out on `raster`, an item's whose `transform` is kept, with the coverage placing that larger raster where the original's pixels were.
/// `None` when there's nothing to carry out, or the raster is empty or its placement can't be inverted.
fn evaluate(raster: &Raster<GPU>, transform: DAffine2, operations: &[EffectOperation], fill: f32, pipeline: &WgpuPipelineCache) -> Result<Option<Raster<GPU>>, NodeError> {
	let size = raster.size();
	let coverage = raster.coverage();
	let placement = transform * coverage;
	if (operations.is_empty() && fill >= 1.) || size.x == 0 || size.y == 0 || placement.matrix2.determinant() == 0. {
		return Ok(None);
	}

	// Leading adjustments change the raster's own colors, keeping even transparent pixels' colors
	let leading = leading_adjustments(operations.iter().cloned(), fill);
	let operations = &operations[leading.len()..];
	let mut adjusted = raster.clone();
	for adjustment in &leading {
		adjusted = adjust_raster(adjustment, adjusted, pipeline)?;
	}
	if operations.is_empty() && fill >= 1. {
		return Ok(Some(adjusted));
	}
	let raster = &adjusted;

	// Lengths in the document map onto the raster's own pixels, through which the margin the effects cast into is measured along each axis
	let lengths_to_pixels = DMat2::from_diagonal(size.as_dvec2()) * placement.matrix2.inverse();
	let reach: f64 = operations.iter().map(EffectOperation::reach).sum();
	let pixels_along = |row: DVec2| reach * row.length();
	let mut margin = DVec2::new(pixels_along(lengths_to_pixels.row(0)), pixels_along(lengths_to_pixels.row(1))).ceil();

	// A margin that would grow the raster past its limits is cut short, leaving what reaches farther out
	let grown = |margin: DVec2| size.as_dvec2() + 2. * margin;
	let area = grown(margin).x * grown(margin).y;
	if area > MAX_PIXELS_WITH_MARGIN {
		margin = (margin * (MAX_PIXELS_WITH_MARGIN / area).sqrt()).floor();
	}
	margin = margin.min(((DVec2::splat(MAX_FRAME_SIDE as f64) - size.as_dvec2()) / 2.).max(DVec2::ZERO).floor());
	let grown_size = grown(margin).as_uvec2();

	// A paint is placed in the space the raster's item is placed in, or fitted to the raster's extent there, and rendered over the grown raster's pixels
	let parent_to_pixels = DAffine2::from_translation(margin) * DAffine2::from_scale(size.as_dvec2()) * placement.inverse();
	let bounds = (placement * Quad::from_box([DVec2::ZERO, DVec2::ONE])).bounding_box();
	let mut render_paint = |_: usize, paint: &Graphic| {
		let executor = pipeline.executor().ok_or_else(|| NodeError::new("A GPU is required to render a paint"))?;
		let fitted = fitted_paint(paint, bounds);
		let paint = fitted.as_ref().unwrap_or(paint);
		let render_params = RenderParams::default().for_pattern();
		let mut draw = |scene: &mut VelloSceneBuilder| paint.render_to_vello(scene, DAffine2::IDENTITY, &render_params);
		let texture = executor
			.render_vello_raster(grown_size, parent_to_pixels, &mut draw)
			.map_err(|error| NodeError::new(error.to_string()))?;
		Ok(Raster::<GPU>::new_gpu(texture))
	};

	let mut pass = EffectsPass::new(raster, grown_size, DAffine2::from_translation(-margin), fill, lengths_to_pixels, pipeline)?;
	for operation in operations {
		pass.apply(operation, &mut render_paint)?;
	}
	let result = pass.finish()?;

	let grown_coverage = coverage * DAffine2::from_scale(1. / size.as_dvec2()) * DAffine2::from_translation(-margin) * DAffine2::from_scale(grown_size.as_dvec2());
	Ok(Some(result.with_coverage(grown_coverage)))
}

/// Carries out `operations` in order on `content`, a straight-alpha raster in the pixels `lengths_to_pixels` maps the effects' lengths onto, into a raster of its size.
/// `fill` fades the content but not the shadows and glows cast from it, and `render_paint` renders each paint overlay's paint, by its index among them, over the same pixels.
pub fn apply_effects(
	content: &Raster<GPU>,
	operations: impl IntoIterator<Item = EffectOperation>,
	fill: f32,
	lengths_to_pixels: DMat2,
	render_paint: &mut dyn FnMut(usize, &Graphic) -> Result<Raster<GPU>, NodeError>,
	pipeline: &WgpuPipelineCache,
) -> Result<Raster<GPU>, NodeError> {
	let mut pass = EffectsPass::new(content, content.size(), DAffine2::IDENTITY, fill, lengths_to_pixels, pipeline)?;
	for operation in operations {
		pass.apply(&operation, render_paint)?;
	}
	Ok(pass.finish()?.with_coverage(content.coverage()))
}

/// The images of an effects group, all of one size and in the premultiplied gamma-encoded channels the kernels work with, as its effects are carried out in turn.
pub struct EffectsPass<'a> {
	pipeline: &'a WgpuPipelineCache,
	lengths_to_pixels: DMat2,
	size: UVec2,
	/// The content at full strength, which casts every silhouette.
	full: Raster<GPU>,
	/// The content faded by the fill, which shows in the result, when the fill fades it.
	filled: Option<Raster<GPU>>,
	/// The shadows and glows stacked beneath the content.
	beneath: Option<Raster<GPU>>,
	/// How many paint overlays have been carried out.
	paint_overlays: usize,
}

impl<'a> EffectsPass<'a> {
	/// Starts the effects of the straight-alpha `content`, placed by `output_to_texel` in images of `size`, whose pixels `lengths_to_pixels` maps the effects' lengths onto.
	pub fn new(content: &Raster<GPU>, size: UVec2, output_to_texel: DAffine2, fill: f32, lengths_to_pixels: DMat2, pipeline: &'a WgpuPipelineCache) -> Result<Self, NodeError> {
		let encode = |amount| run_raster_kernel(&EffectEncode { amount }, vec![RasterKernelInput { raster: content, output_to_texel }], size, pipeline);
		let full = encode(1.)?;
		let filled = if fill < 1. { Some(encode(fill)?) } else { None };

		Ok(Self {
			pipeline,
			lengths_to_pixels,
			size,
			full,
			filled,
			beneath: None,
			paint_overlays: 0,
		})
	}

	/// Carries out the next effect, with `render_paint` rendering a paint overlay's paint, by its index among them, into a straight-alpha raster over the images' pixels.
	pub fn apply(&mut self, operation: &EffectOperation, render_paint: &mut dyn FnMut(usize, &Graphic) -> Result<Raster<GPU>, NodeError>) -> Result<(), NodeError> {
		let lengths_to_pixels = self.lengths_to_pixels;
		let length_scale = lengths_to_pixels.determinant().abs().sqrt();

		match *operation {
			EffectOperation::Blur { radius, box_blur, gamma } => {
				let radius = radius * length_scale;
				if radius < NEGLIGIBLE_LENGTH {
					return Ok(());
				}

				// What lies beneath blurs together with the content
				if let Some(beneath) = self.beneath.take() {
					if let Some(filled) = &self.filled {
						self.filled = Some(self.composite_beneath(filled, &beneath, &self.full)?);
					}
					self.full = self.composite_beneath(&self.full, &beneath, &self.full)?;
				}

				self.full = self.blur_image(&self.full, radius, box_blur, gamma)?;
				if let Some(filled) = &self.filled {
					self.filled = Some(self.blur_image(filled, radius, box_blur, gamma)?);
				}
			}
			EffectOperation::DropShadow { color, offset, blur, spread } => {
				let coverage = self.cast(lengths_to_pixels * offset, blur * length_scale, spread * length_scale, false)?;
				self.accumulate(&coverage, color)?;
			}
			EffectOperation::OuterGlow { color, blur, spread } => {
				let coverage = self.cast(DVec2::ZERO, blur * length_scale, spread * length_scale, false)?;
				self.accumulate(&coverage, color)?;
			}
			EffectOperation::InnerShadow { color, offset, blur, spread } => {
				let coverage = self.cast(lengths_to_pixels * offset, blur * length_scale, spread * length_scale, true)?;
				self.shade_within(&coverage, color)?;
			}
			EffectOperation::InnerGlow { color, blur, choke, source } => {
				let mut coverage = self.cast(DVec2::ZERO, blur * length_scale, choke * length_scale, true)?;
				if source == GlowSource::Center {
					coverage = self.run(&InvertCoverage {}, &[&coverage])?;
				}
				self.shade_within(&coverage, color)?;
			}
			EffectOperation::Satin { color, offset, blur, invert } => {
				let offset = lengths_to_pixels * offset;
				let forward = self.cast(offset, blur * length_scale, 0., false)?;
				let backward = self.cast(-offset, blur * length_scale, 0., false)?;
				let coverage = self.run(&SatinShading { invert }, &[&forward, &backward])?;
				self.shade_within(&coverage, color)?;
			}
			EffectOperation::Outline { color, weight, align } => {
				let (outside, inside) = outline_extents(weight * length_scale, align);
				if outside >= NEGLIGIBLE_LENGTH {
					let coverage = self.edge_band(outside, false)?;
					self.accumulate(&coverage, color)?;
				}
				if inside >= NEGLIGIBLE_LENGTH {
					let coverage = self.edge_band(inside, true)?;
					self.shade_within(&coverage, color)?;
				}
			}
			EffectOperation::BevelAndEmboss {
				style,
				depth,
				direction,
				size,
				soften,
				angle,
				altitude,
				highlight_color,
				shadow_color,
			} => {
				let (size, soften) = (size * length_scale, soften * length_scale);
				if size < NEGLIGIBLE_LENGTH {
					return Ok(());
				}

				// The surface's height, rounded by a blur, lit from the light's direction into its highlights and shadows
				let to_inside = self.nearest_across_edge(size, false)?;
				let to_outside = self.nearest_across_edge(size, true)?;
				let kernel = BevelHeight {
					size: size as f32,
					style: style as u32,
				};
				let mut height = self.run(&kernel, &[&to_inside, &to_outside])?;
				if bevel_smoothing(size) >= NEGLIGIBLE_LENGTH {
					height = blur_raster(&height, bevel_smoothing(size), false, BlurSpace::Premultiplied, self.pipeline)?;
				}
				let slope = (depth / 100. * size * if direction == BevelDirection::Down { -1. } else { 1. }) as f32;
				let light = bevel_light(angle, altitude, lengths_to_pixels).as_vec3();

				// Their parts outside the content lie beneath it, and those inside are placed over it
				for (shadow, color) in [(false, highlight_color), (true, shadow_color)] {
					let kernel = BevelShading {
						slope,
						light_x: light.x,
						light_y: light.y,
						light_z: light.z,
						shadow,
					};
					let mut coverage = self.run(&kernel, &[&height])?;
					if soften >= NEGLIGIBLE_LENGTH {
						coverage = blur_raster(&coverage, soften, false, BlurSpace::Premultiplied, self.pipeline)?;
					}

					self.accumulate(&coverage, color)?;
					self.shade_within(&coverage, color)?;
				}
			}
			EffectOperation::PaintOverlay { ref paint, blend_mode, opacity } => {
				let paint = render_paint(self.paint_overlays, paint)?;
				self.paint_overlays += 1;

				let overlay = PaintOverlay {
					blend_mode,
					opacity: (opacity / 100.) as f32,
				};
				self.full = self.run(&overlay, &[&self.full, &paint])?;
				if let Some(filled) = &self.filled {
					self.filled = Some(self.run(&overlay, &[filled, &paint])?);
				}
			}
			EffectOperation::Adjustment(ref adjustment) => {
				self.full = self.adjust_image(&self.full, adjustment)?;
				if let Some(filled) = &self.filled {
					self.filled = Some(self.adjust_image(filled, adjustment)?);
				}
			}
		}

		Ok(())
	}

	/// The straight-alpha raster of the content with its effects carried out: the image shown, with the shadows and glows beneath it.
	pub fn finish(self) -> Result<Raster<GPU>, NodeError> {
		let shown = self.filled.as_ref().unwrap_or(&self.full);
		let result = match &self.beneath {
			Some(beneath) => self.composite_beneath(shown, beneath, &self.full)?,
			None => shown.clone(),
		};
		self.run(&EffectDecode {}, &[&result])
	}

	/// Runs `kernel` on `inputs`, each at the images' own placement, into an image of the images' size.
	fn run<K: RasterKernel>(&self, kernel: &K, inputs: &[&Raster<GPU>]) -> Result<Raster<GPU>, NodeError> {
		let inputs = inputs
			.iter()
			.map(|raster| RasterKernelInput {
				raster,
				output_to_texel: DAffine2::IDENTITY,
			})
			.collect();
		run_raster_kernel(kernel, inputs, self.size, self.pipeline)
	}

	/// Carries out an adjustment on an image's straight colors, leaving its fully transparent pixels empty since they keep no color to adjust.
	fn adjust_image(&self, image: &Raster<GPU>, adjustment: &Adjustment) -> Result<Raster<GPU>, NodeError> {
		let decoded = self.run(&EffectDecode {}, &[image])?;
		let adjusted = adjust_raster(adjustment, decoded, self.pipeline)?;
		let encoded = self.run(&EffectEncode { amount: 1. }, &[&adjusted])?;

		// Only an adjustment that changes alpha can put anything in them
		if preserves_alpha(adjustment) {
			return Ok(encoded);
		}
		self.run(&KeepEmpty {}, &[&encoded, image])
	}

	/// Blurs an image by `radius` pixels, in linear light unless `gamma`.
	fn blur_image(&self, image: &Raster<GPU>, radius: f64, box_blur: bool, gamma: bool) -> Result<Raster<GPU>, NodeError> {
		// The images' premultiplied gamma-encoded channels are what a blur in gamma space sums
		if gamma {
			return blur_raster(image, radius, box_blur, BlurSpace::Premultiplied, self.pipeline);
		}

		let decoded = self.run(&EffectDecode {}, &[image])?;
		let blurred = blur_raster(&decoded, radius, box_blur, BlurSpace::Linear, self.pipeline)?;
		self.run(&EffectEncode { amount: 1. }, &[&blurred])
	}

	/// The coverage of the content's silhouette (or of the area outside it, if `inverse`), cast by `offset`, grown by `spread` and softened by `blur`, all in pixels.
	fn cast(&self, offset: DVec2, blur: f64, spread: f64, inverse: bool) -> Result<Raster<GPU>, NodeError> {
		let input = RasterKernelInput {
			raster: &self.full,
			output_to_texel: DAffine2::from_translation(-offset),
		};
		let mut coverage = run_raster_kernel(&CastSilhouette { inverse }, vec![input], self.size, self.pipeline)?;

		if spread >= NEGLIGIBLE_LENGTH {
			coverage = self.dilate(coverage, spread)?;
		}
		if blur >= NEGLIGIBLE_LENGTH {
			coverage = blur_raster(&coverage, blur, false, BlurSpace::Premultiplied, self.pipeline)?;
		}

		Ok(coverage)
	}

	/// Grows the shapes in `coverage` by `distance` pixels along each axis, in a square like SVG's `feMorphology`.
	fn dilate(&self, coverage: Raster<GPU>, distance: f64) -> Result<Raster<GPU>, NodeError> {
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

		let mut coverage = coverage;
		for step in steps {
			for axis in [DVec2::X, DVec2::Y] {
				let placed = |offset: DVec2| RasterKernelInput {
					raster: &coverage,
					output_to_texel: DAffine2::from_translation(offset),
				};
				let inputs = vec![placed(DVec2::ZERO), placed(-axis * step), placed(axis * step)];
				coverage = run_raster_kernel(&Dilate {}, inputs, self.size, self.pipeline)?;
			}
		}

		Ok(coverage)
	}

	/// Stacks a silhouette's coverage, tinted `color`, over what lies beneath the content so far.
	fn accumulate(&mut self, coverage: &Raster<GPU>, color: Color) -> Result<(), NodeError> {
		let color = premultiplied_gamma_channels(color);
		self.beneath = Some(match &self.beneath {
			Some(beneath) => self.run(&Accumulate { color }, &[beneath, coverage])?,
			None => self.run(&Tint { color }, &[coverage])?,
		});
		Ok(())
	}

	/// Places a silhouette's coverage, tinted `color`, over both the full-strength content and the faded fill, within the content.
	fn shade_within(&mut self, coverage: &Raster<GPU>, color: Color) -> Result<(), NodeError> {
		let kernel = CompositeWithin {
			color: premultiplied_gamma_channels(color),
		};
		if let Some(filled) = &self.filled {
			self.filled = Some(self.run(&kernel, &[filled, coverage, &self.full])?);
		}
		self.full = self.run(&kernel, &[&self.full, coverage, &self.full])?;
		Ok(())
	}

	/// The image with the shadows and glows `beneath` placed under it, knocked out by the full-strength content.
	fn composite_beneath(&self, image: &Raster<GPU>, beneath: &Raster<GPU>, full_content: &Raster<GPU>) -> Result<Raster<GPU>, NodeError> {
		self.run(&CompositeBeneath {}, &[image, beneath, full_content])
	}

	/// The coverage of the band reaching `width` pixels from the content's silhouette edge, outward or (if `inward`) inward, measured in straight lines so it rounds corners.
	fn edge_band(&self, width: f64, inward: bool) -> Result<Raster<GPU>, NodeError> {
		let nearest = self.nearest_across_edge(width, inward)?;
		self.run(&EdgeBand { width: width as f32 }, &[&nearest])
	}

	/// Each texel's offset to the nearest texel across the content's silhouette edge: outside it if `inward`, or inside it otherwise, in the layout [`JumpFloodStep`] keeps.
	/// Jump flooding finds them in a pass for each halving of a step that starts past `reach`, so farther texels may find none.
	fn nearest_across_edge(&self, reach: f64, inward: bool) -> Result<Raster<GPU>, NodeError> {
		let mut nearest = self.run(&JumpFloodSeed { inward }, &[&self.full])?;

		let mut step = (reach.max(0.).ceil() as u32 + 1).next_power_of_two() as i32;
		while step >= 1 {
			nearest = self.run(&JumpFloodStep { step }, &[&nearest])?;
			step /= 2;
		}

		Ok(nearest)
	}
}
