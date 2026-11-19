//! Carries out a raster's effects on the CPU at the raster's own resolution, for the nodes that read its pixels.
//! Each step mirrors the renderer's GPU passes, working on premultiplied gamma-encoded channels as they do.

use crate::adjustments::adjust_colors;
use crate::blending::blend_gamma;
use crate::effect_kernels::{BlurKernel, NEGLIGIBLE_LENGTH, Plane};
use crate::generators::pattern_sampler;
use crate::layer_blending::blend_layer;
use crate::masks::{feather_strength, mask_strength, masked, masked_radius_blur, masked_radius_blur_codes};
use crate::value_effects::{adjustments, evaluate_gradient_effects, filled_color};
use core_types::FallibleVec2Operations;
use core_types::blending::BlendMode;
use core_types::color::{linear_to_srgb, srgb_to_linear};
use core_types::list::{ATTR_EFFECTS, ATTR_GRADIENT_FORM, Item};
use core_types::transfer_curve::TransferCurve;
use core_types::{ATTR_BLEND_MODE, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_TRANSFORM, Color};
use glam::{DAffine2, DMat2, DVec2};
use graphic_types::Graphic;
use graphic_types::adjustment::Adjustment;
use graphic_types::effect_layers::{EffectLayers, LayerPart, PiecePlace};
use graphic_types::effects::{
	BevelDirection, BevelStyle, BevelTechnique, EffectCapture, EffectOperation, Effects, GlowFill, GlowSource, GlowTechnique, OutlineFill, bevel_light, continued_map_pixels, contour_gradient_span,
	effect_space, fitted_paint, grow_and_tent, outline_extents, precise_falloff, shadow_offset, shape_strength, spans_along_axes,
};
use graphic_types::generator::GeneratorPattern;
use graphic_types::masks::masked_radius_blur_levels;
use raster_types::parallel_rows::fill_rows;
use raster_types::{CPU, Image, Raster};
use std::collections::HashMap;
use vector_types::gradient::{Gradient, GradientForm, GradientSettings};

mod styles;

/// The most pixels an image may grow to with the margin its effects cast into, beyond which the margin is cut short.
const MAX_PIXELS_WITH_MARGIN: f64 = (1 << 24) as f64;

/// The raster with its effects carried out into its pixels, grown to hold what they cast past its edges, and with the effects and the fill opacity they used removed.
/// A raster without the effects attribute is returned as it is.
pub fn evaluate_raster_effects(item: Item<Raster<CPU>>) -> Item<Raster<CPU>> {
	let Some(effects) = item.attribute::<Effects>(ATTR_EFFECTS).cloned() else { return item };
	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let fill = item.attribute_cloned_or::<f64>(ATTR_OPACITY_FILL, 1.) as f32;

	let (raster, attributes) = item.into_parts();
	let (raster, transform) = match evaluate(&raster, transform, &effects, fill) {
		Some((image, grown_transform)) => (Raster::new_cpu(image), grown_transform),
		None => (raster, transform),
	};

	// Which attributes come out depends only on whether the effects attribute came in, not on what it holds
	let mut item = Item::from_parts(raster, attributes);
	item.remove_attribute::<Effects>(ATTR_EFFECTS);
	item.remove_attribute::<f64>(ATTR_OPACITY_FILL);
	item.set_attribute(ATTR_TRANSFORM, transform);
	item
}

/// The raster with `adjustments` carried out on its colors, even those of transparent pixels, as for the adjustments leading its effects.
/// The result is kept with the raster's pixels, so adjusting them the same way again reuses it, ID and all.
pub fn adjusted_raster(raster: &Raster<CPU>, adjustments: &[Adjustment]) -> Raster<CPU> {
	let mut hasher = std::hash::DefaultHasher::new();
	adjustments.iter().for_each(|adjustment| core_types::graphene_hash::CacheHash::cache_hash(adjustment, &mut hasher));
	let key = std::hash::Hasher::finish(&hasher);

	raster.derived(key, |raster| {
		let mut image = raster.data().clone();
		adjustments.iter().for_each(|adjustment| adjust_colors(adjustment, &mut image.data));
		Raster::new_cpu(image)
	})
}

/// The pixels a masked radius blur's image map continues past its edges with, and the opacity fading them: the image with its own adjustments carried out, since they act on each pixel alone.
/// `None` for a map with other effects, or with adjustments changing alpha that its fill would fade first, which can't continue that way.
pub fn continued_map_raster(item: &Item<Raster<CPU>>) -> Option<(Raster<CPU>, f32)> {
	let fill = item.attribute_cloned_or::<f64>(ATTR_OPACITY_FILL, 1.);
	let opacity = (item.attribute_cloned_or::<f64>(ATTR_OPACITY, 1.) * fill) as f32;
	let adjustments = item.attribute::<Effects>(ATTR_EFFECTS).map_or(Some(Vec::new()), |effects| {
		(effects.operations().filter(|operation| !operation.changes_nothing()))
			.map(|operation| match operation {
				EffectOperation::Adjustment(adjustment) if !matches!(adjustment, Adjustment::ColorOverlay { .. }) => Some(adjustment),
				_ => None,
			})
			.collect::<Option<Vec<_>>>()
	})?;

	// A fill fades the image before its adjustments, so with one, only those keeping alpha commute with it
	if fill < 1. && !adjustments.iter().all(crate::adjustments::preserves_alpha) {
		return None;
	}

	let raster = if adjustments.is_empty() {
		item.element().clone()
	} else {
		adjusted_raster(item.element(), &adjustments)
	};
	Some((raster, opacity))
}

