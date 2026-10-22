//! The luma and saturation constructions that the component blend modes (hue, saturation, color, and luminosity)
//! and the adjustments derived from them are built on. All of them work on gamma-encoded channels across 0..1.
//!
//! <https://www.w3.org/TR/compositing-1/#blendingnonseparable>

/// The Rec. 601 luma with its weights rounded to two decimal places, as the specification defines it for these blend modes.
pub fn luma_rec_601_rounded(r: f32, g: f32, b: f32) -> f32 {
	0.3 * r + 0.59 * g + 0.11 * b
}

/// The spread between the largest and smallest channels, which is what the component blend modes call saturation.
pub fn channel_range(r: f32, g: f32, b: f32) -> f32 {
	r.max(g).max(b) - r.min(g).min(b)
}

fn pull_toward_luminosity(channels: [f32; 3], luminosity: f32, scale: f32) -> [f32; 3] {
	[
		luminosity + (channels[0] - luminosity) * scale,
		luminosity + (channels[1] - luminosity) * scale,
		luminosity + (channels[2] - luminosity) * scale,
	]
}

/// The Luminosity blend mode's construction: shifts gamma-encoded channels from `luma` to `luminosity`,
/// then pulls them toward it just enough to bring every channel back into 0..1.
pub fn set_luminosity(r: f32, g: f32, b: f32, luma: f32, luminosity: f32) -> [f32; 3] {
	let shift = luminosity - luma;
	let mut channels = [r + shift, g + shift, b + shift];

	let low = channels[0].min(channels[1]).min(channels[2]);
	if low < 0. {
		channels = pull_toward_luminosity(channels, luminosity, luminosity / (luminosity - low));
	}
	let high = channels[0].max(channels[1]).max(channels[2]);
	if high > 1. {
		channels = pull_toward_luminosity(channels, luminosity, (1. - luminosity) / (high - luminosity));
	}

	[channels[0].clamp(0., 1.), channels[1].clamp(0., 1.), channels[2].clamp(0., 1.)]
}

/// The Saturation blend mode's construction: scales gamma-encoded channels about their smallest, which becomes 0, so the largest becomes `saturation`. A gray becomes black.
pub fn set_saturation(r: f32, g: f32, b: f32, saturation: f32) -> [f32; 3] {
	let low = r.min(g).min(b);
	let range = channel_range(r, g, b);
	if range <= 0. {
		return [0.; 3];
	}

	let scale = saturation / range;
	[(r - low) * scale, (g - low) * scale, (b - low) * scale]
}

#[cfg(test)]
mod tests {
	use super::*;

	// The specification's own formulation, which sorts the channels and scales only the middle one
	fn specification_set_saturation(channels: [f32; 3], saturation: f32) -> [f32; 3] {
		let mut order = [0, 1, 2];
		order.sort_by(|&a, &b| channels[a].total_cmp(&channels[b]));
		let [low, middle, high] = order;

		let mut result = [0.; 3];
		if channels[high] > channels[low] {
			result[middle] = (channels[middle] - channels[low]) * saturation / (channels[high] - channels[low]);
			result[high] = saturation;
		}
		result
	}

	#[test]
	fn set_saturation_matches_the_specification() {
		for channels in [[0.2, 0.7, 0.4], [0.9, 0.1, 0.5], [0.3, 0.3, 0.8], [1., 0., 0.], [0.6, 0.6, 0.6]] {
			for saturation in [0., 0.35, 1.] {
				let ours = set_saturation(channels[0], channels[1], channels[2], saturation);
				let expected = specification_set_saturation(channels, saturation);

				for channel in 0..3 {
					assert!((ours[channel] - expected[channel]).abs() < 1e-6, "{channels:?} at {saturation} gave {ours:?}, not {expected:?}");
				}
			}
		}
	}

	#[test]
	fn set_luminosity_lands_on_the_luminosity_and_stays_in_range() {
		// Pure red raised to a mid luminosity overshoots in red, so it is pulled back toward the luminosity
		let [r, g, b] = set_luminosity(1., 0., 0., luma_rec_601_rounded(1., 0., 0.), 0.5);

		assert!((luma_rec_601_rounded(r, g, b) - 0.5).abs() < 1e-5);
		assert!((r - 1.).abs() < 1e-5 && (g - b).abs() < 1e-6 && g > 0. && g < 0.5, "got {r}, {g}, {b}");
	}
}
