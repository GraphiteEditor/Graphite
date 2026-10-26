struct HueSaturationRangeEffect {
	hue_shift: f32,
	saturation_factor: f32,
	fully_saturate: bool,
	rgb: vec3<f32>,
}

fn gamma_rgb_to_hsl(rgb: vec3<f32>) -> vec3<f32> {
	let maximum = max(max(rgb.r, rgb.g), rgb.b);
	let minimum = min(min(rgb.r, rgb.g), rgb.b);
	let chroma = maximum - minimum;
	let lightness = (maximum + minimum) / 2.;
	if chroma <= 0. {
		return vec3<f32>(0., 0., lightness);
	}

	let saturation = chroma / max(1. - abs(2. * lightness - 1.), 1e-6);
	return vec3<f32>(hexagon_hue_degrees(rgb), min(saturation, 1.), lightness);
}

fn hsl_to_gamma_rgb(hue: f32, saturation: f32, lightness: f32) -> vec3<f32> {
	let chroma = (1. - abs(2. * lightness - 1.)) * saturation;
	let sector = wrap_positive(hue, 360.) / 60.;
	let x = chroma * (1. - abs(wrap_positive(sector, 2.) - 1.));
	var rgb: vec3<f32>;
	if sector < 1. {
		rgb = vec3<f32>(chroma, x, 0.);
	} else if sector < 2. {
		rgb = vec3<f32>(x, chroma, 0.);
	} else if sector < 3. {
		rgb = vec3<f32>(0., chroma, x);
	} else if sector < 4. {
		rgb = vec3<f32>(0., x, chroma);
	} else if sector < 5. {
		rgb = vec3<f32>(x, 0., chroma);
	} else {
		rgb = vec3<f32>(chroma, 0., x);
	}
	let m = lightness - chroma / 2.;

	return clamp(rgb + m, vec3<f32>(0.), vec3<f32>(1.));
}

fn hue_distance(from_hue: f32, to_hue: f32) -> f32 {
	return wrap_positive(to_hue - from_hue, 360.);
}

fn hue_saturation_range_settings_weight(range: HueSaturationRangeSettings, hue: f32) -> f32 {
	if hue_distance(range.range_start, hue) <= hue_distance(range.range_start, range.range_end) {
		return 1.;
	}
	let start_falloff = hue_distance(range.falloff_start, range.range_start);
	let end_falloff = hue_distance(range.range_end, range.falloff_end);
	if hue_distance(range.falloff_start, hue) < start_falloff {
		return hue_distance(range.falloff_start, hue) / start_falloff;
	}
	if hue_distance(range.range_end, hue) < end_falloff {
		return 1. - hue_distance(range.range_end, hue) / end_falloff;
	}
	return 0.;
}

fn hue_saturation_range_effect_apply(effect: HueSaturationRangeEffect, range: HueSaturationRangeSettings, original_hue: f32, original_saturation: f32) -> HueSaturationRangeEffect {
	var range_weight = 0.;
	if original_saturation > 0. {
		range_weight = hue_saturation_range_settings_weight(range, original_hue);
	}
	if range_weight <= 0. {
		return effect;
	}

	var result = effect;
	result.hue_shift += range.settings.hue * range_weight;
	if range.settings.saturation >= 1. {
		result.fully_saturate = true;
	} else {
		result.saturation_factor *= 1. + (range.settings.saturation_gain - 1.) * range_weight;
	}
	result.rgb = lightness_toward_max_or_min(result.rgb, range.settings.lightness * range_weight);
	return result;
}

fn lightness_toward_white_or_black(value: f32, amount: f32) -> f32 {
	if amount >= 0. {
		return value + (1. - value) * amount;
	}
	return value * (1. + amount);
}

fn lightness_toward_max_or_min(rgb: vec3<f32>, amount: f32) -> vec3<f32> {
	let maximum = max(max(rgb.r, rgb.g), rgb.b);
	let minimum = min(min(rgb.r, rgb.g), rgb.b);
	let toward = select(minimum, maximum, amount >= 0.);
	return rgb + (toward - rgb) * abs(amount);
}

fn map_color(color: vec4<f32>) -> vec4<f32> {
	let parameters = params();
	let master = parameters.master;
	let channels = to_gamma_srgb_channels(color);

	if parameters.colorize {
		let colorize_settings = parameters.colorize_settings;
		let lightness = lightness_toward_white_or_black(gamma_rgb_to_hsl(channels.rgb).z, colorize_settings.lightness);
		return from_gamma_srgb_channels(vec4<f32>(hsl_to_gamma_rgb(colorize_settings.hue, colorize_settings.saturation, lightness), channels.a));
	}

	let original = gamma_rgb_to_hsl(channels.rgb);
	var effect = HueSaturationRangeEffect(master.hue, 1., false, channels.rgb);
	effect = hue_saturation_range_effect_apply(effect, parameters.reds, original.x, original.y);
	effect = hue_saturation_range_effect_apply(effect, parameters.yellows, original.x, original.y);
	effect = hue_saturation_range_effect_apply(effect, parameters.greens, original.x, original.y);
	effect = hue_saturation_range_effect_apply(effect, parameters.cyans, original.x, original.y);
	effect = hue_saturation_range_effect_apply(effect, parameters.blues, original.x, original.y);
	effect = hue_saturation_range_effect_apply(effect, parameters.magentas, original.x, original.y);
	var saturation_factor = effect.saturation_factor;
	var fully_saturate = effect.fully_saturate;
	if master.saturation >= 1. {
		fully_saturate = true;
	} else {
		saturation_factor *= master.saturation_gain;
	}

	let rgb = vec3<f32>(
		lightness_toward_white_or_black(effect.rgb.r, master.lightness),
		lightness_toward_white_or_black(effect.rgb.g, master.lightness),
		lightness_toward_white_or_black(effect.rgb.b, master.lightness),
	);
	let hsl = gamma_rgb_to_hsl(rgb);
	var saturation = min(hsl.y * saturation_factor, 1.);
	if hsl.y <= 0. {
		saturation = 0.;
	} else if fully_saturate {
		saturation = 1.;
	}

	return from_gamma_srgb_channels(vec4<f32>(hsl_to_gamma_rgb(hsl.x + effect.hue_shift, saturation, hsl.z), channels.a));
}
