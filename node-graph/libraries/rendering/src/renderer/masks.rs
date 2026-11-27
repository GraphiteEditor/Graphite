//! SVG masks of content by another graphic: SVG's own `<mask>` for the masks before and after the other effects, and filter primitives for those between them.
//! Both forms run the same steps as the GPU's passes: the coverage, its feather, then its density. Also the masked radius blur's filter primitives.

use super::{ENDLESS_EXTENT, Render, RenderParams, RenderSvgSegmentList, SvgFilterPrimitives, SvgRender, drawn_bounding_box, nested_effects_reach, svg_drawn_paint, svg_effects_region};
use core_types::ATTR_TRANSFORM;
use core_types::bounds::RenderBoundingBox;
use core_types::list::{ATTR_APPEARANCE, ATTR_EFFECTS, Item, List};
use core_types::uuid::generate_uuid;
use glam::{DAffine2, DVec2};
use graphic_types::effects::{EffectOperation, Effects, is_unplaced_paint, spans_along_axes};
use graphic_types::masks::{MaskMode, mask_density_line, mask_feather_sigma, masked_radius_blur_weight};
use graphic_types::{Appearance, Graphic};
use raster_kernels::effect_kernels::NEGLIGIBLE_LENGTH;
use std::fmt::Write;
use std::ops::Range;

/// One mask row for SVG's own mask: its settings, its graphic's markup, and the region of the user space its strength is worked out over.
struct NativeMask<'a> {
	mode: MaskMode,
	density: f64,
	/// The feather's standard deviation along each axis of the user space.
	sigma: DVec2,
	invert: bool,
	markup: &'a str,
	region: [DVec2; 2],
}

/// The ranges of the effects' masks that lead all their other effects and that trail them, which SVG's own masks carry out around the filter.
pub(super) fn native_masks(effects: &Effects) -> (Range<usize>, Range<usize>) {
	let is_mask = |operation: &EffectOperation| matches!(operation, EffectOperation::Mask { .. });
	let operations: Vec<EffectOperation> = effects.operations().collect();

	let leading = operations.iter().take_while(|operation| is_mask(operation)).count();
	let trailing = operations[leading..].iter().rev().take_while(|operation| is_mask(operation)).count();
	(0..leading, operations.len() - trailing..operations.len())
}

/// The standard deviation along each axis of the user space of the Gaussian softening a mask by its `feather`, or zero for a feather too small to show, judged in the document as the GPU judges it in pixels.
pub(super) fn svg_feather_sigma(feather: f64, length_scale: DVec2) -> DVec2 {
	let sigma = mask_feather_sigma(feather) * length_scale;
	if sigma.max_element() < NEGLIGIBLE_LENGTH { DVec2::ZERO } else { sigma }
}

/// Whether the graphic, anything nested in it, or any paint its appearance fills or strokes them with, carries effects.
pub(super) fn holds_effects(graphic: &Graphic) -> bool {
	fn row_holds_effects(effects: Option<&Effects>, appearance: Option<&Appearance>) -> bool {
		effects.is_some() || appearance.is_some_and(|appearance| appearance.covers_with_paints().any(|(_, paint)| paint.is_some_and(holds_effects)))
	}
	fn list_holds_effects<T>(list: &List<T>, nested: impl Fn(&T) -> bool) -> bool {
		(0..list.len()).any(|index| row_holds_effects(list.attribute(ATTR_EFFECTS, index), list.attribute(ATTR_APPEARANCE, index)) || list.element(index).is_some_and(&nested))
	}
	fn item_holds_effects<T>(item: &Item<T>) -> bool {
		row_holds_effects(item.attribute(ATTR_EFFECTS), item.attribute(ATTR_APPEARANCE))
	}

	match graphic {
		Graphic::None(_) | Graphic::NoneList(_) | Graphic::StrokeList(_) => false,
		Graphic::Graphic(item) => item_holds_effects(item) || holds_effects(item.element()),
		Graphic::Vector(item) => item_holds_effects(item),
		Graphic::RasterCPU(item) => item_holds_effects(item),
		Graphic::RasterGPU(item) => item_holds_effects(item),
		Graphic::Color(item) => item_holds_effects(item),
		Graphic::Gradient(item) => item_holds_effects(item),
		Graphic::Generator(item) => item_holds_effects(item),
		Graphic::Text(item) => item_holds_effects(item),
		Graphic::GraphicList(list) => list_holds_effects(list, holds_effects),
		Graphic::VectorList(list) => list_holds_effects(list, |_| false),
		Graphic::RasterCPUList(list) => list_holds_effects(list, |_| false),
		Graphic::RasterGPUList(list) => list_holds_effects(list, |_| false),
		Graphic::ColorList(list) => list_holds_effects(list, |_| false),
		Graphic::GradientList(list) => list_holds_effects(list, |_| false),
		Graphic::GeneratorList(list) => list_holds_effects(list, |_| false),
		Graphic::TextList(list) => list_holds_effects(list, |_| false),
	}
}

