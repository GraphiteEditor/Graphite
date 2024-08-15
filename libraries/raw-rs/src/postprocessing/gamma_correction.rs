use crate::Image;
use std::f64::consts::E;

pub fn gamma_correction(mut image: Image<u16>) -> Image<u16> {
	todo!()
}

fn generate_gamma_curve(power: f64, threshold: f64, max_intensity: i32) -> Vec<u16> {
	let mut bounds = if threshold >= 1.0 { [0., 1.] } else { [1., 0.] };

	let mut transition_point = 0.;
	let mut transition_ratio = 0.;
	let mut curve_adjustment = 0.;
	if threshold != 0.0 && (threshold - 1.0) * (power - 1.0) <= 0.0 {
		for _ in 0..48 {
			transition_point = (bounds[0] + bounds[1]) / 2.0;
			let index = if power != 0.0 {
				((transition_point / threshold).powf(-power) - 1.0) / power - 1.0 / transition_point > -1.0
			} else {
				transition_point / (E.powf(1.0 - 1.0 / transition_point)) < threshold
			};
			bounds[index as usize] = transition_point;
		}
		transition_ratio = transition_point / threshold;
		if power != 0.0 {
			curve_adjustment = transition_point * (1.0 / power - 1.0);
		}
	}

	let mut curve = vec![0xffffu16; 0x10000];
	for i in 0..0x10000 {
		let ratio = (i as f64) / (max_intensity as f64);
		if ratio < 1.0 {
			curve[i as usize] = (0x10000 as f64
				* if ratio < transition_ratio {
					ratio * threshold
				} else if power != 0.0 {
					ratio.powf(power) * (1.0 + curve_adjustment) - curve_adjustment
				} else {
					ratio.ln() * transition_point + 1.0
				}) as u16;
		}
	}

	curve
}
