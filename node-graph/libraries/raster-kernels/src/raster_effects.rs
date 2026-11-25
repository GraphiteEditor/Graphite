//! Carries out a raster's effects on the CPU at the raster's own resolution, for the nodes that read its pixels.
//! Each step mirrors the renderer's GPU passes, working on premultiplied gamma-encoded channels as they do.

use crate::adjustments::adjust_colors;
use crate::blending::overlay_paint;
use crate::effect_kernels::{BlurKernel, NEGLIGIBLE_LENGTH, Plane};
use crate::generators::pattern_sampler;
use crate::value_effects::evaluate_gradient_effects;
use core_types::FallibleVec2Operations;
use core_types::blending::{BlendMode, apply_blend_mode};
use core_types::color::{linear_to_srgb, srgb_to_linear};
use core_types::list::{ATTR_EFFECTS, ATTR_GRADIENT_FORM, Item};
use core_types::transfer_curve::TransferCurve;
use core_types::{ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_TRANSFORM, Color};
use glam::{DAffine2, DMat2, DVec2, DVec3};
use graphic_types::Graphic;
use graphic_types::adjustment::Adjustment;
use graphic_types::effects::{
	BevelDirection, BevelStyle, BevelTechnique, EffectOperation, Effects, GlowFill, GlowSource, GlowTechnique, bevel_light, bevel_lighting, effect_space, fitted_paint, grow_and_tent, outline_extents,
	precise_falloff, shadow_offset, shape_strength, spans_along_axes,
};
use graphic_types::generator::GeneratorPattern;
use raster_types::parallel_rows::fill_rows;
use raster_types::{CPU, Image, Raster};
use vector_types::gradient::{Gradient, GradientForm, GradientSettings};

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