/// Whether SVG draws a blur's `radius_mask` as markup in its filter, keeping the blur resolution-independent, rather than taking its luminance as the GPU's map.
/// Markup embedded as an image reaches no maps of the radius mask's own effects, and can't continue an unplaced image's edge pixels past the content as the GPU does.
pub(super) fn radius_mask_draws_as_markup(radius_mask: &Graphic) -> bool {
	let unplaced_image = matches!(radius_mask, Graphic::RasterCPU(_) | Graphic::RasterGPU(_)) && is_unplaced_paint(radius_mask);
	!unplaced_image && !holds_effects(radius_mask)
}

/// A mask's graphic placed by its content's `placement`, which it follows.
pub(super) fn placed(mask: &Graphic, placement: DAffine2) -> Graphic {
	Graphic::Graphic(Box::new(Item::new_from_element(mask.clone()).with_attribute(ATTR_TRANSFORM, placement)))
}

/// Writes into `svg_defs` the masks for the effects in each of the `ranges`, given the markup of the graphics the effects draw with in order, and the content's `bounds` in the user space, which is the effects' own.
/// Returns the reference to each range's mask, which shows the content as much as all its masks together do.
pub(super) fn write_native_masks(svg_defs: &mut String, effects: &Effects, ranges: [Range<usize>; 2], paints: &[String], bounds: [DVec2; 2]) -> [Option<String>; 2] {
	let length_scale = DVec2::ONE;
	let region = svg_effects_region(effects, bounds, length_scale);

	// Each mask row by its place in the effects, with its graphic's markup
	let mut paints = paints.iter();
	let masks: Vec<(usize, NativeMask)> = effects
		.operations()
		.enumerate()
		.filter_map(|(index, operation)| {
			let paint = svg_drawn_paint(&operation).and_then(|_| paints.next());
			let EffectOperation::Mask { mode, density, feather, invert, .. } = operation else { return None };
			let sigma = svg_feather_sigma(feather, length_scale);
			Some((
				index,
				NativeMask {
					mode,
					density,
					sigma,
					invert,
					markup: paint?,
					region,
				},
			))
		})
		.collect();

	ranges.map(|range| {
		let in_range: Vec<&NativeMask> = masks.iter().filter(|(index, _)| range.contains(index)).map(|(_, mask)| mask).collect();
		write_combined_mask(svg_defs, &in_range, false)
	})
}

/// The extent in the user space that the leading masks of content without edges (like a color or gradient filling everything) confine it to, with the reach of their feathers, given the content's `placement` there.
/// `None` unless the first effect is such a mask: not inverted, at full density, with a graphic of bounded extent. Each such mask after it narrows the extent.
pub(super) fn masked_extent(effects: &Effects, placement: DAffine2) -> Option<[DVec2; 2]> {
	// TODO: Masks that cut holes (like an inverted one) leave the content endless, so both renderers skip its other effects (blurs, shadows, feathers) at the holes' edges.
	// Carry those effects out over the area in view when masks give the content edges without confining it.
	let mut extent: Option<[DVec2; 2]> = None;
	for (operation, length_scale) in effects.operations().zip(effects.length_spans(placement)) {
		let EffectOperation::Mask { mask, density, feather, invert, .. } = operation else { break };
		let RenderBoundingBox::Rectangle([min, max]) = drawn_bounding_box(&mask, placement) else { break };
		if invert || density < 100. {
			break;
		}

		let reach = 3. * svg_feather_sigma(feather, length_scale) + nested_effects_reach(&mask, placement, EffectOperation::painted_reach);
		let [min, max] = [min - reach, max + reach];
		extent = Some(match extent {
			// Masks that don't overlap leave an empty extent at the first one's corner
			Some([extent_min, extent_max]) => {
				let narrowed_min = extent_min.max(min);
				[narrowed_min, extent_max.min(max).max(narrowed_min)]
			}
			None => [min, max],
		});
	}
	extent
}