/// The image of the `effects` carried out on `raster`, which `transform` places in the document, with the transform placing that larger image.
/// Their layers are drawn over nothing, blending with each other by their modes. `None` when there's nothing to carry out, or the image is empty or its transform can't be inverted.
fn evaluate(raster: &Raster<CPU>, transform: DAffine2, effects: &Effects, fill: f32) -> Option<(Image<Color>, DAffine2)> {
	let (width, height) = (raster.width as usize, raster.height as usize);
	if (effects.change_nothing() && fill >= 1.) || width == 0 || height == 0 || transform.matrix2.determinant() == 0. {
		return None;
	}
	let size = DVec2::new(width as f64, height as f64);

	// Each effect's lengths become pixels along each of the image's axes, and its offsets map from its effect space through the inverse of the image's pixel placement
	let document_to_image_pixels = (transform.matrix2 * DMat2::from_diagonal(size.recip())).inverse();
	let rows: Vec<(EffectOperation, DMat2, bool)> = (effects.runs().into_iter().enumerate())
		.flat_map(|(run_index, (stamped_transform, rows))| {
			let effect_to_pixels = document_to_image_pixels * effect_space(transform, stamped_transform).matrix2;
			(effects.operations_in(rows).enumerate()).map(move |(index, operation)| (operation, effect_to_pixels, run_index > 0 && index == 0))
		})
		.collect();

	// Unless a fill fades them first, leading adjustments flattened into the image change its own colors, keeping even transparent pixels' colors
	let layers = EffectLayers::plan_runs(effects, BlendMode::Normal, fill, true);
	let leading_adjustments = rows.iter().take(layers.flattened).take_while(|_| fill == 1.).map_while(|(operation, ..)| match operation {
		EffectOperation::Adjustment(adjustment) => Some(adjustment),
		_ => None,
	});
	let mut colors = std::borrow::Cow::Borrowed(raster.data.as_slice());
	let mut leading = 0;
	for adjustment in leading_adjustments {
		adjust_colors(adjustment, colors.to_mut());
		leading += 1;
	}
	if leading == rows.len() {
		let data = colors.iter().map(|color| color.with_alpha(color.a() * fill)).collect();
		return Some((
			Image {
				width: raster.width,
				height: raster.height,
				data,
			},
			transform,
		));
	}

	// A margin around the image to hold the blur and shadows cast past its edges, cut short evenly for an image that would grow too large
	let reach_in_pixels: DVec2 = (rows[leading..].iter())
		.map(|(operation, effect_to_pixels, _)| operation.painted_reach() * spans_along_axes(*effect_to_pixels))
		.sum();
	let mut margin = reach_in_pixels.ceil();
	// A feather needs the pixels past the image's edges, which are absent, to find those edges
	if rows.iter().any(|(operation, ..)| matches!(operation, EffectOperation::Feather { distance } if *distance > 0.)) {
		margin = margin.max(DVec2::ONE);
	}
	if (size + 2. * margin).element_product() > MAX_PIXELS_WITH_MARGIN {
		let reach_in_pixels = margin.max_element();
		margin = (margin * margin_fraction_that_fits(size, margin)).floor();
		if margin.max_element() < reach_in_pixels {
			log::warn!(
				"The effects on a {width} by {height} image end at a hard edge {} pixels past it, short of the {reach_in_pixels} they reach, since the image can't grow past {MAX_PIXELS_WITH_MARGIN} pixels",
				margin.max_element()
			);
		}
	}
	let (margin_x, margin_y) = (margin.x as usize, margin.y as usize);

	// The content at full strength, which casts every silhouette, and the content faded by the fill, which shows in the result
	let mut full = Plane::<[f32; 4]>::new(width + 2 * margin_x, height + 2 * margin_y);
	for (y, row) in colors.chunks_exact(width).enumerate() {
		let start = (y + margin_y) * full.width + margin_x;
		full.data[start..start + width]
			.iter_mut()
			.zip(row)
			.for_each(|(target, &color)| *target = premultiplied_gamma_channels(color));
	}
	let mut filled = (fill < 1.).then(|| full.map(|pixel| pixel.map(|channel| channel * fill)));
	// The shadows and glows stacked beneath the content
	let mut beneath: Option<Plane<[f32; 4]>> = None;
	// The layered effects' pieces, each tinted
	let mut pieces: HashMap<EffectCapture, Plane<[f32; 4]>> = HashMap::new();
	// How much the trailing masks together show of each finished layer
	let mut layer_fade: Option<Plane<f32>> = None;

	// Each run after the first takes the result of those before it as its content at full strength, like the renderers' nested runs
	let mut run_fill = fill;
	for (index, &(ref operation, effect_to_pixels, starts_later_run)) in rows.iter().enumerate().skip(leading) {
		if starts_later_run {
			full = match (filled.take(), beneath.take()) {
				(filled, Some(beneath)) => composite_beneath(filled.as_ref().unwrap_or(&full), &beneath),
				(Some(filled), None) => filled,
				(None, None) => full,
			};
			run_fill = 1.;
		}
		if operation.changes_nothing() {
			continue;
		}
		// TODO: Unlike the GPU and SVG, this ignores skew, and the distance-based effects (spread, choke, Precise, Outline, Bevel & Emboss, Feather) average the axes' scales.
		// Delete this baking once its callers read back the GPU's renders, which are exact.
		let pixels_per_length = spans_along_axes(effect_to_pixels);

		// A layered effect's coverages become pieces, which the content knocks out once its layer is drawn, and a flattened one is knocked out by the faded content now
		let layered = index >= layers.flattened;
		let piece = |plane: usize| layered.then_some(EffectCapture { index, plane });
		let knockout_fill = if layered { 1. } else { run_fill };

		match *operation {
			EffectOperation::Blur { radius, box_blur, gamma, .. } if !operation.varies_blur_radius() => {
				let radius = radius * pixels_per_length;
				if radius.max_element() < NEGLIGIBLE_LENGTH {
					continue;
				}

				merge_beneath(&mut full, &mut filled, &mut beneath);
				let kernels = match box_blur {
					true => (BlurKernel::Box { radius: radius.x }, BlurKernel::Box { radius: radius.y }),
					false => (BlurKernel::Gaussian { sigma: radius.x }, BlurKernel::Gaussian { sigma: radius.y }),
				};
				full = blur(full, kernels, !gamma);
				filled = filled.map(|filled| blur(filled, kernels, !gamma));
			}
			// A blur varying its radius by its radius mask
			EffectOperation::Blur { radius, box_blur, gamma, .. } => {
				if (radius * pixels_per_length).max_element() < NEGLIGIBLE_LENGTH {
					continue;
				}
				let Some(map_colors) = drawn_paint_colors(operation, transform, size, effect_to_pixels, (margin_x, margin_y), full.width, full.height) else {
					continue;
				};
				merge_beneath(&mut full, &mut filled, &mut beneath);

				let codes = masked_radius_blur_codes(&map_colors);
				// The levels are chosen at the GPU's scale, so both renderers blur by the same ones
				let length_scale = effect_to_pixels.determinant().abs().sqrt();
				let levels: Vec<DVec2> = masked_radius_blur_levels(radius, length_scale).into_iter().map(|level| level * pixels_per_length).collect();
				full = masked_radius_blur(&full, &codes, &levels, box_blur, !gamma);
				filled = filled.map(|filled| masked_radius_blur(&filled, &codes, &levels, box_blur, !gamma));
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
				let offset = effect_to_pixels * shadow_offset(angle, distance);
				let silhouette = full.map(|[_, _, _, alpha]| alpha).shifted(offset.x, offset.y);
				let strength = cast(silhouette, size, spread, GlowTechnique::Softer, 100., contour, pixels_per_length);
				let shadow = tinted(&strength, color);
				let shadow = if knockout { knocked_out(shadow, &full, knockout_fill) } else { shadow };
				place_beneath(&mut pieces, piece(0), &mut beneath, shadow);
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
				let raw = cast_strength(full.map(|[_, _, _, alpha]| alpha), size, spread, technique, pixels_per_length);
				let strength = shaped(raw.clone(), range, contour);
				let glow = knocked_out(glow_tint(&paint, &strength, &raw, true), &full, knockout_fill);
				place_beneath(&mut pieces, piece(0), &mut beneath, glow);
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
				// Cast from the area outside the content, which the margin and anything moved in from past it belong to
				let offset = effect_to_pixels * shadow_offset(angle, distance);
				let outside = full.map(|[_, _, _, alpha]| alpha).shifted(offset.x, offset.y).map(|alpha| 1. - alpha);
				let strength = cast(outside, size, choke, GlowTechnique::Softer, 100., contour, pixels_per_length);
				place_within(&mut pieces, piece(0), &mut full, &mut filled, tinted(&strength, color), blend_mode);
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
				let outside = full.map(|[_, _, _, alpha]| 1. - alpha);
				let mut strength = cast(outside, size, choke, technique, range, contour, pixels_per_length);
				if source == GlowSource::Center {
					strength = strength.map(|strength| 1. - strength);
				}

				// An inner glow doesn't fade, since the content bounds it
				let glow = match paint {
					GlowFill::Solid(color) => tinted(&strength, color),
					GlowFill::Gradient { .. } => glow_tint(&paint, &strength, &strength, false),
				};
				place_within(&mut pieces, piece(0), &mut full, &mut filled, glow, blend_mode);
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
				let offset = effect_to_pixels * shadow_offset(angle, distance);
				let silhouette = full.map(|[_, _, _, alpha]| alpha);
				let cast_toward = |offset: DVec2| {
					cast(
						silhouette.clone().shifted(offset.x, offset.y),
						size,
						0.,
						GlowTechnique::Softer,
						100.,
						&TransferCurve::default(),
						pixels_per_length,
					)
				};
				let difference = cast_toward(offset).zip(&cast_toward(-offset), |forward, backward| (forward - backward).abs());

				let strength = shaped(if invert { difference.map(|difference| 1. - difference) } else { difference }, 100., contour);
				place_within(&mut pieces, piece(0), &mut full, &mut filled, tinted(&strength, color), blend_mode);
			}
			EffectOperation::Outline {
				ref paint,
				blend_mode,
				weight,
				align,
				contour_gradient,
			} => {
				let Some(outline_fill) = OutlineFill::of(paint, contour_gradient) else { continue };

				// Straight-line distances can't stretch along one axis, so an image scaled unevenly takes the mean of its two scales
				let mean_scale = (pixels_per_length.x * pixels_per_length.y).sqrt();
				let (outside, inside) = outline_extents(weight * mean_scale, align);
				let contour_span = matches!(outline_fill, OutlineFill::ContourGradient(_))
					.then(|| contour_gradient_span(weight, align))
					.map(|(start, span)| (start * mean_scale, span * mean_scale));
				let bands = styles::outline_bands(&full.map(|[_, _, _, alpha]| alpha), outside, inside, contour_span);

				// The premultiplied colors the bands take before their coverage, unless a solid color tints them
				let colors = match &outline_fill {
					OutlineFill::Solid(_) => None,
					OutlineFill::Drawn(_) => {
						// TODO: A paint the CPU can't draw leaves the outline out (see `drawn_paint_colors`)
						let Some(colors) = drawn_paint_colors(operation, transform, size, effect_to_pixels, (margin_x, margin_y), full.width, full.height) else {
							continue;
						};
						Some(Plane {
							width: full.width,
							height: full.height,
							data: colors.into_iter().map(premultiplied_gamma_channels).collect(),
						})
					}
					OutlineFill::ContourGradient(gradient) => {
						let Some(positions) = &bands.positions else { continue };
						Some(styles::contour_gradient_colors(gradient, positions))
					}
				};
				let painted = |coverage: &Plane<f32>| match (&outline_fill, &colors) {
					(OutlineFill::Solid(color), _) => tinted(coverage, *color),
					(_, Some(colors)) => coverage.zip(colors, |coverage, color| color.map(|channel| channel * coverage)),
					(_, None) => coverage.map(|_| [0.; 4]),
				};

				// The band outside lies beneath the content like a shadow, and the band inside over it like an inner shadow
				if let Some(band) = &bands.outside {
					let band = knocked_out(painted(band), &full, knockout_fill);
					place_beneath(&mut pieces, piece(0), &mut beneath, band);
				}
				if let Some(band) = &bands.inside {
					place_within(&mut pieces, piece(1), &mut full, &mut filled, painted(band), blend_mode);
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
				// Straight-line distances can't stretch along one axis, so an image scaled unevenly takes the mean of its two scales
				let mean_scale = (pixels_per_length.x * pixels_per_length.y).sqrt();
				let bevel_size = size * mean_scale;
				if bevel_size < NEGLIGIBLE_LENGTH && gloss_contour.is_identity() {
					continue;
				}
				let image_size = DVec2::new(width as f64, height as f64);
				// TODO: A texture the CPU can't draw leaves the surface untextured (see `drawn_paint_colors`)
				let texture = drawn_paint_colors(operation, transform, image_size, effect_to_pixels, (margin_x, margin_y), full.width, full.height)
					.map(|colors| styles::texture_grays(colors, full.width, full.height));

				// The surface's height, reshaped by the contour and sunk by the texture, lit from the light's direction into its highlights and shadows
				let (height, rise) = bevel_height(&full.map(|[_, _, _, alpha]| alpha), style, technique, bevel_size);
				let rise = rise * depth / 100. * if direction == BevelDirection::Down { -1. } else { 1. };
				let surface = styles::BevelSurface {
					style,
					slope_contour,
					slope_range,
					slope_antialiased,
					texture,
					texture_depth,
				};
				let gloss = (!gloss_contour.is_identity()).then(|| (gloss_contour.evaluator(), gloss_antialiased));
				let light = bevel_light(angle, altitude, effect_to_pixels);
				let bevel_shading = |rise: f64| styles::bevel_shading(&height, &surface, rise, light, gloss.as_ref());
				let lit = bevel_shading(rise);

				// Under a gloss contour or texture, the shading beneath the content fades out across the slopes' foot
				let fades_at_foot = gloss.is_some() || surface.texture.is_some();
				let beneath_shading = |amounts: [Plane<f32>; 2]| if fades_at_foot { styles::foot_faded(amounts, &height) } else { amounts };
				let (_, soften) = grow_and_tent(soften * mean_scale, 0.);
				let softened = |amounts: [Plane<f32>; 2]| {
					amounts.map(|amount| match soften >= NEGLIGIBLE_LENGTH {
						true => amount.blurred(BlurKernel::Tent { radius: soften }, BlurKernel::Tent { radius: soften }),
						false => amount,
					})
				};

				// The slopes inside the content are lit over it, and those outside lie beneath it
				let inside = (style != BevelStyle::OuterBevel).then(|| softened(lit.clone()));
				let outside = match style {
					BevelStyle::InnerBevel => None,
					BevelStyle::OuterBevel | BevelStyle::Emboss => Some(match (&inside, fades_at_foot) {
						(Some(inside), false) => inside.clone(),
						_ => softened(beneath_shading(lit)),
					}),
					// A pillow's slopes outside are lit mirrored, so they rise away from the edge
					BevelStyle::PillowEmboss => Some(softened(beneath_shading(bevel_shading(-rise)))),
				};
				let colors = [(highlight_color, highlight_blend_mode), (shadow_color, shadow_blend_mode)];
				for (plane, (amount, (color, blend_mode))) in inside.into_iter().flatten().zip(colors).enumerate() {
					place_within(&mut pieces, piece(plane), &mut full, &mut filled, tinted(&amount, color), blend_mode);
				}
				for (plane, (amount, (color, _))) in outside.into_iter().flatten().zip(colors).enumerate() {
					let slope = knocked_out(tinted(&amount, color), &full, knockout_fill);
					place_beneath(&mut pieces, piece(plane + 2), &mut beneath, slope);
				}
			}
			EffectOperation::PaintOverlay { blend_mode, opacity, .. } => {
				// TODO: A paint the CPU can't draw leaves the overlay out (see `drawn_paint_colors`)
				let Some(paint_colors) = drawn_paint_colors(operation, transform, size, effect_to_pixels, (margin_x, margin_y), full.width, full.height) else {
					continue;
				};

				let opacity = (opacity / 100.) as f32;
				let data = paint_colors.iter().map(|&paint| premultiplied_gamma_channels(paint).map(|channel| channel * opacity)).collect();
				let painted = Plane {
					width: full.width,
					height: full.height,
					data,
				};
				place_within(&mut pieces, piece(0), &mut full, &mut filled, painted, blend_mode);
			}
			// A solid paint overlay paints within the content alone, like any other paint overlay
			EffectOperation::Adjustment(Adjustment::ColorOverlay { color, blend_mode, opacity }) => {
				let painted = premultiplied_gamma_channels(color.with_alpha(color.a() * (opacity / 100.) as f32));
				let painted = full.map(|_| painted);
				place_within(&mut pieces, piece(0), &mut full, &mut filled, painted, blend_mode);
			}
			EffectOperation::Mask { mode, density, feather, invert, .. } => {
				// The mask graphic is read as far past the image as its feather reaches, which paints nothing there, cut short like the image's own margin
				let read_margin = (operation.reach() * pixels_per_length).ceil();
				let read_margin = (read_margin * margin_fraction_that_fits(DVec2::new(full.width as f64, full.height as f64), read_margin)).floor();
				let (read_x, read_y) = (read_margin.x as usize, read_margin.y as usize);
				let (read_width, read_height) = (full.width + 2 * read_x, full.height + 2 * read_y);

				// TODO: A mask graphic the CPU can't draw, like a vector shape, leaves the content unmasked (see `drawn_paint_colors`)
				let Some(mask_colors) = drawn_paint_colors(operation, transform, size, effect_to_pixels, (margin_x + read_x, margin_y + read_y), read_width, read_height) else {
					continue;
				};
				let strength = mask_strength(&mask_colors, read_width, read_height, mode, density, feather, invert, pixels_per_length);
				let strength = Plane {
					width: full.width,
					height: full.height,
					data: (0..full.height)
						.flat_map(|y| strength.data[(y + read_y) * read_width + read_x..][..full.width].iter().copied())
						.collect(),
				};

				if layers.trailing_masks.contains(&index) {
					layer_fade = Some(match layer_fade.take() {
						Some(fade) => fade.zip(&strength, |fade, strength| fade * strength),
						None => strength,
					});
					continue;
				}

				merge_beneath(&mut full, &mut filled, &mut beneath);
				full = masked(&full, &strength);
				filled = filled.map(|filled| masked(&filled, &strength));
			}
			EffectOperation::Feather { distance } => {
				merge_beneath(&mut full, &mut filled, &mut beneath);

				// Distances run in the image's pixels, so an image scaled unevenly takes the mean of its two scales, as an outline does
				let mean_scale = (pixels_per_length.x * pixels_per_length.y).sqrt();
				let strength = feather_strength(&full, distance * mean_scale);
				full = masked(&full, &strength);
				filled = filled.map(|filled| masked(&filled, &strength));
			}
			EffectOperation::Adjustment(ref adjustment) => {
				merge_beneath(&mut full, &mut filled, &mut beneath);
				full = adjust(&full, adjustment);
				filled = filled.map(|filled| adjust(&filled, adjustment));
			}
		}
	}

	let result = composite_layers(&layers, filled.as_ref().unwrap_or(&full), beneath.as_ref(), &full, &pieces, layer_fade.as_ref());
	let image = Image {
		width: result.width as u32,
		height: result.height as u32,
		data: result.data.into_iter().map(straight_linear_color).collect(),
	};

	// The grown image spans the margin on each side of the original's unit square
	let unit_margin = DVec2::new(margin_x as f64, margin_y as f64) / size;
	let grown_transform = transform * DAffine2::from_translation(-unit_margin) * DAffine2::from_scale(DVec2::ONE + 2. * unit_margin);

	Some((image, grown_transform))
}

/// The fraction of `margin` that grows an image of `size` to [`MAX_PIXELS_WITH_MARGIN`], or 0 if the image alone already reaches it.
fn margin_fraction_that_fits(size: DVec2, margin: DVec2) -> f64 {
	// The positive root of (width + 2 × fraction × margin width) × (height + 2 × fraction × margin height) = the limit
	let quadratic = 4. * margin.x * margin.y;
	let linear = 2. * (size.x * margin.y + size.y * margin.x);
	let constant = size.element_product() - MAX_PIXELS_WITH_MARGIN;
	if constant >= 0. || linear <= 0. {
		return 0.;
	}

	let fraction = match quadratic > 0. {
		true => (-linear + (linear * linear - 4. * quadratic * constant).sqrt()) / (2. * quadratic),
		false => -constant / linear,
	};
	fraction.clamp(0., 1.)
}

/// A color as the premultiplied, gamma-encoded channels the effects work with.
pub(crate) fn premultiplied_gamma_channels(color: Color) -> [f32; 4] {
	let [red, green, blue, alpha] = color.to_gamma_srgb_channels();
	[red * alpha, green * alpha, blue * alpha, alpha]
}

/// Premultiplied, gamma-encoded channels back as a color.
fn straight_linear_color([red, green, blue, alpha]: [f32; 4]) -> Color {
	if alpha <= 0. {
		return Color::TRANSPARENT;
	}
	Color::from_gamma_srgb_channels(red / alpha, green / alpha, blue / alpha, alpha.min(1.))
}

/// Adjusts premultiplied gamma-encoded pixels by way of their straight colors, as every renderer carries out an adjustment among an item's effects.
/// Fully transparent pixels hold no color to adjust, so they stay empty even where the adjustment would raise their alpha.
pub fn adjust_premultiplied(pixels: &mut [[f32; 4]], adjustment: &Adjustment) {
	let mut colors: Vec<Color> = pixels.iter().copied().map(straight_linear_color).collect();
	adjust_colors(adjustment, &mut colors);

	for (pixel, color) in pixels.iter_mut().zip(colors) {
		*pixel = if pixel[3] > 0. { premultiplied_gamma_channels(color) } else { [0.; 4] };
	}
}

fn adjust(plane: &Plane<[f32; 4]>, adjustment: &Adjustment) -> Plane<[f32; 4]> {
	let mut data = plane.data.clone();
	adjust_premultiplied(&mut data, adjustment);

	Plane {
		width: plane.width,
		height: plane.height,
		data,
	}
}

/// Premultiplied channels with `transfer` applied to their straight color.
fn transferred(transfer: fn(f32) -> f32) -> impl Fn([f32; 4]) -> [f32; 4] + Sync {
	move |[red, green, blue, alpha]| {
		if alpha <= 0. {
			return [0.; 4];
		}
		let channel = |value: f32| transfer((value / alpha).max(0.)) * alpha;
		[channel(red), channel(green), channel(blue), alpha]
	}
}

/// Blurs premultiplied gamma-encoded pixels by a kernel along each axis, in linear light if `linear`.
pub(crate) fn blur(plane: Plane<[f32; 4]>, (horizontal, vertical): (BlurKernel, BlurKernel), linear: bool) -> Plane<[f32; 4]> {
	match linear {
		true => plane.map(transferred(srgb_to_linear)).blurred(horizontal, vertical).map(transferred(linear_to_srgb)),
		false => plane.blurred(horizontal, vertical),
	}
}

/// The strength from 0 to 1 of a shadow or glow cast from a `silhouette`'s coverage, shaped for PSD interop: grown by the `spread` percentage
/// of its `size` (in document pixels), softened over the rest by the `technique`, then strengthened by its `range` percentage and reshaped by its `contour`.
fn cast(silhouette: Plane<f32>, size: f64, spread: f64, technique: GlowTechnique, range: f64, contour: &TransferCurve, pixels_per_length: DVec2) -> Plane<f32> {
	shaped(cast_strength(silhouette, size, spread, technique, pixels_per_length), range, contour)
}

/// The strength from 0 to 1 of a shadow or glow cast from a `silhouette`'s coverage before its range and contour shape it, as [`cast`] describes.
fn cast_strength(silhouette: Plane<f32>, size: f64, spread: f64, technique: GlowTechnique, pixels_per_length: DVec2) -> Plane<f32> {
	// Straight-line distances can't stretch along one axis, so an image scaled unevenly measures them by the mean of its two scales
	let mean_scale = (pixels_per_length.x * pixels_per_length.y).sqrt();
	let (grow, tent) = grow_and_tent(size, spread);

	match technique {
		GlowTechnique::Softer => {
			let grown = match grow * mean_scale >= NEGLIGIBLE_LENGTH {
				true => silhouette.grown(grow * mean_scale),
				false => silhouette,
			};
			let radius = tent * pixels_per_length;
			match radius.max_element() >= NEGLIGIBLE_LENGTH {
				true => grown.blurred(BlurKernel::Tent { radius: radius.x }, BlurKernel::Tent { radius: radius.y }),
				false => grown,
			}
		}
		GlowTechnique::Precise => {
			let grow_pixels = (grow * mean_scale) as f32;
			let to_content = silhouette.distances_to_silhouette(((size + 1.) * mean_scale).max(grow * mean_scale + 3.));

			// Within the grown shape, the falloff measures how far it lies in from the grown shape's edge, which at the content's inner corners is farther than the grow.
			// Each pixel up to 3 past the grown edge, a band any path in from farther crosses, seeds that measure with how far short of the edge it lies, so the edge falls between pixels.
			let distances = match grow_pixels >= NEGLIGIBLE_LENGTH as f32 {
				true => {
					let past_grown = to_content.map(|distance| match distance > grow_pixels && distance <= grow_pixels + 3. {
						true => grow_pixels - distance,
						false => f32::INFINITY,
					});
					let within_grown = past_grown.distances_through_seeds(grow * mean_scale + 1.);
					to_content.zip(&within_grown, |to_content, within_grown| match to_content > grow_pixels {
						true => to_content,
						false => grow_pixels - within_grown,
					})
				}
				false => to_content,
			};
			distances.map(|distance| precise_falloff(distance as f64 / mean_scale, grow, size) as f32)
		}
	}
}

/// The `strength` from 0 to 1 at each pixel, strengthened by the `range` percentage and reshaped by the `contour`.
fn shaped(strength: Plane<f32>, range: f64, contour: &TransferCurve) -> Plane<f32> {
	if range >= 100. && contour.is_identity() {
		return strength;
	}
	let contour = contour.evaluator();
	strength.map(|strength| shape_strength(strength as f64, range, &|value| contour.evaluate(value)) as f32)
}

/// The premultiplied color a glow filled with `paint` paints at each pixel, from its `strength` after range and contour and its `raw` strength before them.
/// An outer glow `fades` out with its raw strength when filled with a gradient.
fn glow_tint(paint: &GlowFill, strength: &Plane<f32>, raw: &Plane<f32>, fades: bool) -> Plane<[f32; 4]> {
	strength
		.zip(raw, |strength, raw| [strength, raw, 0., 0.])
		.map(|[strength, raw, ..]| premultiplied_gamma_channels(paint.color_at(strength as f64, raw as f64, fades)))
}

/// A silhouette's coverage tinted `color`.
fn tinted(coverage: &Plane<f32>, color: Color) -> Plane<[f32; 4]> {
	let color = premultiplied_gamma_channels(color);
	coverage.map(|coverage| color.map(|channel| channel * coverage))
}

/// A tinted image of what lies beneath, hidden behind the full-strength content where the content covers it as strongly as `fill`.
fn knocked_out(tinted: Plane<[f32; 4]>, full_content: &Plane<[f32; 4]>, fill: f32) -> Plane<[f32; 4]> {
	if fill >= 1. {
		return tinted;
	}

	// The faded content shows what's beneath through 1 - fill × alpha of it, of which only 1 - alpha may show this
	tinted.zip(full_content, |tinted, content| {
		let shown = (1. - content[3]) / (1. - fill * content[3]).max(f32::EPSILON);
		tinted.map(|channel| channel * shown)
	})
}

/// Places a tinted image beneath the content: as the layered `piece` if it names one, or else stacked over what lies `beneath` the content so far.
fn place_beneath(pieces: &mut HashMap<EffectCapture, Plane<[f32; 4]>>, piece: Option<EffectCapture>, beneath: &mut Option<Plane<[f32; 4]>>, tinted: Plane<[f32; 4]>) {
	match piece {
		Some(piece) => {
			pieces.insert(piece, tinted);
		}
		None => *beneath = Some(stack_over(beneath.take(), tinted)),
	}
}

/// Places a premultiplied `tinted` image within the content: as the layered `piece` if it names one, or else over the content's images, mixed with its colors by the blend mode.
fn place_within(
	pieces: &mut HashMap<EffectCapture, Plane<[f32; 4]>>,
	piece: Option<EffectCapture>,
	full: &mut Plane<[f32; 4]>,
	filled: &mut Option<Plane<[f32; 4]>>,
	tinted: Plane<[f32; 4]>,
	blend_mode: BlendMode,
) {
	match piece {
		Some(piece) => {
			pieces.insert(piece, tinted);
		}
		None => {
			*filled = filled.take().map(|filled| composite_in_share(&filled, &tinted, full, blend_mode, SharePart::Whole));
			*full = composite_in_share(full, &tinted, full, blend_mode, SharePart::Whole);
		}
	}
}

/// The `upper` image stacked over the `stack`, if there is one.
fn stack_over(stack: Option<Plane<[f32; 4]>>, upper: Plane<[f32; 4]>) -> Plane<[f32; 4]> {
	match stack {
		Some(stack) => composite_beneath(&upper, &stack),
		None => upper,
	}
}

/// The `layers` drawn over nothing, from the content's image with its flattened effects, the shadows and glows flattened `beneath` it, and the layered `pieces`.
/// The content's layer is finished first, since the knockouts and the parts showing through its fill read its alpha.
/// Each layer then stacks its parts, fades by the trailing masks' `fade` if there is one, and blends by its mode onto the layers before it.
fn composite_layers(
	layers: &EffectLayers,
	content_image: &Plane<[f32; 4]>,
	beneath: Option<&Plane<[f32; 4]>>,
	full_content: &Plane<[f32; 4]>,
	pieces: &HashMap<EffectCapture, Plane<[f32; 4]>>,
	fade: Option<&Plane<f32>>,
) -> Plane<[f32; 4]> {
	// Drawn over nothing, the item's opacity applies afterward to the whole image
	const OPACITY: f32 = 1.;

	let content_layer = layers.content_layer();
	let mut content = content_image.clone();
	for piece in content_layer.into_iter().flat_map(|layer| &layers.layers[layer].parts).filter_map(|part| match part {
		LayerPart::Piece(piece) => Some(piece),
		LayerPart::Content => None,
	}) {
		let Some(tinted) = pieces.get(&piece.capture) else { continue };
		let part = match piece.place {
			PiecePlace::Within => SharePart::Whole,
			PiecePlace::IntoContent { over: false } => SharePart::IntoContent,
			PiecePlace::IntoContent { over: true } => SharePart::IntoContentBeneathPartOver { opacity: OPACITY, fade },
			PiecePlace::Beneath | PiecePlace::Through { .. } => continue,
		};
		content = composite_in_share(&content, tinted, full_content, piece.blend_mode, part);
	}

	let mut result: Option<Plane<[f32; 4]>> = None;
	for (index, layer) in layers.layers.iter().enumerate() {
		let holds_content = Some(index) == content_layer;
		let mut stack = beneath.filter(|_| holds_content).cloned();

		for part in &layer.parts {
			let LayerPart::Piece(piece) = part else { continue };
			let Some(tinted) = pieces.get(&piece.capture) else { continue };

			let upper = match piece.place {
				PiecePlace::Beneath if piece.knocks_out => {
					let cover_fade = fade.filter(|_| layers.masks_fade_knockout_cover(piece.capture));
					knocked_out_by_layer(tinted, full_content, &content, layers.knockout_cover(piece.capture, OPACITY), cover_fade)
				}
				PiecePlace::Beneath => tinted.clone(),
				PiecePlace::Within if !holds_content => tinted.zip(full_content, |tinted, content| tinted.map(|channel| channel * content[3])),
				PiecePlace::Through { .. } => tinted.zip(&through_shares(full_content, &content, OPACITY, fade), |tinted, share| tinted.map(|channel| channel * share)),
				PiecePlace::Within | PiecePlace::IntoContent { .. } => continue,
			};
			stack = Some(stack_over(stack, upper));
		}

		let image = if holds_content { Some(stack_over(stack.take(), content.clone())) } else { stack };
		let Some(image) = image else { continue };
		let image = match fade {
			Some(fade) => masked(&image, fade),
			None => image,
		};
		result = Some(match result {
			Some(below) => blend_layer(&image, &below, layer.blend_mode),
			None => image,
		});
	}

	result.unwrap_or(content)
}

/// How a part of an interior piece composites within the content's share of each pixel.
#[derive(Clone, Copy)]
enum SharePart<'a> {
	/// The whole piece, blending with the content's colors as far as they show and painting plainly where the fill lets nothing show.
	Whole,
	/// The part blending with the content's colors, which leaves the content's alpha as it is.
	IntoContent,
	/// That part for a piece whose part over the content takes the share through its fill, at the `opacity` the `fade` leaves, leaving this the rest.
	IntoContentBeneathPartOver { opacity: f32, fade: Option<&'a Plane<f32>> },
}

/// The image with a premultiplied `tinted` part of an interior piece composited within the full-strength content's share of each pixel, mixed with the image's own colors by the blend mode in gamma.
/// For PSD interop, interior pieces stack inside the share as layers over nothing would, and the share then scales the stack. O(width × height).
fn composite_in_share(image: &Plane<[f32; 4]>, tinted: &Plane<[f32; 4]>, full_content: &Plane<[f32; 4]>, blend_mode: BlendMode, part: SharePart) -> Plane<[f32; 4]> {
	let mut result = Plane::<[f32; 4]>::new(image.width, image.height);
	fill_rows(&mut result.data, image.width, |y, row| {
		for (x, composited) in row.iter_mut().enumerate() {
			let index = y * image.width + x;
			let (pixel, paint, share) = (image.data[index], tinted.data[index], full_content.data[index][3]);
			let (strength, alpha) = (paint[3], pixel[3]);
			if strength <= 0. {
				*composited = pixel;
				continue;
			}

			// The paint blended with the image's own straight color, weighted by how much of the share the image covers
			let color = [0, 1, 2].map(|channel| paint[channel] / strength);
			let blended = match alpha > 0. {
				true => blend_gamma([0, 1, 2].map(|channel| pixel[channel] / alpha), color, blend_mode).map(|channel| channel * alpha),
				false => [0.; 3],
			};

			let (strength, plain) = match part {
				SharePart::Whole => (strength, share - alpha),
				SharePart::IntoContent => (strength, 0.),
				SharePart::IntoContentBeneathPartOver { opacity, fade } => {
					let opacity = faded_opacity(opacity, fade, index);
					(strength * (1. - opacity * through_share(share, alpha, opacity)), 0.)
				}
			};
			let [red, green, blue] = std::array::from_fn(|channel| (1. - strength) * pixel[channel] + strength * (plain * color[channel] + blended[channel]));
			*composited = [red, green, blue, alpha + strength * plain];
		}
	});
	result
}

/// The share of a pixel within the content of full-strength alpha `share` that what lies beneath shows through, given the content's layer's alpha, drawn at `opacity`.
fn through_share(share: f32, layer_alpha: f32, opacity: f32) -> f32 {
	let shown = 1. - opacity * layer_alpha;
	if shown > f32::EPSILON { ((share - layer_alpha) / shown).clamp(0., 1.) } else { 0. }
}

/// The `opacity` that the trailing masks' `fade` leaves at the pixel at `index`.
fn faded_opacity(opacity: f32, fade: Option<&Plane<f32>>, index: usize) -> f32 {
	opacity * fade.map_or(1., |fade| fade.data[index])
}

/// The share [`through_share`] gives at each pixel of the content and its finished layer, by [`faded_opacity`].
fn through_shares(full_content: &Plane<[f32; 4]>, content_layer: &Plane<[f32; 4]>, opacity: f32, fade: Option<&Plane<f32>>) -> Plane<f32> {
	let mut shares = Plane::<f32>::new(full_content.width, full_content.height);
	fill_rows(&mut shares.data, full_content.width, |y, row| {
		for (x, share) in row.iter_mut().enumerate() {
			let index = y * full_content.width + x;
			*share = through_share(full_content.data[index][3], content_layer.data[index][3], faded_opacity(opacity, fade, index));
		}
	});
	shares
}

/// A tinted image of what lies beneath, hidden behind the full-strength content where its finished layer covers it, at the faded `cover` times its alpha.
fn knocked_out_by_layer(tinted: &Plane<[f32; 4]>, full_content: &Plane<[f32; 4]>, content_layer: &Plane<[f32; 4]>, cover: f32, fade: Option<&Plane<f32>>) -> Plane<[f32; 4]> {
	let mut knocked_out = Plane::<[f32; 4]>::new(tinted.width, tinted.height);
	fill_rows(&mut knocked_out.data, tinted.width, |y, row| {
		for (x, pixel) in row.iter_mut().enumerate() {
			let index = y * tinted.width + x;
			let covered = faded_opacity(cover, fade, index) * content_layer.data[index][3];
			let shown = ((1. - full_content.data[index][3]) / (1. - covered).max(f32::EPSILON)).min(1.);

			*pixel = tinted.data[index].map(|channel| channel * shown);
		}
	});
	knocked_out
}

/// Places what lies `beneath` under the full-strength content and the faded fill, so a pixel filter after it (a blur, an adjustment, a mask, or a feather) processes them together.
fn merge_beneath(full: &mut Plane<[f32; 4]>, filled: &mut Option<Plane<[f32; 4]>>, beneath: &mut Option<Plane<[f32; 4]>>) {
	let Some(beneath) = beneath.take() else { return };

	*filled = filled.take().map(|filled| composite_beneath(&filled, &beneath));
	*full = composite_beneath(full, &beneath);
}

/// The image with the shadows and glows `beneath` placed under it.
fn composite_beneath(image: &Plane<[f32; 4]>, beneath: &Plane<[f32; 4]>) -> Plane<[f32; 4]> {
	image.zip(beneath, |image, beneath| std::array::from_fn(|channel| image[channel] + beneath[channel] * (1. - image[3])))
}

/// A bevel's height over the `style`'s slopes `size` pixels wide, shaped by the `technique`, from -0.5 at their foot to 0.5 at their top, with how many pixels they rise.
/// It's centered on the edge, where the GPU's matching half floats are finest. Smooth slopes are a tent blur of the silhouette, and chiseled ones rise evenly with the distance from its edge.
fn bevel_height(silhouette: &Plane<f32>, style: BevelStyle, technique: BevelTechnique, size: f64) -> (Plane<f32>, f64) {
	// Embossed slopes span half the size on each side of the edge
	let span = match style {
		BevelStyle::InnerBevel | BevelStyle::OuterBevel => size,
		BevelStyle::Emboss | BevelStyle::PillowEmboss => size / 2.,
	};

	// Slopes too narrow to see leave a flat surface, of the coverage less a half, which only a gloss contour shades
	if span < NEGLIGIBLE_LENGTH {
		return (silhouette.map(|alpha| alpha - 0.5), 0.);
	}

	match technique {
		BevelTechnique::Smooth => {
			let (_, tent) = grow_and_tent(span, 0.);
			(silhouette.map(|alpha| alpha - 0.5).blurred(BlurKernel::Tent { radius: tent }, BlurKernel::Tent { radius: tent }), span)
		}
		BevelTechnique::ChiselHard | BevelTechnique::ChiselSoft => {
			// For PSD interop, the slope continues through the edge from 0 at the first pixel outside, and levels off a pixel past its span having risen half the span.
			// The height spans only the part across the edge the style lights.
			let near = if technique == BevelTechnique::ChiselSoft { 3. } else { 2. };
			let (low, high) = match style {
				BevelStyle::InnerBevel => (-near, span + 1.),
				BevelStyle::OuterBevel => (-span - 1., near),
				BevelStyle::Emboss | BevelStyle::PillowEmboss => (-span - 1., span + 1.),
			};

			let field = match technique {
				// For PSD interop, a soft chisel measures from a slope-limited matte, where a pixel's coverage away from half gives its distance across the edge a head start scaled to span the slope
				BevelTechnique::ChiselSoft => {
					let reach = (span + 1.) as f32;
					let matte = silhouette.map(|alpha| (alpha - 0.5) * 2. * reach);
					let inward = matte.map(|matte| matte.max(0.)).distances_through_seeds(high + 1.);
					let outward = matte.map(|matte| (-matte).max(0.)).distances_through_seeds(-low + 1.);
					inward.zip(&outward, |inward, outward| inward - outward)
				}
				_ => {
					let inward = silhouette.map(|alpha| 1. - alpha).distances_to_silhouette(high + 1.);
					let outward = silhouette.distances_to_silhouette(-low + 1.);
					inward.zip(&outward, |inward, outward| (inward - 0.5).max(0.) - (outward - 0.5).max(0.) + 0.5)
				}
			};

			// The slopes run from their foot a pixel past their span outside to their top a pixel past it inside
			let (low, high, reach) = (low as f32, high as f32, (span + 1.) as f32);
			let height = field.map(|field| field.clamp(low, high) / (2. * reach));
			(height, span)
		}
	}
}

/// The color of the operation's drawn paint at each pixel's center in an image `width` by `height`, which is the image of `size` pixels that `transform` places grown by `margin` on each side.
/// A paint without its own placement is fitted to the bounds [`EffectOperation::drawn_paint_bounds`] gives, whose lengths `effect_to_pixels` maps, and one following the content is placed with it.
/// `None` if the operation has no paint the CPU can draw.
#[allow(clippy::too_many_arguments)]
fn drawn_paint_colors(operation: &EffectOperation, transform: DAffine2, size: DVec2, effect_to_pixels: DMat2, (margin_x, margin_y): (usize, usize), width: usize, height: usize) -> Option<Vec<Color>> {
	let paint = operation.drawn_paint()?;
	let corners = [DVec2::ZERO, DVec2::X, DVec2::Y, DVec2::ONE].map(|corner| transform.transform_point2(corner));
	let content_bounds = [corners.into_iter().reduce(DVec2::min)?, corners.into_iter().reduce(DVec2::max)?];
	let length_spans = spans_along_axes(transform.matrix2 * DMat2::from_diagonal(size.recip()) * effect_to_pixels);
	let fitted = fitted_paint(paint, operation.drawn_paint_bounds(content_bounds, length_spans));
	let placement = match fitted.is_none() && operation.drawn_paint_follows_content() {
		true => transform,
		false => DAffine2::IDENTITY,
	};
	if !placement.matrix2.determinant().is_normal() {
		return None;
	}

	// A blur's radius mask image continues its edge pixels across the whole image, as the GPU pads it, so it blurs everything the effects before it paint past the content
	let continued = match paint {
		Graphic::RasterCPU(map) if matches!(operation, EffectOperation::Blur { .. }) => {
			let size = DVec2::new(map.element().width as f64, map.element().height as f64);
			continued_map_pixels(paint, size, content_bounds).zip(continued_map_raster(map))
		}
		_ => None,
	};
	let sample_paint: Box<dyn Fn(DVec2) -> Color + Sync + '_> = match continued {
		Some((map_pixels, (raster, opacity))) => {
			let to_pixels = map_pixels.inverse();
			let last_center = DVec2::new(raster.width as f64, raster.height as f64) - 0.5;
			Box::new(move |point| {
				let color = sample_bilinear_gamma(&raster, to_pixels.transform_point2(point).clamp(DVec2::splat(0.5), last_center));
				color.with_alpha(color.a() * opacity)
			})
		}
		// TODO: The CPU can't draw vector, text, GPU image, or effected group paints, so pixel-reading nodes get their content unmasked, unpainted, untextured, or unblurred.
		// Fix by porting this baking's callers to read Vello's renders back to the CPU, then deleting this baking.
		None => match paint_sampler(fitted.as_ref().unwrap_or(paint)) {
			Some(sample_paint) => sample_paint,
			None => {
				log::warn!("An effect's paint is left out of the pixels, since only the renderers can draw a graphic of its kind");
				return None;
			}
		},
	};

	// The paint at each pixel's center, in the space the paint is placed in
	let pixels_to_placement = placement.inverse() * transform * DAffine2::from_scale(size.recip()) * DAffine2::from_translation(-DVec2::new(margin_x as f64, margin_y as f64));
	let mut colors = vec![Color::TRANSPARENT; width * height];
	fill_rows(&mut colors, width, |y, row| {
		for (x, color) in row.iter_mut().enumerate() {
			*color = sample_paint(pixels_to_placement.transform_point2(DVec2::new(x as f64 + 0.5, y as f64 + 0.5)));
		}
	});
	Some(colors)
}

