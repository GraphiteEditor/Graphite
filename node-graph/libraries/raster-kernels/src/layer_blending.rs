//! Blends a layer onto those beneath it by a blend mode, with the W3C Compositing and Blending formulas on gamma-encoded colors, as SVG and the GPU's layers blend.
//! The modes CSS lacks blend normally, since layers only ever take the others.

use crate::effect_kernels::Plane;
use core_types::blending::BlendMode;

/// The premultiplied gamma-encoded `layer` blended by the mode onto the layers `below` it. O(width × height).
pub fn blend_layer(layer: &Plane<[f32; 4]>, below: &Plane<[f32; 4]>, blend_mode: BlendMode) -> Plane<[f32; 4]> {
	layer.zip(below, |source, backdrop| blend_pixel(source, backdrop, blend_mode))
}

/// One premultiplied pixel blended onto another: its color mixes toward the blend of both as far as the backdrop covers it, and it then composites over the backdrop.
pub fn blend_pixel(source: [f32; 4], backdrop: [f32; 4], blend_mode: BlendMode) -> [f32; 4] {
	let (source_alpha, backdrop_alpha) = (source[3], backdrop[3]);
	if source_alpha <= 0. {
		return backdrop;
	}

	let straight = |pixel: [f32; 4]| match pixel[3] > 0. {
		true => [0, 1, 2].map(|channel| (pixel[channel] / pixel[3]).clamp(0., 1.)),
		false => [0.; 3],
	};
	let (source_color, backdrop_color) = (straight(source), straight(backdrop));
	let blended = blend(backdrop_color, source_color, blend_mode);

	let mut result = [0.; 4];
	for channel in 0..3 {
		let mixed = (1. - backdrop_alpha) * source_color[channel] + backdrop_alpha * blended[channel];
		result[channel] = source_alpha * mixed + (1. - source_alpha) * backdrop[channel];
	}
	result[3] = source_alpha + backdrop_alpha * (1. - source_alpha);
	result
}

/// The mode's blend of straight `backdrop` and `source` colors.
fn blend(backdrop: [f32; 3], source: [f32; 3], blend_mode: BlendMode) -> [f32; 3] {
	match blend_mode {
		BlendMode::Hue => set_luminosity(set_saturation(source, saturation(backdrop)), luminosity(backdrop)),
		BlendMode::Saturation => set_luminosity(set_saturation(backdrop, saturation(source)), luminosity(backdrop)),
		BlendMode::Color => set_luminosity(source, luminosity(backdrop)),
		BlendMode::Luminosity => set_luminosity(backdrop, luminosity(source)),
		_ => [0, 1, 2].map(|channel| blend_channel(backdrop[channel], source[channel], blend_mode)),
	}
}

/// The mode's blend of one channel of the backdrop and source, for the modes that blend each channel on its own.
fn blend_channel(backdrop: f32, source: f32, blend_mode: BlendMode) -> f32 {
	let hard_light = |backdrop: f32, source: f32| match source <= 0.5 {
		true => backdrop * 2. * source,
		false => screen(backdrop, 2. * source - 1.),
	};

	match blend_mode {
		BlendMode::Multiply => backdrop * source,
		BlendMode::Screen => screen(backdrop, source),
		BlendMode::Overlay => hard_light(source, backdrop),
		BlendMode::Darken => backdrop.min(source),
		BlendMode::Lighten => backdrop.max(source),
		BlendMode::ColorDodge if backdrop <= 0. => 0.,
		BlendMode::ColorDodge if source >= 1. => 1.,
		BlendMode::ColorDodge => (backdrop / (1. - source)).min(1.),
		BlendMode::ColorBurn if backdrop >= 1. => 1.,
		BlendMode::ColorBurn if source <= 0. => 0.,
		BlendMode::ColorBurn => 1. - ((1. - backdrop) / source).min(1.),
		BlendMode::HardLight => hard_light(backdrop, source),
		BlendMode::SoftLight if source <= 0.5 => backdrop - (1. - 2. * source) * backdrop * (1. - backdrop),
		BlendMode::SoftLight => {
			let darkened = if backdrop <= 0.25 { ((16. * backdrop - 12.) * backdrop + 4.) * backdrop } else { backdrop.sqrt() };
			backdrop + (2. * source - 1.) * (darkened - backdrop)
		}
		BlendMode::Difference => (backdrop - source).abs(),
		BlendMode::Exclusion => backdrop + source - 2. * backdrop * source,
		_ => source,
	}
}

