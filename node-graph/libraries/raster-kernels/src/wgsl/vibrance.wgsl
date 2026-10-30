const VIBRANCE_PROTECTION_LOSS: f32 = 0.4857;

fn scale_about_maximum(channel: vec3<f32>, maximum: f32, chroma_factor: f32, brightness_factor: f32) -> vec3<f32> {
	return clamp(brightness_factor * (maximum + chroma_factor * (channel - maximum)), vec3<f32>(0.), vec3<f32>(1.));
}

fn vibrance_factors(rgb: vec3<f32>, amount: f32) -> vec2<f32> {
	let maximum = max(max(rgb.r, rgb.g), rgb.b);
	let minimum = min(min(rgb.r, rgb.g), rgb.b);
	if maximum <= 0. {
		return vec2<f32>(1., 1.);
	}
	let ratio = minimum / maximum;
	let saturation = 1. - ratio;
	let q = ratio * saturation;
	let toe = 1. - min(16. * maximum, 1.);
	let rolloff = 1. - toe * toe;
	let brightness = (2. * q - q * q) * (1. - maximum) * rolloff;

	if amount < 0. {
		let reduction = -amount;
		let chroma_factor = (1. - reduction / 4.) * (1. - reduction * (1. - rolloff * saturation * (1. + saturation) / 2.));
		return vec2<f32>(chroma_factor, 1. - reduction * brightness);
	}

	let protection = skin_tone_window(hexagon_hue_degrees(rgb)) * (1. - saturation * saturation);
	let protected_amount = amount * (1. - protection * (1. - amount));
	let boost = (5. / 6.) * (1. - VIBRANCE_PROTECTION_LOSS * protection) * protected_amount * ratio * (1. - minimum) * rolloff;
	return vec2<f32>(1. / (1. - boost), 1. + protected_amount * brightness / 4.);
}

fn skin_tone_window(hue: f32) -> f32 {
	if hue < 45. {
		return min((45. - hue) / 15., 1.);
	} else if hue >= 300. {
		return (hue - 300.) / 60.;
	}
	return 0.;
}

fn map_color(color: vec4<f32>) -> vec4<f32> {
	let parameters = params();
	let maximum = max(max(color.r, color.g), color.b);
	let factors = vibrance_factors(color.rgb, parameters.vibrance);
	let after_vibrance = scale_about_maximum(color.rgb, maximum, factors.x, factors.y);

	let gray = 0.288040 * after_vibrance.r + 0.711874 * after_vibrance.g + 0.000086 * after_vibrance.b;
	return vec4<f32>(clamp(gray + (after_vibrance - gray) * parameters.saturation_scale, vec3<f32>(0.), vec3<f32>(1.)), color.a);
}