/// Samples the paint's color at points in the space its placement maps into, faded by its opacity, for the paints the CPU can draw: colors, images, generators, linear or radial gradients, and those nested in a placement of their own.
/// The paint's own adjustments are carried out first, and an image's own effects too.
fn paint_sampler(paint: &Graphic) -> Option<Box<dyn Fn(DVec2) -> Color + Sync + '_>> {
	let inverse = |placement: DAffine2| (placement.matrix2.determinant() != 0.).then(|| placement.inverse());
	let faded = |color: Color, opacity: f32| color.with_alpha(color.a() * opacity);

	match paint {
		Graphic::Color(item) => {
			// The fill fades the color beneath its paint overlays, which paint over it at full strength, as the renderers fill an endless color
			let opacity = item.attribute_cloned_or::<f64>(ATTR_OPACITY, 1.) as f32;
			let fill = item.attribute_cloned_or::<f64>(ATTR_OPACITY_FILL, 1.) as f32;
			let adjustments = item.attribute::<Effects>(ATTR_EFFECTS).map(adjustments).unwrap_or_default();
			let color = filled_color(*item.element(), &adjustments, fill);
			Some(Box::new(move |_| faded(color, opacity)))
		}
		Graphic::RasterCPU(item) => {
			// Carrying out an image's effects fades it by its fill, which they take from it, so its opacity is found after
			let item = evaluate_raster_effects((**item).clone());
			let opacity = paint_opacity(&item);
			let to_pixels = DAffine2::from_scale(DVec2::new(item.element().width as f64, item.element().height as f64)) * inverse(item.attribute_cloned_or_default(ATTR_TRANSFORM))?;
			Some(Box::new(move |point| faded(sample_bilinear_gamma(item.element(), to_pixels.transform_point2(point)), opacity)))
		}
		Graphic::Generator(item) => {
			let opacity = paint_opacity(item);
			let to_pattern = inverse(item.attribute_cloned_or_default(ATTR_TRANSFORM))?;
			let sample = pattern_sampler(&GeneratorPattern::of_item(item));
			Some(Box::new(move |point| faded(sample(to_pattern.transform_point2(point)), opacity)))
		}
		Graphic::Gradient(item) => {
			let opacity = paint_opacity(item);
			let item = evaluate_gradient_effects(item.clone());
			let position = gradient_position(&item)?;
			let evaluator = item.element().evaluator(GradientSettings::from_item_attributes(&item));
			Some(Box::new(move |point| faded(evaluator.evaluate(position(point)), opacity)))
		}
		Graphic::Graphic(item) => nested_sampler(item.element(), &|key| item.attributes().get_any(key)),
		// Each graphic of a list lies over those before it, which only the renderers can mix by other blend modes
		Graphic::GraphicList(list) => {
			let samplers = (0..list.len())
				.map(|index| {
					let attribute = |key: &str| list.attribute_any(key, index);
					let normal = attribute(ATTR_BLEND_MODE)
						.and_then(|blend_mode| blend_mode.downcast_ref::<BlendMode>())
						.is_none_or(|&blend_mode| blend_mode == BlendMode::Normal);
					normal.then(|| nested_sampler(list.element(index)?, &attribute)).flatten()
				})
				.collect::<Option<Vec<_>>>()?;
			Some(Box::new(move |point| {
				let stacked = samplers.iter().fold([0.; 4], |beneath: [f32; 4], sampler| {
					let over = premultiplied_gamma_channels(sampler(point));
					std::array::from_fn(|channel| over[channel] + beneath[channel] * (1. - over[3]))
				});
				straight_linear_color(stacked)
			}))
		}
		_ => None,
	}
}