/// Writes into the defs the mask carrying out the mask rows among the effects of content without edges, like a color or gradient filling everything, placed by `placement`, or `None` if there are none.
/// This is for masks that leave the content without edges, so SVG leaves the other effects undone as it does for such content. Each mask's strength is worked out where its graphic reaches.
pub(super) fn write_endless_content_mask(render: &mut SvgRender, effects: &Effects, placement: DAffine2, render_params: &RenderParams) -> Option<String> {
	// The mask graphics' markup lies in the document, where the maps and images of their own effects reach it
	let live_params = RenderParams {
		inherited_appearance: None,
		embeds_images: true,
		..render_params.clone()
	};
	let mut rows = Vec::new();
	for (operation, length_scale) in effects.operations().zip(effects.length_spans(placement)) {
		let EffectOperation::Mask {
			ref mask,
			mode,
			density,
			feather,
			invert,
		} = operation
		else {
			continue;
		};
		let feather_reach = operation.reach() * spans_along_axes(render.transform.matrix2).max_element() * length_scale.max_element();
		let mask = placed(mask, placement);

		// The maps of the mask graphic's own effects cover past the view as far as its feather and the groups around the content read it
		let mut mask_render = SvgRender::new();
		mask_render.transform = render.transform;
		mask_render.ancestors_reach = render.ancestors_reach + feather_reach;
		mask.render_svg(&mut mask_render, &live_params);
		render.effect_maps.append(&mut mask_render.effect_maps);
		render.deferred_images.append(&mut mask_render.deferred_images);
		let markup = format!("<defs>{}</defs>{}", mask_render.svg_defs, mask_render.svg.to_svg_string());

		let sigma = svg_feather_sigma(feather, length_scale);
		let region = match drawn_bounding_box(&mask, DAffine2::IDENTITY) {
			RenderBoundingBox::Rectangle([min, max]) => {
				let reach = 3. * sigma + nested_effects_reach(&mask, DAffine2::IDENTITY, EffectOperation::painted_reach);
				[min - reach, max + reach]
			}
			_ => [DVec2::splat(-ENDLESS_EXTENT), DVec2::splat(ENDLESS_EXTENT)],
		};
		rows.push((mode, density, sigma, invert, markup, region));
	}

	let masks: Vec<NativeMask> = rows
		.iter()
		.map(|(mode, density, sigma, invert, markup, region)| NativeMask {
			mode: *mode,
			density: *density,
			sigma: *sigma,
			invert: *invert,
			markup,
			region: *region,
		})
		.collect();
	write_combined_mask(&mut render.svg_defs, &masks.iter().collect::<Vec<_>>(), true)
}

