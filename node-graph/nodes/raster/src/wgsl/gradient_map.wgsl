fn map_color(color: vec4<f32>) -> vec4<f32> {
	let parameters = params();
	let channels = to_gamma_srgb_channels(color);
	let intensity = 0.3 * channels.r + 0.59 * channels.g + 0.11 * channels.b;
	var t = select(intensity, 1. - intensity, parameters.reverse);
	switch parameters.spread {
		case 1u: {
			let cycle = t - 2. * floor(t / 2.);
			t = select(cycle, 2. - cycle, cycle > 1.);
		}
		case 2u: {
			t -= floor(t);
		}
		case 3u: {
			if t < 0. || t > 1. {
				return vec4<f32>(0., 0., 0., color.a);
			}
		}
		default: {}
	}
	let entry = select(gradient(t), gradient_stepped(t), parameters.stepped);
	return vec4<f32>(entry.rgb, color.a);
}