/// Samples a graphic nested within an envelope, with the `attribute` lookup giving its placement and opacity, or `None` if the envelope holds effects, which only the renderers can carry out on everything within it.
fn nested_sampler<'a>(element: &'a Graphic, attribute: &dyn Fn(&str) -> Option<&'a dyn std::any::Any>) -> Option<Box<dyn Fn(DVec2) -> Color + Sync + 'a>> {
	let read = |key: &str| attribute(key).and_then(|value| value.downcast_ref::<f64>()).copied().unwrap_or(1.);
	if attribute(ATTR_EFFECTS).is_some() {
		return None;
	}

	let placement = attribute(ATTR_TRANSFORM).and_then(|value| value.downcast_ref::<DAffine2>()).copied().unwrap_or_default();
	let to_nested = (placement.matrix2.determinant() != 0.).then(|| placement.inverse())?;
	let opacity = (read(ATTR_OPACITY) * read(ATTR_OPACITY_FILL)) as f32;
	let sample = paint_sampler(element)?;
	Some(Box::new(move |point| {
		let color = sample(to_nested.transform_point2(point));
		color.with_alpha(color.a() * opacity)
	}))
}

/// The alpha multiplier an item's opacity attributes apply when it serves as a paint.
fn paint_opacity<T>(item: &Item<T>) -> f32 {
	(item.attribute_cloned_or::<f64>(ATTR_OPACITY, 1.) * item.attribute_cloned_or::<f64>(ATTR_OPACITY_FILL, 1.)) as f32
}

