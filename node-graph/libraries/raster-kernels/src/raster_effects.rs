//! Carries out a raster's effects on the CPU at the raster's own resolution, for the nodes that read its pixels.
//! Each step mirrors the renderer's GPU passes, working on premultiplied gamma-encoded channels as they do.

use crate::effect_kernels::{BlurKernel, NEGLIGIBLE_LENGTH, Plane};
use core_types::color::{linear_to_srgb, srgb_to_linear};
use core_types::list::{ATTR_EFFECTS, Item};
use core_types::transfer_curve::TransferCurve;
use core_types::{ATTR_OPACITY_FILL, ATTR_TRANSFORM, Color};
use glam::{DAffine2, DMat2, DVec2};
use graphic_types::effects::{EffectOperation, Effects, GlowFill, GlowTechnique, effect_space, grow_and_tent, precise_falloff, shadow_offset, shape_strength, spans_along_axes};
use raster_types::{CPU, Image, Raster};
use std::ops::Range;

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

/// The image of the effects carried out on `raster`, which `transform` places in the document, with the transform placing that larger image.
/// `None` when there's nothing to carry out, or the image is empty or its transform can't be inverted.
fn evaluate(raster: &Raster<CPU>, transform: DAffine2, effects: &Effects, fill: f32) -> Option<(Image<Color>, DAffine2)> {
	let (width, height) = (raster.width as usize, raster.height as usize);
	if (effects.change_nothing() && fill >= 1.) || width == 0 || height == 0 || transform.matrix2.determinant() == 0. {
		return None;
	}
	let size = DVec2::new(width as f64, height as f64);

	// Each effect's lengths become pixels along each of the image's axes, and its offsets map from its effect space through the inverse of the image's pixel placement
	let document_to_pixels = (transform.matrix2 * DMat2::from_diagonal(size.recip())).inverse();
	let runs: Vec<(DMat2, Range<usize>)> = (effects.runs().into_iter())
		.map(|(stamped_transform, rows)| (document_to_pixels * effect_space(transform, stamped_transform).matrix2, rows))
		.collect();

	// A margin around the image to hold the blur and shadows cast past its edges, cut short evenly for an image that would grow too large
	let run_reach = |rows: &Range<usize>| effects.operations_in(rows.clone()).map(|operation| operation.painted_reach()).sum::<f64>();
	let reach_in_pixels: DVec2 = runs.iter().map(|(effect_to_pixels, rows)| run_reach(rows) * spans_along_axes(*effect_to_pixels)).sum();
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
	for (y, row) in raster.data.chunks_exact(width).enumerate() {
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
	let operations = (runs.iter().enumerate())
		.flat_map(|(run_index, (effect_to_pixels, rows))| (effects.operations_in(rows.clone()).enumerate()).map(move |(index, operation)| (operation, *effect_to_pixels, run_index > 0 && index == 0)));
	for (effect, effect_to_pixels, starts_later_run) in operations {
		if starts_later_run {
			full = match (filled.take(), beneath.take()) {
				(filled, Some(beneath)) => composite_beneath(filled.as_ref().unwrap_or(&full), &beneath),
				(Some(filled), None) => filled,
				(None, None) => full,
			};
			run_fill = 1.;
		}
		let pixels_per_length = spans_along_axes(effect_to_pixels);
		match effect {
			EffectOperation::Blur { radius, box_blur, gamma } => {
				let radius = radius * pixels_per_length;
				if radius.max_element() < NEGLIGIBLE_LENGTH {
					continue;
				}

				// What lies beneath blurs together with the content
				if let Some(beneath) = beneath.take() {
					filled = filled.map(|filled| composite_beneath(&filled, &beneath));
					full = composite_beneath(&full, &beneath);
				}

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
				contour,
				knockout,
			} => {
				let offset = effect_to_pixels * shadow_offset(angle, distance);
				let silhouette = full.map(|[_, _, _, alpha]| alpha).shifted(offset.x, offset.y);
				let strength = cast(silhouette, size, spread, GlowTechnique::Softer, 100., &contour, pixels_per_length);
				beneath = Some(accumulate(beneath, &strength, color, knockout, &full, run_fill));
			}
			EffectOperation::OuterGlow {
				color,
				technique,
				spread,
				size,
				range,
				contour,
			} => {
				let Some(paint) = GlowFill::of(&color) else { continue };
				let raw = cast_strength(full.map(|[_, _, _, alpha]| alpha), size, spread, technique, pixels_per_length);
				let strength = shaped(raw.clone(), range, &contour);
				beneath = Some(stack_beneath(beneath, glow_tint(&paint, &strength, &raw, true), true, &full, run_fill));
			}
			EffectOperation::InnerShadow {
				color,
				angle,
				distance,
				choke,
				size,
				contour,
			} => {
				// Cast from the area outside the content, which the margin and anything moved in from past it belong to
				let offset = effect_to_pixels * shadow_offset(angle, distance);
				let outside = full.map(|[_, _, _, alpha]| alpha).shifted(offset.x, offset.y).map(|alpha| 1. - alpha);
				let strength = cast(outside, size, choke, GlowTechnique::Softer, 100., &contour, pixels_per_length);
				filled = filled.map(|filled| composite_within(&filled, &strength, &full, color));
				full = composite_within(&full, &strength, &full, color);
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
	let coverages = coverage.zip(full_content, |coverage, content| [coverage, content[3], 0., 0.]);

	image.zip(&coverages, |image, [coverage, content_alpha, _, _]| {
		let tinted = color.map(|channel| channel * coverage);
		std::array::from_fn(|channel| tinted[channel] * content_alpha + image[channel] * (1. - tinted[3]))
	})
}

/// The image with the shadows and glows `beneath` placed under it.
fn composite_beneath(image: &Plane<[f32; 4]>, beneath: &Plane<[f32; 4]>) -> Plane<[f32; 4]> {
	image.zip(beneath, |image, beneath| std::array::from_fn(|channel| image[channel] + beneath[channel] * (1. - image[3])))
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
}
