//! Carries out the adjustments among a color's or gradient's effects, for the nodes and paints that read the value rather than render it.
//! A value read on its own has no pixels for the image operations among the effects (like blurs and shadows) to act on, so those are skipped.

use crate::adjustments::adjust_color;
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

/// The gradient with `adjustments` carried out along its ramp, with the settings to read the result with, or `None` if there are no adjustments.
/// A stepped ramp holds each stop's color, so adjusting the stops is exact. Elsewhere an adjustment can bend the ramp between stops,
/// so the adjusted ramp is resampled into stops that interpolate linearly in gamma.
pub fn adjusted_gradient(gradient: &Gradient, settings: GradientSettings, adjustments: &[Adjustment]) -> Option<(Gradient, GradientSettings)> {
	if adjustments.is_empty() {
		return None;
	}
	let adjust = |color: Color| adjusted_color(color, adjustments);

	if settings.interpolation == GradientInterpolation::Stepped {
		return Some((gradient.map_colors(|color| adjust(*color)), settings));
	}

	// Resampling takes thousands of evaluations for a steep adjustment, so recent results are reused
	let mut hasher = DefaultHasher::new();
	gradient.cache_hash(&mut hasher);
	settings.cache_hash(&mut hasher);
	adjustments.iter().for_each(|adjustment| adjustment.cache_hash(&mut hasher));
	let key = hasher.finish();

	// A lock poisoned by a panic elsewhere still guards a usable cache
	let mut cached = ADJUSTED_GRADIENTS.lock().unwrap_or_else(PoisonError::into_inner);
	if let Some(index) = cached.iter().position(|(cached_key, ..)| *cached_key == key) {
		let entry = cached.remove(index);
		let result = (entry.1.clone(), entry.2);
		cached.push(entry);
		return Some(result);
	}

	let resampled = gradient.map_ramp(settings, adjust);
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
	let settings = match adjusted_gradient(item.element(), settings, &adjustments) {
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