/// The image's color at a point in its pixels, interpolated between the four pixels nearest it on their premultiplied gamma-encoded channels as the GPU samples a texture, or transparent outside the image.
fn sample_bilinear_gamma(image: &Image<Color>, point: DVec2) -> Color {
	let size = DVec2::new(image.width as f64, image.height as f64);
	if !(point.cmpge(DVec2::ZERO).all() && point.cmplt(size).all()) {
		return Color::TRANSPARENT;
	}

	let point = point.clamp(DVec2::splat(0.5), size - 0.5);
	let corner = (point - 0.5).floor();
	let fraction = (point - 0.5 - corner).as_vec2();

	let channels_at = |offset_x: i64, offset_y: i64| {
		let (x, y) = (corner.x as i64 + offset_x, corner.y as i64 + offset_y);
		let pixel = (x < image.width as i64 && y < image.height as i64)
			.then(|| image.data.get((y * image.width as i64 + x) as usize))
			.flatten();
		pixel.map_or([0.; 4], |&pixel| premultiplied_gamma_channels(pixel))
	};
	let lerp = |a: [f32; 4], b: [f32; 4], t: f32| std::array::from_fn::<f32, 4, _>(|channel| a[channel] + (b[channel] - a[channel]) * t);

	let top = lerp(channels_at(0, 0), channels_at(1, 0), fraction.x);
	let bottom = lerp(channels_at(0, 1), channels_at(1, 1), fraction.x);
	straight_linear_color(lerp(top, bottom, fraction.y))
}

