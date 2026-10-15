//! Carries out a raster's effects on the CPU at the raster's own resolution, for the nodes that read its pixels.
//! Each step mirrors the renderer's GPU passes, working on premultiplied gamma-encoded channels as they do.

use crate::Graphic;
use crate::adjustment::Adjustment;
use crate::adjustment_kernels::overlay_paint;
use crate::effects::{BevelDirection, BevelStyle, EffectOperation, Effects, GlowSource, bevel_light, bevel_smoothing, fitted_paint, outline_extents};
use crate::generator::GeneratorPattern;
use core_types::FallibleVec2Operations;
use core_types::blending::BlendMode;
use core_types::color::{linear_to_srgb, srgb_to_linear};
use core_types::list::{ATTR_EFFECTS, ATTR_GRADIENT_END_CURVATURE, ATTR_GRADIENT_FORM, ATTR_GRADIENT_START_CURVATURE, Item};
use core_types::{ATTR_OPACITY_FILL, ATTR_TRANSFORM, Color};
use glam::{DAffine2, DMat2, DVec2, DVec3};
use raster_types::effect_kernels::{BlurKernel, NEGLIGIBLE_LENGTH, Plane};
use raster_types::parallel_rows::fill_rows;
use raster_types::{CPU, Image, Raster};
use vector_types::gradient::{Gradient, GradientForm, GradientSettings, LINEAR_DIALS, RADIAL_DIALS, wrapped_dial};

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
				let pixels_per_length = (pixels_per_length.x * pixels_per_length.y).sqrt();
				let (size, soften) = (size * pixels_per_length, soften * pixels_per_length);
				if size < NEGLIGIBLE_LENGTH {
					continue;
				}

				// The surface's height, rounded by a blur, lit from the light's direction into its highlights and shadows
				let height = soften_by(bevel_height(&full.map(|[_, _, _, alpha]| alpha), size, style), bevel_smoothing(size));
				let slope = depth / 100. * size * if direction == BevelDirection::Down { -1. } else { 1. };
				let light = bevel_light(angle, altitude, document_to_pixels);

				// Their parts outside the content lie beneath it, and those inside are placed over it
				for (shadow, color) in [(false, highlight_color), (true, shadow_color)] {
					let coverage = soften_by(bevel_shading(&height, slope, light, shadow), soften);
					beneath = Some(accumulate(beneath, &coverage, color));
					filled = filled.map(|filled| composite_within(&filled, &coverage, &full, color));
					full = composite_within(&full, &coverage, &full, color);
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

/// The plane blurred by a Gaussian of deviation `sigma` pixels, or left as it is for one too small to see.
fn soften_by(plane: Plane<f32>, sigma: f64) -> Plane<f32> {
	match sigma >= NEGLIGIBLE_LENGTH {
		true => plane.blurred(BlurKernel::Gaussian { sigma }, BlurKernel::Gaussian { sigma }),
		false => plane,
	}
}

/// A bevel's height from 0 to 1, from each pixel's signed distance to the silhouette's edge (positive inside), shaped by the `style` over `size` pixels.
fn bevel_height(silhouette: &Plane<f32>, size: f64, style: BevelStyle) -> Plane<f32> {
	let to_inside = silhouette.distances_across_edge(size, false);
	let to_outside = silhouette.distances_across_edge(size, true);

	// The pixels inside the silhouette are those at no distance from it, and the edge lies about half a pixel short of the nearest pixel across it
	to_inside.zip(&to_outside, |to_inside, to_outside| {
		let signed_distance = if to_inside == 0. { to_outside - 0.5 } else { 0.5 - to_inside };
		bevel_profile(signed_distance, size as f32, style)
	})
}

/// The height from 0 to 1 at a signed distance from the edge (positive inside), rising smoothly across the `style`'s slope `size` pixels wide.
fn bevel_profile(signed_distance: f32, size: f32, style: BevelStyle) -> f32 {
	let progress = match style {
		BevelStyle::InnerBevel => signed_distance / size,
		BevelStyle::OuterBevel => (signed_distance + size) / size,
		BevelStyle::Emboss => (signed_distance + size / 2.) / size,
		BevelStyle::PillowEmboss => signed_distance.abs() / (size / 2.),
	};
	let progress = progress.clamp(0., 1.);
	progress * progress * (3. - 2. * progress)
}

/// How far the `light` brightens the bevel's surface past a flat surface's brightness, or if `shadow`, how far it falls short, from 0 to 1.
/// `slope` scales the height's gradient into the surface's steepness, and is negative for a bevel pressed into the surface. O(width × height).
fn bevel_shading(height: &Plane<f32>, slope: f64, light: DVec3, shadow: bool) -> Plane<f32> {
	let (width, rows) = (height.width as isize, height.height as isize);
	let at = |x: isize, y: isize| height.data[(y.clamp(0, rows - 1) * width + x.clamp(0, width - 1)) as usize];
	let (slope, light) = (slope as f32, light.as_vec3());

	let mut shading = Plane::new(height.width, height.height);
	fill_rows(&mut shading.data, height.width, |y, row| {
		for (x, value) in row.iter_mut().enumerate() {
			let (x, y) = (x as isize, y as isize);
			let gradient = glam::Vec2::new(at(x + 1, y) - at(x - 1, y), at(x, y + 1) - at(x, y - 1)) / 2.;
			let shade = (-slope * gradient).extend(1.).normalize().dot(light);

			*value = match shadow {
				true => (light.z - shade) / light.z.max(1e-4),
				false => (shade - light.z) / (1. - light.z).max(1e-4),
			}
			.clamp(0., 1.);
		}
	});
	shading
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

/// Samples the paint's color at points in the space its placement maps into, for the paints the CPU can draw: colors, images, generators, and linear or radial gradients.
fn paint_sampler(paint: &Graphic) -> Option<Box<dyn Fn(DVec2) -> Color + Sync + '_>> {
	let inverse = |placement: DAffine2| (placement.matrix2.determinant() != 0.).then(|| placement.inverse());

	match paint {
		Graphic::Color(color) => {
			let color = *color.element();
			Some(Box::new(move |_| color))
		}
		Graphic::RasterCPU(item) => {
			let image = item.element();
			let to_pixels = DAffine2::from_scale(DVec2::new(image.width as f64, image.height as f64)) * inverse(item.attribute_cloned_or_default(ATTR_TRANSFORM))?;
			Some(Box::new(move |point| image.sample_bilinear(to_pixels.transform_point2(point))))
		}
		Graphic::Generator(item) => {
			let to_pattern = inverse(item.attribute_cloned_or_default(ATTR_TRANSFORM))?;
			let sample = GeneratorPattern::of_item(item).sampler();
			Some(Box::new(move |point| sample(to_pattern.transform_point2(point))))
		}
		Graphic::Gradient(item) => {
			let position = gradient_position(item)?;
			let evaluator = item.element().evaluator(GradientSettings::from_item_attributes(item));
			Some(Box::new(move |point| evaluator.evaluate(position(point))))
		}
		_ => None,
	}
}

/// How far along a linear or radial gradient each point lies, or `None` for the other shapes its curvature dials make, which only the renderers draw.
fn gradient_position(item: &Item<Gradient>) -> Option<Box<dyn Fn(DVec2) -> f64 + Sync>> {
	let placement: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let preset = item.attribute_cloned_or_default::<GradientForm>(ATTR_GRADIENT_FORM).dials();
	let dial = |key: &str, preset: f64| wrapped_dial(item.attribute_cloned_or(key, preset), preset);

	match [dial(ATTR_GRADIENT_START_CURVATURE, preset[0]), dial(ATTR_GRADIENT_END_CURVATURE, preset[1])] {
		// Measured along the line across the bands, which stay parallel to the placement's sheared y axis as the renderers draw them
		LINEAR_DIALS => {
			let axis = placement.matrix2.x_axis;
			let line = axis.try_project_onto(placement.matrix2.y_axis.perp()).unwrap_or(axis);
			let origin = placement.translation;
			(line.length_squared() > 0.).then(|| Box::new(move |point: DVec2| (point - origin).dot(line) / line.length_squared()) as Box<dyn Fn(DVec2) -> f64 + Sync>)
		}
		RADIAL_DIALS => {
			let to_unit = (placement.matrix2.determinant() != 0.).then(|| placement.inverse())?;
			Some(Box::new(move |point| to_unit.transform_point2(point).length()))
		}
		_ => None,
	}
}
