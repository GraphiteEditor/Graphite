//! Carries out the adjustments among a color's or gradient's effects, for the nodes and paints that read the value rather than render it.
//! A value read on its own has no pixels for the image operations among the effects (like blurs and shadows) to act on, so those are skipped.

use crate::adjustments::{PreparedAdjustment, adjust_color};
use crate::blending::overlay_paint;
use core_types::Color;
use core_types::graphene_hash::CacheHash;
use core_types::list::{ATTR_EFFECTS, ATTR_GRADIENT_HUE_DIRECTION, ATTR_GRADIENT_INTERPOLATION, ATTR_GRADIENT_SPACE, Item};
use graphic_types::adjustment::Adjustment;
use graphic_types::effects::{EffectOperation, Effects};
use std::hash::{DefaultHasher, Hasher};
use std::sync::{Mutex, PoisonError};
use vector_types::{Gradient, GradientInterpolation, GradientSettings, GradientSpace};

/// How many adjusted gradients stay cached after the renders that used them, since paints read the same ones every render.
const MAX_CACHED_GRADIENTS: usize = 32;

/// The gradients adjusted most recently, least recent first, keyed by the hash of what they were adjusted from.
static ADJUSTED_GRADIENTS: Mutex<Vec<(u64, Gradient, GradientSettings)>> = Mutex::new(Vec::new());

/// The adjustments among the effects, in order.
pub fn adjustments(effects: &Effects) -> Vec<Adjustment> {
	effects
		.operations()
		.filter_map(|operation| match operation {
			EffectOperation::Adjustment(adjustment) => Some(adjustment),
			_ => None,
		})
		.collect()
}

/// The color with `adjustments` carried out on it in order.
pub fn adjusted_color(color: Color, adjustments: &[Adjustment]) -> Color {
	adjustments.iter().fold(color, |color, adjustment| adjust_color(adjustment, color))
}

/// The color an endless paint of `color` shows with `adjustments` carried out over it and its `fill` fading it, though not the paint overlays painted over it.
/// Without edges, each paint overlay blends with the paint itself, which is exact where the paint is opaque.
pub fn filled_color(color: Color, adjustments: &[Adjustment], fill: f32) -> Color {
	if fill >= 1. {
		return adjusted_color(color, adjustments);
	}
	fill_through(color, adjustments, fill, |index, color| adjust_color(&adjustments[index], color))
}

/// Carries out `adjustments` over a paint faded by `fill` as [`filled_color`] does, with `adjust` carrying out each one besides a paint overlay, given its index.
fn fill_through(color: Color, adjustments: &[Adjustment], fill: f32, adjust: impl Fn(usize, Color) -> Color) -> Color {
	// The color at full strength, which each paint overlay blends with, and the color the fill fades, which it paints over at full strength
	let (mut full, mut filled) = (color, color.with_alpha(color.a() * fill));
	for (index, adjustment) in adjustments.iter().enumerate() {
		let Adjustment::ColorOverlay { color: paint, blend_mode, opacity } = *adjustment else {
			(full, filled) = (adjust(index, full), adjust(index, filled));
			continue;
		};

		let strength = paint.a() * (opacity / 100.) as f32;
		let [red, green, blue, _] = overlay_paint(full, paint.with_alpha(1.), blend_mode, 1.).to_gamma_srgb_channels();
		let [under_red, under_green, under_blue, under_alpha] = filled.to_gamma_srgb_channels();

		// Premultiplied, the blended paint covers the full-strength content's alpha by its strength, over what it leaves of the faded color
		let (painted, kept) = (strength * full.a(), under_alpha * (1. - strength));
		let alpha = painted + kept;
		let mix = |over: f32, under: f32| if alpha > 0. { (over * painted + under * kept) / alpha } else { 0. };
		filled = Color::from_gamma_srgb_channels(mix(red, under_red), mix(green, under_green), mix(blue, under_blue), alpha);
		full = overlay_paint(full, paint, blend_mode, (opacity / 100.) as f32);
	}
	filled
}

/// Carries out `adjustments` in order on any number of colors with the `fill` fading them, with each one's kernel prepared once.
fn prepared(adjustments: &[Adjustment], fill: f32) -> Box<dyn Fn(Color) -> Color> {
	let kernels: Vec<PreparedAdjustment> = adjustments.iter().map(PreparedAdjustment::new).collect();
	if fill >= 1. {
		return Box::new(move |color| kernels.iter().fold(color, |color, kernel| kernel.adjust(color)));
	}

	let adjustments = adjustments.to_vec();
	Box::new(move |color| fill_through(color, &adjustments, fill, |index, color| kernels[index].adjust(color)))
}