/// How far along the gradient each point lies, or `None` where its placement collapses it.
fn gradient_position(item: &Item<Gradient>) -> Option<Box<dyn Fn(DVec2) -> f64 + Sync>> {
	let placement: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);

	match item.attribute_cloned_or_default::<GradientForm>(ATTR_GRADIENT_FORM) {
		// Measured along the line across the bands, which stay parallel to the placement's sheared y axis as the renderers draw them
		GradientForm::Linear => {
			let axis = placement.matrix2.x_axis;
			let line = axis.try_project_onto(placement.matrix2.y_axis.perp()).unwrap_or(axis);
			let origin = placement.translation;
			(line.length_squared() > 0.).then(|| Box::new(move |point: DVec2| (point - origin).dot(line) / line.length_squared()) as Box<dyn Fn(DVec2) -> f64 + Sync>)
		}
		GradientForm::Radial => {
			let to_unit = (placement.matrix2.determinant() != 0.).then(|| placement.inverse())?;
			Some(Box::new(move |point| to_unit.transform_point2(point).length()))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_margin_too_large_is_cut_short_to_fit_the_pixel_limit() {
		// A tall strip whose margins alone would grow it eightfold past the limit
		let size = DVec2::new(1., 16_384.);
		let margin = DVec2::splat(3000.);

		let fitted = (margin * margin_fraction_that_fits(size, margin)).floor();
		let grown_area = (size + 2. * fitted).element_product();

		assert!(grown_area <= MAX_PIXELS_WITH_MARGIN, "the grown image has {grown_area} pixels");
		assert!(grown_area > 0.99 * MAX_PIXELS_WITH_MARGIN, "the grown image has only {grown_area} pixels");
	}

	#[test]
	fn a_feathered_mask_reads_past_the_image_without_growing_it() {
		use graphic_types::MaskMode;

		// A white image of 10 by 10 pixels at a pixel per unit, masked by a white image without a placement, which stretches across it and no further
		let mask = EffectOperation::Mask {
			mask: Graphic::from(Item::new_from_element(Raster::new_cpu(Image::new(1, 1, Color::WHITE)))),
			mode: MaskMode::Luminance,
			density: 100.,
			feather: 2.,
			invert: false,
		};
		let mut item = Item::new_from_element(Raster::new_cpu(Image::new(10, 10, Color::WHITE))).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::splat(10.)));
		graphic_types::stamp_effect(&mut item, graphic_types::Effect::new(&mask));
		let evaluated = evaluate_raster_effects(item);
		let image = evaluated.element();
		assert_eq!((image.width, image.height), (10, 10), "a mask paints nothing past the image");

		// The feather finds the mask's edge where the image ends, about 0.6 shown half a pixel within it, rather than an edge extended past it
		let alpha_at = |x: u32, y: u32| image.data[(y * image.width + x) as usize].a();
		assert!((0.5..0.7).contains(&alpha_at(0, 5)), "{}", alpha_at(0, 5));
		assert!(alpha_at(5, 5) > 0.9, "{}", alpha_at(5, 5));
	}

	#[test]
	fn image_paints_fade_by_their_fill_once_with_effects_of_their_own() {
		use graphic_types::EffectKind;

		// A white image at half fill with a Levels row changing nothing, which carrying out its effects fades by the fill
		let image = Item::new_from_element(Raster::new_cpu(Image::new(2, 2, Color::WHITE)))
			.with_attribute(ATTR_EFFECTS, Effects::from_operations([EffectOperation::default_for(EffectKind::Levels)]))
			.with_attribute(ATTR_OPACITY_FILL, 0.5_f64);
		let paint = Graphic::from(image);
		let Some(sample) = paint_sampler(&paint) else { panic!("the CPU draws images") };
		assert!((sample(DVec2::splat(0.5)).a() - 0.5).abs() < 1e-6, "{:?}", sample(DVec2::splat(0.5)));
	}

	#[test]
	fn blurs_without_a_radius_mask_drawing_something_blur_evenly() {
		let blurred = |radius_mask: Option<Graphic>| {
			let mut row = graphic_types::Effect::new(&EffectOperation::Blur {
				radius_mask: Graphic::default(),
				radius: 3.,
				box_blur: false,
				gamma: false,
			});
			if let Some(radius_mask) = radius_mask {
				row.0.set_attribute("radius_mask", radius_mask);
			}
			let mut item = Item::new_from_element(Raster::new_cpu(Image::new(20, 20, Color::WHITE))).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::splat(20.)));
			graphic_types::stamp_effect(&mut item, row);
			evaluate_raster_effects(item)
		};

		// A radius mask left out of the row, and those the row carries that draw nothing, give the very same even blur
		let plain = blurred(None);
		let no_pixels = Graphic::from(Item::new_from_element(Raster::new_cpu(Image::new(0, 0, Color::WHITE))));
		for radius_mask in [Graphic::default(), Graphic::GraphicList(core_types::list::List::new()), no_pixels] {
			assert!(blurred(Some(radius_mask)) == plain, "a radius mask drawing nothing changes the blur");
		}
	}

	#[test]
	fn a_blurs_radius_mask_image_blurs_everything_the_effects_before_it_paint() {
		// A hard shadow cast 30 pixels right of content 20 wide, far past the blur's own reach of 9
		let shadow = EffectOperation::DropShadow {
			color: Color::BLACK,
			blend_mode: BlendMode::Normal,
			angle: 180.,
			distance: 30.,
			spread: 0.,
			size: 0.,
			contour: Default::default(),
			knockout: false,
		};

		// A coarse white map, and a fine one 1024 pixels across the content, where 512 of its pixels past its side reach only 10 pixels past the content
		for (map_width, map_height) in [(2, 2), (1024, 1)] {
			let white = Graphic::from(Item::new_from_element(Raster::new_cpu(Image::new(map_width, map_height, Color::WHITE))));
			let blur = EffectOperation::Blur {
				radius_mask: white,
				radius: 3.,
				box_blur: false,
				gamma: false,
			};
			let mut item = Item::new_from_element(Raster::new_cpu(Image::new(20, 20, Color::WHITE))).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::splat(20.)));
			for operation in [&shadow, &blur] {
				graphic_types::stamp_effect(&mut item, graphic_types::Effect::new(operation));
			}
			let evaluated = evaluate_raster_effects(item);
			let raster = evaluated.element();
			let (width, height) = (raster.width as usize, raster.height as usize);
			let alpha = |x: usize| raster.data[(height / 2) * width + x].a();

			// The white map blurs everywhere, so the shadow's far edge is soft
			let far_edge = (width - 20) / 2 + 50;
			let soft = (far_edge - 4..far_edge + 4).filter(|&x| (0.05..0.95).contains(&alpha(x))).count();
			assert!(soft >= 4, "only {soft} pixels soften the shadow's far edge with a map {map_width} by {map_height}");
		}
	}

	#[test]
	fn contour_gradient_outlines_measure_their_span_in_document_units() {
		use core_types::list::ATTR_GRADIENT_SPACE;
		use vector_types::gradient::{GradientSpace, GradientStop};

		// A gradient from black to white in gamma, so a pixel's gray is its place across the outline
		let stops = [(0., Color::BLACK), (1., Color::WHITE)].map(|(position, color)| GradientStop { position, midpoint: 0.5, color });
		let paint = Graphic::from(Item::new_from_element(Gradient::new(stops)).with_attribute(ATTR_GRADIENT_SPACE, GradientSpace::RgbGamma));

		// At two pixels per unit, an outline of 3 aligned outside spans 8 pixels from 7 past the edge
		let mut item = Item::new_from_element(Raster::new_cpu(Image::new(20, 20, Color::WHITE))).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::splat(10.)));
		let outline = EffectOperation::Outline {
			paint,
			blend_mode: BlendMode::Normal,
			weight: 3.,
			align: graphic_types::vector_types::vector::style::StrokeAlign::Outside,
			contour_gradient: true,
		};
		graphic_types::stamp_effect(&mut item, graphic_types::Effect::new(&outline));
		let evaluated = evaluate_raster_effects(item);
		let raster = evaluated.element();
		let width = raster.width as usize;
		let margin = (width - 20) / 2;
		let gray = |x: usize| raster.data[(margin + 10) * width + x].to_gamma_srgb_channels()[0];

		// The first pixel outside lies half a pixel past the edge, (7 - 0.5) / 8 across, and the band's last lies 5.5 past it
		assert!((gray(margin - 1) - 207. / 255.).abs() < 1e-3, "the first pixel outside is {}", gray(margin - 1) * 255.);
		assert!((gray(margin - 6) - 48. / 255.).abs() < 1e-3, "the band's last pixel is {}", gray(margin - 6) * 255.);
	}

	#[test]
	fn textured_bevels_shade_nothing_beneath_the_content_past_their_slopes() {
		use vector_types::gradient::GradientStop;

		// A ramp from black to white across far more than the content, which lights flat ground wherever it reaches
		let stops = [(0., Color::BLACK), (1., Color::WHITE)].map(|(position, color)| GradientStop { position, midpoint: 0.5, color });
		let ramp = Item::new_from_element(Gradient::new(stops))
			.with_attribute(ATTR_TRANSFORM, DAffine2::from_translation(DVec2::splat(-100.)) * DAffine2::from_scale(DVec2::splat(300.)))
			.with_attribute(ATTR_GRADIENT_FORM, GradientForm::Linear);

		let mut emboss = EffectOperation::default_for(graphic_types::EffectKind::BevelAndEmboss);
		if let EffectOperation::BevelAndEmboss { style, size, texture, .. } = &mut emboss {
			(*style, *size, *texture) = (BevelStyle::Emboss, 10., Graphic::from(ramp));
		}
		let mut item = Item::new_from_element(Raster::new_cpu(Image::new(20, 20, Color::WHITE))).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::splat(20.)));
		graphic_types::stamp_effect(&mut item, graphic_types::Effect::new(&emboss));
		let evaluated = evaluate_raster_effects(item);
		let raster = evaluated.element();
		let width = raster.width as usize;
		let margin = (width - 20) / 2;
		let alpha = |x: usize| raster.data[(margin + 10) * width + x].a();

		// An emboss of 10 spans 5 each way, so its slopes reach their foot 6 pixels out
		assert!(alpha(margin - 3) > 0., "the slope beneath the content goes unshaded");
		for x in 0..margin - 7 {
			assert!(alpha(x) < 0.5 / 255., "pixel {x} past the foot is shaded by {}", alpha(x));
		}
	}

	#[test]
	fn an_inner_shadow_of_only_choke_shades_an_opaque_raster() {
		use graphic_types::{Effect, stamp_effect};

		let raster = Raster::new_cpu(Image::new(8, 8, Color::WHITE));
		let mut item = Item::new_from_element(raster).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::splat(8.)));
		let inner_shadow = EffectOperation::InnerShadow {
			color: Color::BLACK,
			blend_mode: BlendMode::Multiply,
			angle: 0.,
			distance: 0.,
			choke: 100.,
			size: 2.,
			contour: TransferCurve::default(),
		};
		stamp_effect(&mut item, Effect::new(&inner_shadow));

		let evaluated = evaluate_raster_effects(item);
		let raster = evaluated.element();
		let width = raster.width as usize;
		let pixel = |x: usize, y: usize| raster.data[y * width + x];

		// The margin around the content lies outside it, which the shadow grows inward from
		let margin = (width - 8) / 2;
		assert!(pixel(margin, margin + 4).r() < 0.01, "the content's edge is shaded");
		assert!(pixel(margin + 4, margin + 4).r() > 0.99, "the content's middle is not");
	}

	#[test]
	fn a_gradient_paint_overlays_with_its_own_adjustments_carried_out() {
		use graphic_types::{Effect, stamp_effect};
		use vector_types::gradient::GradientStop;

		// A black to white gradient, inverted by its own effect, painted over a raster two pixels wide
		let gradient = Gradient::new([0., 1.].map(|position| GradientStop {
			position,
			midpoint: 0.5,
			color: Color::from_rgbaf32_unchecked(position as f32, position as f32, position as f32, 1.),
		}));
		let mut paint = Item::new_from_element(gradient);
		stamp_effect(&mut paint, Effect::new(&EffectOperation::Adjustment(Adjustment::Invert)));

		let raster = Raster::new_cpu(Image::new(2, 1, Color::WHITE));
		let mut item = Item::new_from_element(raster).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::new(2., 1.)));
		let overlay = EffectOperation::PaintOverlay {
			paint: Graphic::Gradient(paint),
			blend_mode: BlendMode::Normal,
			opacity: 100.,
		};
		stamp_effect(&mut item, Effect::new(&overlay));

		let evaluated = evaluate_raster_effects(item);
		let raster = evaluated.element();
		let (left, right) = (raster.data[0], raster.data[raster.width as usize - 1]);
		assert!(left.r() > right.r(), "the inverted gradient runs from {left:?} to {right:?}");
	}

	#[test]
	fn a_paint_overlays_faded_by_its_own_opacity() {
		use graphic_types::{Effect, stamp_effect};
		use vector_types::gradient::GradientStop;

		// A black gradient at half opacity, painted over white
		let gradient = Gradient::new([0., 1.].map(|position| GradientStop {
			position,
			midpoint: 0.5,
			color: Color::BLACK,
		}));
		let paint = Item::new_from_element(gradient).with_attribute(ATTR_OPACITY, 0.5_f64);

		let mut item = Item::new_from_element(Raster::new_cpu(Image::new(1, 1, Color::WHITE)));
		let overlay = EffectOperation::PaintOverlay {
			paint: Graphic::Gradient(paint),
			blend_mode: BlendMode::Normal,
			opacity: 100.,
		};
		stamp_effect(&mut item, Effect::new(&overlay));

		let color = evaluate_raster_effects(item).element().data[0];
		assert!(color.r() > 0.05 && color.r() < 0.95, "half the black paint leaves {color:?}");
	}

	#[test]
	fn adjustments_leave_fully_transparent_pixels_empty() {
		use graphic_types::adjustment::{CurvesSettings, LevelsRecord, LevelsSettings};

		let pixels = [[0.; 4], [0.2, 0.1, 0.05, 0.5], [0.3, 0.3, 0.3, 0.]];
		let adjusted = |run: &[Adjustment]| {
			let mut pixels = pixels;
			run.iter().for_each(|adjustment| adjust_premultiplied(&mut pixels, adjustment));
			pixels
		};

		// Nothing raises the alpha of a pixel with none, be it Make Opaque or a Levels alpha output starting above zero
		let identity = LevelsRecord {
			shadows: 0.,
			midtones: 1.,
			highlights: 100.,
			output_minimums: 0.,
			output_maximums: 100.,
		};
		let raised_alpha = Adjustment::Levels(Box::new(LevelsSettings {
			composite: identity,
			red: identity,
			green: identity,
			blue: identity,
			alpha: LevelsRecord { output_minimums: 50., ..identity },
		}));
		for adjustment in [Adjustment::MakeOpaque, raised_alpha] {
			let [empty, translucent, colored_without_alpha] = adjusted(std::slice::from_ref(&adjustment));
			assert_eq!((empty, colored_without_alpha), ([0.; 4], [0.; 4]), "{adjustment:?}");
			assert!(translucent[3] > 0.5, "{adjustment:?}");
		}

		// A pixel an adjustment empties stays empty for those after it
		let emptied = Adjustment::Curves(Box::new(CurvesSettings {
			alpha: TransferCurve::new(vec![DVec2::ZERO, DVec2::new(1., 0.)]),
			..Default::default()
		}));
		assert_eq!(adjusted(&[emptied, Adjustment::MakeOpaque]), [[0.; 4]; 3]);
	}

	#[test]
	fn interior_pieces_stack_within_the_contents_share_over_nothing() {
		use crate::layer_blending::blend_pixel;

		let (content, paint, strength, alpha) = ([0.9, 0.5, 0.2], [0.3, 0.8, 0.4], 0.75, 0.5);
		for (blend_mode, fill) in [
			(BlendMode::Multiply, 1.),
			(BlendMode::Color, 1.),
			(BlendMode::ColorDodge, 1.),
			(BlendMode::Screen, 0.5),
			(BlendMode::Multiply, 0.),
		] {
			let color = Color::from_gamma_srgb_channels(content[0], content[1], content[2], alpha);
			let overlay = EffectOperation::Adjustment(Adjustment::ColorOverlay {
				color: Color::from_gamma_srgb_channels(paint[0], paint[1], paint[2], 1.),
				blend_mode,
				opacity: strength as f64 * 100.,
			});
			let item = Item::new_from_element(Raster::new_cpu(Image::new(1, 1, color)))
				.with_attribute(ATTR_EFFECTS, Effects::from_operations([overlay]))
				.with_attribute(ATTR_OPACITY_FILL, fill as f64);
			let [red, green, blue, actual_alpha] = evaluate_raster_effects(item).element().data[0].to_gamma_srgb_channels();

			// For PSD interop, the faded content and the piece stack as layers over nothing within the share, which then scales the stack
			let faded = content.map(|channel| channel * fill).into_iter().chain([fill]).collect::<Vec<_>>();
			let stacked = blend_pixel(
				paint.map(|channel| channel * strength).into_iter().chain([strength]).collect::<Vec<_>>().try_into().unwrap(),
				faded.try_into().unwrap(),
				blend_mode,
			);
			let expected_alpha = alpha * stacked[3];
			let expected = [0, 1, 2].map(|channel| stacked[channel] / stacked[3]);
			let close = [red, green, blue].iter().zip(expected).all(|(actual, expected)| (actual - expected).abs() < 2e-3) && (actual_alpha - expected_alpha).abs() < 2e-3;
			assert!(close, "{blend_mode:?} at fill {fill}: {:?} against {expected:?} at {expected_alpha}", [red, green, blue, actual_alpha]);
		}
	}
}
