use crate::{CHANNELS_IN_RGB, Histogram, Image, Pixel};
use std::f64::consts::E;

impl Image<u16> {
	pub fn gamma_correction_fn(&self, histogram: &Histogram) -> impl Fn(Pixel) -> [u16; CHANNELS_IN_RGB] + use<> {
		let percentage = self.width * self.height;

		let mut white = 0;
		for channel_histogram in histogram {
			let mut total = 0;
			for i in (0x20..0x2000).rev() {
				total += channel_histogram[i] as u64;

				if total * 100 > percentage as u64 {
					white = white.max(i);
					break;
				}
			}
		}

		let curve = generate_gamma_curve(0.45, 4.5, (white << 3) as f64);

		move |pixel: Pixel| pixel.values.map(|value| curve[value as usize])
	}
}

/// `max_intensity` must be non-zero.
fn generate_gamma_curve(power: f64, threshold: f64, max_intensity: f64) -> Vec<u16> {
	debug_assert!(max_intensity != 0.);

	let (mut bound_start, mut bound_end) = if threshold >= 1. { (0., 1.) } else { (1., 0.) };

	let mut transition_point = 0.;
	let mut transition_ratio = 0.;
	let mut curve_adjustment = 0.;

	if threshold != 0. && (threshold - 1.) * (power - 1.) <= 0. {
		for _ in 0..48 {
			transition_point = (bound_start + bound_end) / 2.;

			if power != 0. {
				let temp_transition_ratio = transition_point / threshold;
				let exponential_power = temp_transition_ratio.powf(-power);
				let normalized_exponential_power = (exponential_power - 1.) / power;
				let comparison_result = normalized_exponential_power - (1. / transition_point);

				let bound_to_update = if comparison_result > -1. { &mut bound_end } else { &mut bound_start };
				*bound_to_update = transition_point;
			} else {
				let adjusted_transition_point = E.powf(1. - 1. / transition_point);
				let transition_point_ratio = transition_point / adjusted_transition_point;

				let bound_to_update = if transition_point_ratio < threshold { &mut bound_end } else { &mut bound_start };
				*bound_to_update = transition_point;
			}
		}

		transition_ratio = transition_point / threshold;

		if power != 0. {
			curve_adjustment = transition_point * ((1. / power) - 1.);
		}
	}

	let mut curve = vec![0xffff; 0x1_0000];
	let length = curve.len() as f64;

	for (i, entry) in curve.iter_mut().enumerate() {
		let ratio = (i as f64) / max_intensity;
		if ratio < 1. {
			let altered_ratio = if ratio < transition_ratio {
				ratio * threshold
			} else if power != 0. {
				ratio.powf(power) * (1. + curve_adjustment) - curve_adjustment
			} else {
				ratio.ln() * transition_point + 1.
			};

			*entry = (length * altered_ratio) as u16;
		}
	}

	curve
}