/// The gradient with `adjustments` carried out along its ramp, with the settings to read the result with, or `None` if there are no adjustments.
/// A `fill` below 1 fades the ramp but not the paint overlays over it, as for an endless gradient drawn with its fill opacity.
/// A stepped ramp holds each stop's color, so adjusting the stops is exact. Elsewhere an adjustment can bend the ramp between stops,
/// so the adjusted ramp is resampled into stops that interpolate linearly in gamma.
pub fn adjusted_gradient(gradient: &Gradient, settings: GradientSettings, adjustments: &[Adjustment], fill: f32) -> Option<(Gradient, GradientSettings)> {
	if adjustments.is_empty() {
		return None;
	}

	// A gradient without stops reads as black, so its adjusted ramp is one stop of the adjusted black
	if gradient.is_empty() {
		return Some((Gradient::from(vec![prepared(adjustments, fill)(Color::BLACK)]), settings));
	}

	if settings.interpolation == GradientInterpolation::Stepped {
		let adjust = prepared(adjustments, fill);
		return Some((gradient.map_colors(|color| adjust(*color)), settings));
	}

	// Resampling takes thousands of evaluations for a steep adjustment, so recent results are reused
	let mut hasher = DefaultHasher::new();
	gradient.cache_hash(&mut hasher);
	settings.cache_hash(&mut hasher);
	adjustments.iter().for_each(|adjustment| adjustment.cache_hash(&mut hasher));
	hasher.write_u32(fill.to_bits());
	let key = hasher.finish();

	// A lock poisoned by a panic elsewhere still guards a usable cache
	let mut cached = ADJUSTED_GRADIENTS.lock().unwrap_or_else(PoisonError::into_inner);
	if let Some(index) = cached.iter().position(|(cached_key, ..)| *cached_key == key) {
		let entry = cached.remove(index);
		let result = (entry.1.clone(), entry.2);
		cached.push(entry);
		return Some(result);
	}

	let resampled = gradient.map_ramp(settings, prepared(adjustments, fill));
	let resampled_settings = GradientSettings {
		space: GradientSpace::RgbGamma,
		interpolation: GradientInterpolation::Linear,
		hue_direction: Default::default(),
		..settings
	};

	cached.push((key, resampled.clone(), resampled_settings));
	let excess = cached.len().saturating_sub(MAX_CACHED_GRADIENTS);
	cached.drain(..excess);

	Some((resampled, resampled_settings))
}

/// The color with the adjustments among its effects carried out and the effects consumed, for nodes that read the color.
/// A color without the effects attribute is returned as it is.
pub fn evaluate_color_effects(item: Item<Color>) -> Item<Color> {
	let Some(adjustments) = item.attribute::<Effects>(ATTR_EFFECTS).map(adjustments) else {
		return item;
	};

	let mut item = item;
	*item.element_mut() = adjusted_color(*item.element(), &adjustments);
	item.remove_attribute::<Effects>(ATTR_EFFECTS);
	item
}

/// The gradient with the adjustments among its effects carried out along its ramp and the effects consumed, for nodes that read its colors.
/// A gradient without the effects attribute is returned as it is.
pub fn evaluate_gradient_effects(item: Item<Gradient>) -> Item<Gradient> {
	let Some(adjustments) = item.attribute::<Effects>(ATTR_EFFECTS).map(adjustments) else {
		return item;
	};
	let settings = GradientSettings::from_item_attributes(&item);

	let mut item = item;
	let settings = match adjusted_gradient(item.element(), settings, &adjustments, 1.) {
		Some((gradient, adjusted_settings)) => {
			*item.element_mut() = gradient;
			adjusted_settings
		}
		None => settings,
	};

	// Which attributes come out depends only on whether the effects attribute came in, not on what it holds
	item.remove_attribute::<Effects>(ATTR_EFFECTS);
	item.set_attribute(ATTR_GRADIENT_SPACE, settings.space);
	item.set_attribute(ATTR_GRADIENT_INTERPOLATION, settings.interpolation);
	item.set_attribute(ATTR_GRADIENT_HUE_DIRECTION, settings.hue_direction);
	item
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::blending::BlendMode;

	fn gamma(color: Color) -> [f32; 4] {
		color.to_gamma_srgb_channels()
	}

	fn close(actual: [f32; 4], expected: [f32; 4]) -> bool {
		actual.iter().zip(expected).all(|(actual, expected)| (actual - expected).abs() < 1e-4)
	}

	#[test]
	fn paint_overlays_mix_in_gamma_unfaded_by_the_fill() {
		let half_white = [Adjustment::ColorOverlay {
			color: Color::WHITE,
			blend_mode: BlendMode::Normal,
			opacity: 50.,
		}];

		// Half of white over black is the middle gray of gamma-encoded channels
		assert!(close(gamma(filled_color(Color::BLACK, &half_white, 1.)), [0.5, 0.5, 0.5, 1.]));

		// With no fill, the black is gone, but the overlay still paints at half strength
		assert!(close(gamma(filled_color(Color::BLACK, &half_white, 0.)), [1., 1., 1., 0.5]));

		// At half fill, the overlay covers half and the faded black shows through a quarter
		assert!(close(gamma(filled_color(Color::BLACK, &half_white, 0.5)), [2. / 3., 2. / 3., 2. / 3., 0.75]));
	}

	#[test]
	fn a_gradient_without_stops_adjusts_the_black_it_reads_as() {
		let empty = Gradient::from(Vec::<Color>::new());
		for interpolation in [GradientInterpolation::default(), GradientInterpolation::Stepped] {
			let settings = GradientSettings { interpolation, ..Default::default() };
			let adjusted = adjusted_gradient(&empty, settings, &[Adjustment::Invert], 1.).map(|(gradient, _)| gradient);
			assert_eq!(adjusted, Some(Gradient::from(vec![Color::WHITE])), "{interpolation:?}");
		}
	}
}