/// Writes into `svg_defs` one SVG mask showing content as much as all the `masks` together do, returning its reference, or `None` for no masks.
/// An `endless` mask spans all content without edges, and past each mask's region shows it as much as that mask does where its graphic is absent. Others end with their regions.
fn write_combined_mask(svg_defs: &mut String, masks: &[&NativeMask], endless: bool) -> Option<String> {
	let placement = |[min, max]: [DVec2; 2]| format!(r#"x="{}" y="{}" width="{}" height="{}""#, min.x, min.y, max.x - min.x, max.y - min.y);
	let endless_square = format!("M{ENDLESS_EXTENT} {ENDLESS_EXTENT}H-{ENDLESS_EXTENT}V-{ENDLESS_EXTENT}H{ENDLESS_EXTENT}Z");

	// Each mask's content is masked by the next one's, so the outermost shows the content by the product of them all
	let mut combined: Option<String> = None;
	for mask in masks.iter().rev() {
		// Past its region, an endless mask that isn't inverted shows the content by one less its density, drawn everywhere beneath its plain coverage so nothing overlaps along the region's edge.
		// An inverted one shows all of the content past its region, where its own strength is whole too.
		let beneath = endless && !mask.invert && mask.density < 100.;
		let density = if endless && !mask.invert { 100. } else { mask.density };
		let mut content = match beneath {
			true => format!(r#"<path d="{endless_square}" fill-opacity="{}" />"#, 1. - (mask.density / 100.).clamp(0., 1.)),
			false => String::new(),
		};

		let mut primitives = SvgFilterPrimitives::default();
		primitives.mask_strength("SourceGraphic", mask.mode, density, mask.sigma, mask.invert);
		let strength_id = format!("mask-strength-{}", generate_uuid());
		let _ = write!(
			svg_defs,
			r#"<filter id="{strength_id}" filterUnits="userSpaceOnUse" {} color-interpolation-filters="sRGB">{}</filter>"#,
			placement(mask.region),
			primitives.markup
		);
		let _ = write!(content, r#"<g filter="url(#{strength_id})">{}</g>"#, mask.markup);

		if endless && mask.invert {
			let [min, max] = mask.region;
			let _ = write!(content, r#"<path d="{endless_square}M{} {}H{}V{}H{}Z" fill-rule="evenodd" />"#, min.x, min.y, max.x, max.y, min.x);
		}

		if let Some(inner) = &combined {
			content = format!(r#"<g mask="{inner}">{content}</g>"#);
		}
		let mask_region = if endless { [DVec2::splat(-ENDLESS_EXTENT), DVec2::splat(ENDLESS_EXTENT)] } else { mask.region };
		let mask_id = format!("mask-{}", generate_uuid());
		let _ = write!(
			svg_defs,
			r#"<mask id="{mask_id}" mask-type="alpha" maskUnits="userSpaceOnUse" {}>{content}</mask>"#,
			placement(mask_region)
		);
		combined = Some(format!("url(#{mask_id})"));
	}
	combined
}

impl SvgFilterPrimitives {
	/// Each level's parts of a masked radius blur's interpolation, in the alpha of its result, from the radius mask's 8-bit `luminance` (in alpha) for the `levels` of its radius, matching `masked_radius_blur_interpolation`.
	pub(super) fn masked_radius_blur_weights(&mut self, luminance: &str, levels: &[f64]) -> Vec<String> {
		(0..levels.len())
			.map(|level| {
				// A quarter of a level past each whole one lands on it whether a browser rounds or truncates, and none stays exactly none for one that doesn't
				let table = (0..=255_u8)
					.map(|code| {
						let weight = masked_radius_blur_weight(level, code, levels) as f64;
						let biased = if weight > 0. { weight + 0.25 } else { 0. };
						format!("{:.6}", (biased / 255.).min(1.))
					})
					.collect::<Vec<_>>()
					.join(" ");
				self.push("feComponentTransfer", &format!(r#"in="{luminance}""#), &format!(r#"<feFuncA type="table" tableValues="{table}" />"#))
			})
			.collect()
	}

	/// The `image` blurred by each of a masked radius blur's `levels` (in user units) weighed by its `weights` and summed, matching the GPU's `masked_radius_blur`.
	/// The levels are boxes if `box_blur` and Gaussians otherwise, blurring in gamma if `gamma` and in linear light otherwise.
	pub(super) fn masked_radius_blur(&mut self, image: &str, weights: &[String], levels: &[f64], box_blur: bool, gamma: bool) -> String {
		let space = if gamma { "sRGB" } else { "linearRGB" };

		let mut sum: Option<String> = None;
		for (&level, weight) in levels.iter().zip(weights) {
			// SVG has no box blur, so a box becomes the Gaussian of the same variance, as for an even blur
			let sigma = if box_blur { (level * (level + 1.) / 3.).sqrt() } else { level };
			let blurred = match level >= NEGLIGIBLE_LENGTH {
				true => self.push("feGaussianBlur", &format!(r#"in="{image}" stdDeviation="{sigma} {sigma}" color-interpolation-filters="{space}""#), ""),
				false => image.to_string(),
			};
			// Weighed and summed in gamma, the filter's own space, on purpose (see `masked_radius_blur_interpolation`)
			let weighted = self.push("feComposite", &format!(r#"in="{blurred}" in2="{weight}" operator="in""#), "");

			sum = Some(match sum {
				Some(sum) => self.add(&sum, &weighted),
				None => weighted,
			});
		}
		sum.unwrap_or_else(|| image.to_string())
	}

	/// The `mode`'s coverage of a mask graphic's `image`, in the alpha of the result, matching the GPU's `mask_coverage`.
	pub(super) fn mask_coverage(&mut self, image: &str, mode: MaskMode) -> String {
		match mode {
			// Over black, the luminance of the opaque result is that of the premultiplied channels
			MaskMode::Luminance => {
				let black = self.push("feFlood", r#"flood-color="black""#, "");
				let opaque = self.push("feComposite", &format!(r#"in="{image}" in2="{black}" operator="over""#), "");
				self.push("feColorMatrix", &format!(r#"in="{opaque}" type="luminanceToAlpha""#), "")
			}
			MaskMode::Alpha => image.to_string(),
		}
	}

	/// How much the mask graphic's `image` shows the content, in the alpha of the result, matching the GPU's mask passes.
	/// Its coverage is softened by a Gaussian of standard deviation `sigma` along each axis of the user space (none at zero), then mapped through its `density` percentage, flipped if `invert`.
	pub(super) fn mask_strength(&mut self, image: &str, mode: MaskMode, density: f64, sigma: DVec2, invert: bool) -> String {
		let mut coverage = self.mask_coverage(image, mode);

		if sigma != DVec2::ZERO {
			coverage = self.push("feGaussianBlur", &format!(r#"in="{coverage}" stdDeviation="{} {}""#, sigma.x, sigma.y), "");
		}

		let (slope, intercept) = mask_density_line(density, invert);
		self.push(
			"feComponentTransfer",
			&format!(r#"in="{coverage}""#),
			&format!(r#"<feFuncA type="linear" slope="{slope}" intercept="{intercept}" />"#),
		)
	}
}