/// The image of the effects carried out on `raster`, which `transform` places in the document, with the transform placing that larger image.
/// `None` when there's nothing to carry out, or the image is empty or its transform can't be inverted.
fn evaluate(raster: &Raster<CPU>, transform: DAffine2, effects: &Effects, fill: f32) -> Option<(Image<Color>, DAffine2)> {
	let (width, height) = (raster.width as usize, raster.height as usize);
	if (effects.change_nothing() && fill >= 1.) || width == 0 || height == 0 || transform.matrix2.determinant() == 0. {
		return None;
	}
	let size = DVec2::new(width as f64, height as f64);

	// Each effect's lengths become pixels along each of the image's axes, and its offsets map from its effect space through the inverse of the image's pixel placement
	let document_to_image_pixels = (transform.matrix2 * DMat2::from_diagonal(size.recip())).inverse();
	let operations: Vec<(EffectOperation, DMat2, bool)> = (effects.runs().into_iter().enumerate())
		.flat_map(|(run_index, (stamped_transform, rows))| {
			let effect_to_pixels = document_to_image_pixels * effect_space(transform, stamped_transform).matrix2;
			(effects.operations_in(rows).enumerate()).map(move |(index, operation)| (operation, effect_to_pixels, run_index > 0 && index == 0))
		})
		.collect();

	// Unless a fill fades them first, leading adjustments change the image's own colors, keeping even transparent pixels' colors
	let leading_adjustments = operations.iter().take_while(|_| fill == 1.).map_while(|(operation, ..)| match operation {
		EffectOperation::Adjustment(adjustment) => Some(adjustment),
		_ => None,
	});
	let mut colors = std::borrow::Cow::Borrowed(raster.data.as_slice());
	let mut remaining = operations.as_slice();
	for adjustment in leading_adjustments {
		adjust_colors(adjustment, colors.to_mut());
		remaining = &remaining[1..];
	}
	if remaining.is_empty() {
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
	let operations = remaining;

	// A margin around the image to hold the blur and shadows cast past its edges, cut short evenly for an image that would grow too large
	let reach_in_pixels: DVec2 = (operations.iter())
		.map(|(operation, effect_to_pixels, _)| operation.painted_reach() * spans_along_axes(*effect_to_pixels))
		.sum();
	let mut margin = reach_in_pixels.ceil();
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

	// Each run after the first takes the result of those before it as its content at full strength, like the renderers' nested runs
	let mut run_fill = fill;
	for &(ref operation, effect_to_pixels, starts_later_run) in operations {
		if starts_later_run {
			full = match (filled.take(), beneath.take()) {
				(filled, Some(beneath)) => composite_beneath(filled.as_ref().unwrap_or(&full), &beneath),
				(Some(filled), None) => filled,
				(None, None) => full,
			};
			run_fill = 1.;
		}
		let pixels_per_length = spans_along_axes(effect_to_pixels);
		match *operation {
			EffectOperation::Blur { radius, box_blur, gamma } => {
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
			EffectOperation::DropShadow {
				color,
				angle,
				distance,
				spread,
				size,
				ref contour,
				knockout,
			} => {
				let offset = effect_to_pixels * shadow_offset(angle, distance);
				let silhouette = full.map(|[_, _, _, alpha]| alpha).shifted(offset.x, offset.y);
				let strength = cast(silhouette, size, spread, GlowTechnique::Softer, 100., contour, pixels_per_length);
				beneath = Some(accumulate(beneath, &strength, color, knockout, &full, run_fill));
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
				let raw = cast_strength(full.map(|[_, _, _, alpha]| alpha), size, spread, technique, pixels_per_length);
				let strength = shaped(raw.clone(), range, contour);
				beneath = Some(stack_beneath(beneath, glow_tint(&paint, &strength, &raw, true), true, &full, run_fill));
			}
			EffectOperation::InnerShadow {
				color,
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
				filled = filled.map(|filled| composite_within(&filled, &strength, &full, color));
				full = composite_within(&full, &strength, &full, color);
			}
			EffectOperation::InnerGlow {
				ref color,
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
				let tinted = glow_tint(&paint, &strength, &strength, false);
				filled = filled.map(|filled| composite_tinted_within(&filled, &tinted, &full));
				full = composite_tinted_within(&full, &tinted, &full);
			}
			EffectOperation::Satin {
				color,
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
				filled = filled.map(|filled| composite_within(&filled, &strength, &full, color));
				full = composite_within(&full, &strength, &full, color);
			}
			EffectOperation::Outline { color, weight, align } => {
				// Straight-line distances can't stretch along one axis, so an image scaled unevenly takes the mean of its two scales
				let mean_scale = (pixels_per_length.x * pixels_per_length.y).sqrt();
				let (outside, inside) = outline_extents(weight * mean_scale, align);
				let silhouette = full.map(|[_, _, _, alpha]| alpha);

				// The band outside lies beneath the content like a shadow, and the band inside over it like an inner shadow
				if outside >= NEGLIGIBLE_LENGTH {
					beneath = Some(accumulate(beneath, &silhouette.grown(outside), color, true, &full, run_fill));
				}
				if inside >= NEGLIGIBLE_LENGTH {
					let band = silhouette.map(|alpha| 1. - alpha).grown(inside);
					filled = filled.map(|filled| composite_within(&filled, &band, &full, color));
					full = composite_within(&full, &band, &full, color);
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
			} => {
				// Straight-line distances can't stretch along one axis, so an image scaled unevenly takes the mean of its two scales
				let mean_scale = (pixels_per_length.x * pixels_per_length.y).sqrt();
				let size = size * mean_scale;
				if size < NEGLIGIBLE_LENGTH {
					continue;
				}

				// The surface's height, lit from the light's direction into its highlights and shadows
				let (height, rise) = bevel_height(&full.map(|[_, _, _, alpha]| alpha), style, technique, size);
				let rise = rise * depth / 100. * if direction == BevelDirection::Down { -1. } else { 1. };
				let lit = bevel_shading(&height, rise, bevel_light(angle, altitude, effect_to_pixels));
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
					BevelStyle::OuterBevel | BevelStyle::Emboss => Some(inside.clone().unwrap_or_else(|| softened(lit))),
					// A pillow's slopes outside are lit mirrored, so they rise away from the edge
					BevelStyle::PillowEmboss => Some(softened(bevel_shading(&height, -rise, bevel_light(angle, altitude, effect_to_pixels)))),
				};
				let colors = [(highlight_color, highlight_blend_mode), (shadow_color, shadow_blend_mode)];
				for (amount, (color, blend_mode)) in inside.into_iter().flatten().zip(colors) {
					filled = filled.map(|filled| composite_within_by_mode(&filled, &amount, &full, color, blend_mode));
					full = composite_within_by_mode(&full, &amount, &full, color, blend_mode);
				}
				for (amount, (color, _)) in outside.into_iter().flatten().zip(colors) {
					beneath = Some(accumulate(beneath, &amount, color, true, &full, run_fill));
				}
			}
			EffectOperation::PaintOverlay { ref paint, blend_mode, opacity } => {
				let corners = [DVec2::ZERO, DVec2::X, DVec2::Y, DVec2::ONE].map(|corner| transform.transform_point2(corner));
				let content_bounds = [corners.into_iter().reduce(DVec2::min).unwrap(), corners.into_iter().reduce(DVec2::max).unwrap()];
				let fitted = fitted_paint(paint, content_bounds);
				let Some(sample_paint) = paint_sampler(fitted.as_ref().unwrap_or(paint)) else { continue };

				// The paint at each pixel's center, in the space the image is placed in
				let pixels_to_placement = transform * DAffine2::from_scale(size.recip()) * DAffine2::from_translation(-DVec2::new(margin_x as f64, margin_y as f64));
				let mut paint_colors = vec![Color::TRANSPARENT; full.data.len()];
				fill_rows(&mut paint_colors, full.width, |y, row| {
					for (x, color) in row.iter_mut().enumerate() {
						*color = sample_paint(pixels_to_placement.transform_point2(DVec2::new(x as f64 + 0.5, y as f64 + 0.5)));
					}
				});

				let opacity = (opacity / 100.) as f32;
				filled = filled.map(|filled| overlay(&filled, &paint_colors, blend_mode, opacity));
				full = overlay(&full, &paint_colors, blend_mode, opacity);
			}
			EffectOperation::Adjustment(ref adjustment) => {
				merge_beneath(&mut full, &mut filled, &mut beneath);
				full = adjust(&full, adjustment);
				filled = filled.map(|filled| adjust(&filled, adjustment));
			}
		}
	}

	let result = match (filled, beneath) {
		(filled, Some(beneath)) => composite_beneath(filled.as_ref().unwrap_or(&full), &beneath),
		(Some(filled), None) => filled,
		(None, None) => full,
	};
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
fn premultiplied_gamma_channels(color: Color) -> [f32; 4] {
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
fn blur(plane: Plane<[f32; 4]>, (horizontal, vertical): (BlurKernel, BlurKernel), linear: bool) -> Plane<[f32; 4]> {
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

/// Stacks a silhouette's coverage, tinted `color`, over what lies `beneath` the content so far.
/// If `knocked_out`, it stays hidden behind the full-strength content where a `fill` below 1 fades the content that shows over it.
fn accumulate(beneath: Option<Plane<[f32; 4]>>, coverage: &Plane<f32>, color: Color, knocked_out: bool, full_content: &Plane<[f32; 4]>, fill: f32) -> Plane<[f32; 4]> {
	let color = premultiplied_gamma_channels(color);
	stack_beneath(beneath, coverage.map(|coverage| color.map(|channel| channel * coverage)), knocked_out, full_content, fill)
}

/// Stacks a premultiplied `tinted` image over what lies `beneath` the content so far, knocked out like [`accumulate`] describes.
fn stack_beneath(beneath: Option<Plane<[f32; 4]>>, tinted: Plane<[f32; 4]>, knocked_out: bool, full_content: &Plane<[f32; 4]>, fill: f32) -> Plane<[f32; 4]> {
	let tinted = match knocked_out && fill < 1. {
		// The faded content shows what's beneath through 1 - fill × alpha of it, of which only 1 - alpha may show this
		true => tinted.zip(full_content, |tinted, content| {
			let shown = (1. - content[3]) / (1. - fill * content[3]).max(f32::EPSILON);
			tinted.map(|channel| channel * shown)
		}),
		false => tinted,
	};

	match beneath {
		Some(beneath) => tinted.zip(&beneath, |tinted, beneath| std::array::from_fn(|channel| tinted[channel] + beneath[channel] * (1. - tinted[3]))),
		None => tinted,
	}
}

/// The image with a silhouette's coverage, tinted `color`, placed over it within the full-strength content.
fn composite_within(image: &Plane<[f32; 4]>, coverage: &Plane<f32>, full_content: &Plane<[f32; 4]>, color: Color) -> Plane<[f32; 4]> {
	let color = premultiplied_gamma_channels(color);
	composite_tinted_within(image, &coverage.map(|coverage| color.map(|channel| channel * coverage)), full_content)
}

/// The image with a premultiplied `tinted` image placed over it within the full-strength content.
fn composite_tinted_within(image: &Plane<[f32; 4]>, tinted: &Plane<[f32; 4]>, full_content: &Plane<[f32; 4]>) -> Plane<[f32; 4]> {
	let uncovered = image.zip(tinted, |image, tinted| image.map(|channel| channel * (1. - tinted[3])));
	let within = tinted.zip(full_content, |tinted, content| tinted.map(|channel| channel * content[3]));
	uncovered.zip(&within, |uncovered, within| std::array::from_fn(|channel| uncovered[channel] + within[channel]))
}

/// Places what lies `beneath` under the full-strength content and the faded fill, so a pixel filter after it (a blur or an adjustment) processes them together.
fn merge_beneath(full: &mut Plane<[f32; 4]>, filled: &mut Option<Plane<[f32; 4]>>, beneath: &mut Option<Plane<[f32; 4]>>) {
	let Some(beneath) = beneath.take() else { return };

	*filled = filled.take().map(|filled| composite_beneath(&filled, &beneath));
	*full = composite_beneath(full, &beneath);
}

/// The image with the shadows and glows `beneath` placed under it.
fn composite_beneath(image: &Plane<[f32; 4]>, beneath: &Plane<[f32; 4]>) -> Plane<[f32; 4]> {
	image.zip(beneath, |image, beneath| std::array::from_fn(|channel| image[channel] + beneath[channel] * (1. - image[3])))
}

/// A bevel's height from 0 to 1 over the `style`'s slopes `size` pixels wide, shaped by the `technique`, with how many pixels a height of 1 rises.
/// Smooth slopes are a tent blur of the silhouette, and chiseled ones rise evenly with the distance from its edge.
fn bevel_height(silhouette: &Plane<f32>, style: BevelStyle, technique: BevelTechnique, size: f64) -> (Plane<f32>, f64) {
	// Embossed slopes span half the size on each side of the edge
	let span = match style {
		BevelStyle::InnerBevel | BevelStyle::OuterBevel => size,
		BevelStyle::Emboss | BevelStyle::PillowEmboss => size / 2.,
	};

	match technique {
		BevelTechnique::Smooth => {
			let (_, tent) = grow_and_tent(span, 0.);
			(silhouette.clone().blurred(BlurKernel::Tent { radius: tent }, BlurKernel::Tent { radius: tent }), span)
		}
		BevelTechnique::ChiselHard | BevelTechnique::ChiselSoft => {
			// For PSD interop, the slope continues through the edge from 0 at the first pixel outside, and levels off a pixel past its span having risen half the span.
			// The height spans only the part across the edge the style lights, which keeps its precision.
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

			let (low, high) = (low as f32, high as f32);
			let height = field.map(|field| (field.clamp(low, high) - low) / (high - low));
			(height, span / (2. * (span + 1.)) * (high - low) as f64)
		}
	}
}

/// How much each pixel of a bevel's surface is lit by the `light` past a flat surface's brightness (its highlight), and how much it falls short (its shadow), each from 0 to 1.
/// The surface's height rises `rise` pixels for each unit of `height`, and for PSD interop, its slope at each pixel is the difference across its neighbors averaged over the three rows or columns they span. O(width × height).
fn bevel_shading(height: &Plane<f32>, rise: f64, light: DVec3) -> [Plane<f32>; 2] {
	let (columns, rows) = (height.width as isize, height.height as isize);
	let at = |x: isize, y: isize| height.data[(y.clamp(0, rows - 1) * columns + x.clamp(0, columns - 1)) as usize] as f64;

	let mut lighting = Plane::<[f32; 4]>::new(height.width, height.height);
	fill_rows(&mut lighting.data, height.width, |y, row| {
		let y = y as isize;
		for (x, lit) in row.iter_mut().enumerate() {
			let x = x as isize;
			let across = at(x + 1, y - 1) + at(x + 1, y) + at(x + 1, y + 1) - at(x - 1, y - 1) - at(x - 1, y) - at(x - 1, y + 1);
			let down = at(x - 1, y + 1) + at(x, y + 1) + at(x + 1, y + 1) - at(x - 1, y - 1) - at(x, y - 1) - at(x + 1, y - 1);

			let (highlight, shadow) = bevel_lighting(DVec2::new(across, down) * rise / 6., light);
			*lit = [highlight as f32, shadow as f32, 0., 0.];
		}
	});
	[lighting.map(|[highlight, ..]| highlight), lighting.map(|[_, shadow, ..]| shadow)]
}

/// The image with a coverage, tinted by `color` mixed with the full-strength content's colors by the blend mode in gamma, placed over it within the full-strength content.
fn composite_within_by_mode(image: &Plane<[f32; 4]>, coverage: &Plane<f32>, full_content: &Plane<[f32; 4]>, color: Color, blend_mode: BlendMode) -> Plane<[f32; 4]> {
	if blend_mode == BlendMode::Normal {
		return composite_within(image, coverage, full_content, color);
	}
	let [red, green, blue, opacity] = color.to_gamma_srgb_channels();

	let mut result = Plane::<[f32; 4]>::new(image.width, image.height);
	fill_rows(&mut result.data, image.width, |y, row| {
		for (x, composited) in row.iter_mut().enumerate() {
			let index = y * image.width + x;
			let (pixel, content) = (image.data[index], full_content.data[index]);
			let (strength, alpha) = (opacity * coverage.data[index], content[3]);

			let [tint_red, tint_green, tint_blue] = match alpha > 0. {
				true => blend_gamma([content[0] / alpha, content[1] / alpha, content[2] / alpha], [red, green, blue], blend_mode),
				false => [0.; 3],
			};
			let tinted = [tint_red * alpha, tint_green * alpha, tint_blue * alpha, alpha].map(|channel| channel * strength);
			*composited = std::array::from_fn(|channel| tinted[channel] + pixel[channel] * (1. - strength));
		}
	});
	result
}

/// The blend mode's mix of straight gamma-encoded `backdrop` and `paint` colors.
fn blend_gamma(backdrop: [f32; 3], paint: [f32; 3], blend_mode: BlendMode) -> [f32; 3] {
	match blend_mode {
		// The component modes encode the colors they're given themselves
		BlendMode::Hue | BlendMode::Saturation | BlendMode::Color | BlendMode::Luminosity => {
			let color = |[red, green, blue]: [f32; 3]| Color::from_gamma_srgb_channels(red, green, blue, 1.);
			let [red, green, blue, _] = apply_blend_mode(color(paint), color(backdrop), blend_mode).to_gamma_srgb_channels();
			[red, green, blue]
		}
		_ => {
			let color = |[red, green, blue]: [f32; 3]| Color::from_rgbaf32_unchecked(red, green, blue, 1.);
			let blended = apply_blend_mode(color(paint), color(backdrop), blend_mode);
			[blended.r(), blended.g(), blended.b()]
		}
	}
}

/// Mixes the paint's color at each premultiplied gamma-encoded pixel over it by the blend mode, keeping the pixel's alpha.
fn overlay(plane: &Plane<[f32; 4]>, paint: &[Color], blend_mode: BlendMode, opacity: f32) -> Plane<[f32; 4]> {
	let data = plane
		.data
		.iter()
		.zip(paint)
		.map(|(&pixel, &paint)| match pixel[3] > 0. {
			true => premultiplied_gamma_channels(overlay_paint(straight_linear_color(pixel), paint, blend_mode, opacity)),
			false => [0.; 4],
		})
		.collect();
	Plane {
		width: plane.width,
		height: plane.height,
		data,
	}
}

/// Samples the paint's color at points in the space its placement maps into, faded by its opacity, for the paints the CPU can draw: colors, images, generators, and linear or radial gradients.
fn paint_sampler(paint: &Graphic) -> Option<Box<dyn Fn(DVec2) -> Color + Sync + '_>> {
	let inverse = |placement: DAffine2| (placement.matrix2.determinant() != 0.).then(|| placement.inverse());
	let faded = |color: Color, opacity: f32| color.with_alpha(color.a() * opacity);

	match paint {
		Graphic::Color(item) => {
			let color = faded(*item.element(), paint_opacity(item));
			Some(Box::new(move |_| color))
		}
		Graphic::RasterCPU(item) => {
			let opacity = paint_opacity(item.as_ref());
			let image = item.element();
			let to_pixels = DAffine2::from_scale(DVec2::new(image.width as f64, image.height as f64)) * inverse(item.attribute_cloned_or_default(ATTR_TRANSFORM))?;
			Some(Box::new(move |point| faded(image.sample_bilinear(to_pixels.transform_point2(point)), opacity)))
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
		_ => None,
	}
}

/// The alpha multiplier an item's opacity attributes apply when it serves as a paint.
fn paint_opacity<T>(item: &Item<T>) -> f32 {
	(item.attribute_cloned_or::<f64>(ATTR_OPACITY, 1.) * item.attribute_cloned_or::<f64>(ATTR_OPACITY_FILL, 1.)) as f32
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
	fn an_inner_shadow_of_only_choke_shades_an_opaque_raster() {
		use graphic_types::{Effect, stamp_effect};

		let raster = Raster::new_cpu(Image::new(8, 8, Color::WHITE));
		let mut item = Item::new_from_element(raster).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::splat(8.)));
		let inner_shadow = EffectOperation::InnerShadow {
			color: Color::BLACK,
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
}
