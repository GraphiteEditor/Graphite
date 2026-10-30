fn map_color(color: vec4<f32>) -> vec4<f32> {
	let parameters = params();
	let channels = to_gamma_srgb_channels(color);

	let gray_base = min(min(channels.r, channels.g), channels.b);
	let part = channels.rgb - gray_base;

	var additional: f32;
	if part.r == 0. {
		let cyan_part = min(part.g, part.b);
		additional = cyan_part * parameters.cyans + (part.g - cyan_part) * parameters.greens + (part.b - cyan_part) * parameters.blues;
	} else if part.g == 0. {
		let magenta_part = min(part.r, part.b);
		additional = magenta_part * parameters.magentas + (part.r - magenta_part) * parameters.reds + (part.b - magenta_part) * parameters.blues;
	} else {
		let yellow_part = min(part.r, part.g);
		additional = yellow_part * parameters.yellows + (part.r - yellow_part) * parameters.reds + (part.g - yellow_part) * parameters.greens;
	}

	let luminance = clamp(gray_base + additional, 0., 1.);
	if !parameters.use_tint {
		return from_gamma_srgb_channels(vec4<f32>(vec3<f32>(luminance), channels.a));
	}

	let tint = to_gamma_srgb_channels(parameters.tint).rgb;
	let tinted = set_luminosity(tint, luma_rec_601_fixed_point(tint), luminance);
	return from_gamma_srgb_channels(vec4<f32>(tinted, channels.a));
}