fn screen(backdrop: f32, source: f32) -> f32 {
	backdrop + source - backdrop * source
}

fn luminosity([red, green, blue]: [f32; 3]) -> f32 {
	0.3 * red + 0.59 * green + 0.11 * blue
}

/// The color moved to the `target` luminosity, its channels pulled back toward it where they leave the range from 0 to 1.
fn set_luminosity(color: [f32; 3], target: f32) -> [f32; 3] {
	let shift = target - luminosity(color);
	let color = color.map(|channel| channel + shift);

	// Channels all equal to the luminosity have no spread to pull back, only rounding past the range, which clamping removes
	let luminosity = luminosity(color);
	let (minimum, maximum) = (color[0].min(color[1]).min(color[2]), color[0].max(color[1]).max(color[2]));
	color.map(|channel| {
		let channel = if minimum < 0. && luminosity > minimum {
			luminosity + (channel - luminosity) * luminosity / (luminosity - minimum)
		} else {
			channel
		};
		let channel = if maximum > 1. && maximum > luminosity {
			luminosity + (channel - luminosity) * (1. - luminosity) / (maximum - luminosity)
		} else {
			channel
		};
		channel.clamp(0., 1.)
	})
}

fn saturation([red, green, blue]: [f32; 3]) -> f32 {
	red.max(green).max(blue) - red.min(green).min(blue)
}

/// The color with its channels spread to the `target` saturation, keeping which is largest, middle, and smallest.
fn set_saturation(color: [f32; 3], target: f32) -> [f32; 3] {
	let (minimum, maximum) = (color[0].min(color[1]).min(color[2]), color[0].max(color[1]).max(color[2]));
	if maximum <= minimum {
		return [0.; 3];
	}
	color.map(|channel| (channel - minimum) * target / (maximum - minimum))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn blending_over_nothing_leaves_the_source_as_it_is() {
		let source = [0.2, 0.3, 0.1, 0.5];
		for blend_mode in [BlendMode::Multiply, BlendMode::Screen, BlendMode::Hue, BlendMode::ColorBurn] {
			let blended = blend_pixel(source, [0.; 4], blend_mode);
			assert!(blended.iter().zip(source).all(|(blended, source)| (blended - source).abs() < 1e-6), "{blend_mode:?}");
		}
	}

	#[test]
	fn an_opaque_backdrop_takes_the_blend_of_both_colors() {
		let multiplied = blend_pixel([0.5, 0.5, 0.5, 1.], [0.8, 0.4, 1., 1.], BlendMode::Multiply);
		assert!((multiplied[0] - 0.4).abs() < 1e-6 && (multiplied[1] - 0.2).abs() < 1e-6 && (multiplied[2] - 0.5).abs() < 1e-6);
		assert_eq!(multiplied[3], 1.);

		// Half a source mixes halfway toward the blend
		let screened = blend_pixel([0.25, 0.25, 0.25, 0.5], [0.5, 0.5, 0.5, 1.], BlendMode::Screen);
		assert!((screened[0] - (0.5 * 0.75 + 0.5 * 0.5)).abs() < 1e-6);
	}

	#[test]
	fn the_luminosity_of_black_over_any_gray_is_black() {
		for level in 0..=255 {
			let gray = level as f32 / 255.;
			let blended = blend_pixel([0., 0., 0., 1.], [gray, gray, gray, 1.], BlendMode::Luminosity);
			assert!(blended.iter().take(3).all(|&channel| channel.abs() < 1e-6), "gray {level} gives {blended:?}");
		}
	}
}
