//! Carries out a raster's effects on the CPU at the raster's own resolution, for the nodes that read its pixels.
//! Each step mirrors the renderer's GPU passes, working on premultiplied gamma-encoded channels as they do.

use crate::adjustment::Adjustment;
use crate::effects::{EffectOperation, Effects, GlowSource, outline_extents};
use core_types::color::{linear_to_srgb, srgb_to_linear};
use core_types::list::{ATTR_EFFECTS, Item};
use core_types::{ATTR_OPACITY_FILL, ATTR_TRANSFORM, Color};
use glam::{DAffine2, DMat2, DVec2};
use raster_types::effect_kernels::{BlurKernel, NEGLIGIBLE_LENGTH, Plane};
use raster_types::{CPU, Image, Raster};

/// The most pixels an image may grow to with the margin its effects cast into, beyond which the margin is cut short.
const MAX_PIXELS_WITH_MARGIN: f64 = (1 << 24) as f64;

/// The raster with its effects carried out into its pixels, grown to hold what they cast past its edges, and with the effects and the fill opacity they used removed.
/// A raster without the effects attribute is returned as it is.
pub fn evaluate_raster_effects(item: Item<Raster<CPU>>) -> Item<Raster<CPU>> {
	let Some(operations) = item.attribute::<Effects>(ATTR_EFFECTS).map(|effects| effects.operations().collect::<Vec<_>>()) else {
		return item;
	};
	let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let fill = item.attribute_cloned_or::<f64>(ATTR_OPACITY_FILL, 1.) as f32;

	let (raster, attributes) = item.into_parts();
	let (raster, transform) = match evaluate(&raster, transform, &operations, fill) {
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
	adjustments.iter().for_each(|adjustment| graphene_hash::CacheHash::cache_hash(adjustment, &mut hasher));
	let key = std::hash::Hasher::finish(&hasher);

	raster.derived(key, |raster| {
		let mut image = raster.data().clone();
		adjustments.iter().for_each(|adjustment| adjustment.apply(&mut image.data));
		Raster::new_cpu(image)
	})
}

/// The image of the effects carried out on `raster`, which `transform` places in the document, with the transform placing that larger image.
/// `None` when there's nothing to carry out, or the image is empty or its transform can't be inverted.
fn evaluate(raster: &Raster<CPU>, transform: DAffine2, operations: &[EffectOperation], fill: f32) -> Option<(Image<Color>, DAffine2)> {
	let (width, height) = (raster.width as usize, raster.height as usize);
	if (operations.is_empty() && fill >= 1.) || width == 0 || height == 0 || transform.matrix2.determinant() == 0. {
		return None;
	}
	let size = DVec2::new(width as f64, height as f64);

	// Unless a fill fades them first, leading adjustments change the image's own colors, keeping even transparent pixels' colors
	let leading_adjustments = operations.iter().take_while(|_| fill == 1.).map_while(|operation| match operation {
		EffectOperation::Adjustment(adjustment) => Some(adjustment),
		_ => None,
	});
	let mut colors = std::borrow::Cow::Borrowed(raster.data.as_slice());
	let mut remaining = operations;
	for adjustment in leading_adjustments {
		adjustment.apply(colors.to_mut());
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

	// Document lengths become pixels along each of the image's axes, and offsets map through the inverse of its pixel placement
	let pixels_to_document = transform.matrix2 * DMat2::from_diagonal(size.recip());
	let document_to_pixels = pixels_to_document.inverse();
	let pixels_per_length = DVec2::new(pixels_to_document.x_axis.length(), pixels_to_document.y_axis.length()).recip();

	// A margin around the image to hold the blur and shadows cast past its edges, cut short for an image that would grow too large
	let reach: f64 = operations.iter().map(EffectOperation::reach).sum();
	let mut margin = (reach * pixels_per_length).ceil();
	let grown_size = size + 2. * margin;
	if grown_size.x * grown_size.y > MAX_PIXELS_WITH_MARGIN {
		let shrunk_size = grown_size * (MAX_PIXELS_WITH_MARGIN / (grown_size.x * grown_size.y)).sqrt();
		margin = ((shrunk_size - size) / 2.).max(DVec2::ZERO).floor();
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

	for operation in operations {
		match *operation {
			EffectOperation::Blur { radius, box_blur, gamma } => {
				let radius = radius * pixels_per_length;
				if radius.max_element() < NEGLIGIBLE_LENGTH {
					continue;
				}

				// What lies beneath blurs together with the content
				if let Some(beneath) = beneath.take() {
					filled = filled.map(|filled| composite_beneath(&filled, &beneath, &full));
					full = composite_beneath(&full, &beneath, &full);
				}

				let kernels = match box_blur {
					true => (BlurKernel::Box { radius: radius.x }, BlurKernel::Box { radius: radius.y }),
					false => (BlurKernel::Gaussian { sigma: radius.x }, BlurKernel::Gaussian { sigma: radius.y }),
				};
				full = blur(full, kernels, !gamma);
				filled = filled.map(|filled| blur(filled, kernels, !gamma));
			}
			EffectOperation::DropShadow { color, offset, blur, spread } => {
				let coverage = cast(&full, document_to_pixels * offset, blur * pixels_per_length, spread * pixels_per_length, false);
				beneath = Some(accumulate(beneath, &coverage, color));
			}
			EffectOperation::OuterGlow { color, blur, spread } => {
				let coverage = cast(&full, DVec2::ZERO, blur * pixels_per_length, spread * pixels_per_length, false);
				beneath = Some(accumulate(beneath, &coverage, color));
			}
			EffectOperation::InnerShadow { color, offset, blur, spread } => {
				let coverage = cast(&full, document_to_pixels * offset, blur * pixels_per_length, spread * pixels_per_length, true);
				filled = filled.map(|filled| composite_within(&filled, &coverage, &full, color));
				full = composite_within(&full, &coverage, &full, color);
			}
			EffectOperation::InnerGlow { color, blur, choke, source } => {
				let mut coverage = cast(&full, DVec2::ZERO, blur * pixels_per_length, choke * pixels_per_length, true);
				if source == GlowSource::Center {
					coverage = coverage.map(|coverage| 1. - coverage);
				}
				filled = filled.map(|filled| composite_within(&filled, &coverage, &full, color));
				full = composite_within(&full, &coverage, &full, color);
			}
			EffectOperation::Satin { color, offset, blur, invert } => {
				let offset = document_to_pixels * offset;
				let forward = cast(&full, offset, blur * pixels_per_length, DVec2::ZERO, false);
				let backward = cast(&full, -offset, blur * pixels_per_length, DVec2::ZERO, false);
				let coverage = forward.zip(&backward, |forward, backward| satin_coverage(forward, backward, invert));

				filled = filled.map(|filled| composite_within(&filled, &coverage, &full, color));
				full = composite_within(&full, &coverage, &full, color);
			}
			EffectOperation::Outline { color, weight, align } => {
				// Straight-line distances can't stretch along one axis, so an image scaled unevenly takes the mean of its two scales
				let pixels_per_length = (pixels_per_length.x * pixels_per_length.y).sqrt();
				let (outside, inside) = outline_extents(weight * pixels_per_length, align);
				let silhouette = full.map(|[_, _, _, alpha]| alpha);

				if outside >= NEGLIGIBLE_LENGTH {
					beneath = Some(accumulate(beneath, &silhouette.edge_band(outside, false), color));
				}
				if inside >= NEGLIGIBLE_LENGTH {
					let coverage = silhouette.edge_band(inside, true);
					filled = filled.map(|filled| composite_within(&filled, &coverage, &full, color));
					full = composite_within(&full, &coverage, &full, color);
				}
			}
			EffectOperation::Adjustment(ref adjustment) => {
				full = adjust(&full, adjustment);
				filled = filled.map(|filled| adjust(&filled, adjustment));
			}
		}
	}

	let result = match (filled, beneath) {
		(filled, Some(beneath)) => composite_beneath(filled.as_ref().unwrap_or(&full), &beneath, &full),
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

/// Adjusts premultiplied gamma-encoded pixels by way of their straight colors, leaving fully transparent pixels empty since they keep no color to adjust.
fn adjust(plane: &Plane<[f32; 4]>, adjustment: &Adjustment) -> Plane<[f32; 4]> {
	let mut colors: Vec<Color> = plane.data.iter().copied().map(straight_linear_color).collect();
	adjustment.apply(&mut colors);

	let data = plane
		.data
		.iter()
		.zip(colors)
		.map(|(pixel, color)| if pixel[3] <= 0. { [0.; 4] } else { premultiplied_gamma_channels(color) })
		.collect();
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

/// The coverage of the content's silhouette (or of the area outside it, if `inverse`), cast by `offset`, grown by `spread` and softened by a Gaussian of deviation `blur`, all in pixels along each axis.
fn cast(content: &Plane<[f32; 4]>, offset: DVec2, blur: DVec2, spread: DVec2, inverse: bool) -> Plane<f32> {
	let mut coverage = content.map(|[_, _, _, alpha]| alpha).shifted(offset.x, offset.y);
	if inverse {
		coverage = coverage.map(|alpha| 1. - alpha);
	}

	if spread.max_element() >= NEGLIGIBLE_LENGTH {
		coverage = coverage.dilated(spread.x, spread.y);
	}
	if blur.max_element() >= NEGLIGIBLE_LENGTH {
		coverage = coverage.blurred(BlurKernel::Gaussian { sigma: blur.x }, BlurKernel::Gaussian { sigma: blur.y });
	}
	coverage
}

/// Stacks a silhouette's coverage, tinted `color`, over what lies `beneath` the content so far.
fn accumulate(beneath: Option<Plane<[f32; 4]>>, coverage: &Plane<f32>, color: Color) -> Plane<[f32; 4]> {
	let color = premultiplied_gamma_channels(color);
	let tinted = coverage.map(|coverage| color.map(|channel| channel * coverage));

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

/// The image with the shadows and glows `beneath` placed under it, knocked out by the full-strength content.
fn composite_beneath(image: &Plane<[f32; 4]>, beneath: &Plane<[f32; 4]>, full_content: &Plane<[f32; 4]>) -> Plane<[f32; 4]> {
	let knocked_out = beneath.zip(full_content, |beneath, content| beneath.map(|channel| channel * (1. - content[3])));
	image.zip(&knocked_out, |image, beneath| std::array::from_fn(|channel| image[channel] + beneath[channel]))
}

/// Satin's shading where the two casts of the silhouette cover `forward` and `backward`: how much one covers without the other, or how much they agree if `invert`.
fn satin_coverage(forward: f32, backward: f32, invert: bool) -> f32 {
	let difference = (forward - backward).abs();
	if invert { 1. - difference } else { difference }
}
