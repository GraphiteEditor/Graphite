const BLEND_MODE_NORMAL: i32 = 0;
const BLEND_MODE_DARKEN: i32 = 1;
const BLEND_MODE_MULTIPLY: i32 = 2;
const BLEND_MODE_COLOR_BURN: i32 = 3;
const BLEND_MODE_LINEAR_BURN: i32 = 4;
const BLEND_MODE_DARKER_COLOR: i32 = 5;
const BLEND_MODE_LIGHTEN: i32 = 6;
const BLEND_MODE_SCREEN: i32 = 7;
const BLEND_MODE_COLOR_DODGE: i32 = 8;
const BLEND_MODE_LINEAR_DODGE: i32 = 9;
const BLEND_MODE_LIGHTER_COLOR: i32 = 10;
const BLEND_MODE_OVERLAY: i32 = 11;
const BLEND_MODE_SOFT_LIGHT: i32 = 12;
const BLEND_MODE_HARD_LIGHT: i32 = 13;
const BLEND_MODE_VIVID_LIGHT: i32 = 14;
const BLEND_MODE_LINEAR_LIGHT: i32 = 15;
const BLEND_MODE_PIN_LIGHT: i32 = 16;
const BLEND_MODE_HARD_MIX: i32 = 17;
const BLEND_MODE_DIFFERENCE: i32 = 18;
const BLEND_MODE_EXCLUSION: i32 = 19;
const BLEND_MODE_SUBTRACT: i32 = 20;
const BLEND_MODE_DIVIDE: i32 = 21;
const BLEND_MODE_HUE: i32 = 22;
const BLEND_MODE_SATURATION: i32 = 23;
const BLEND_MODE_COLOR: i32 = 24;
const BLEND_MODE_LUMINOSITY: i32 = 25;
const BLEND_MODE_ERASE: i32 = 26;
const BLEND_MODE_RESTORE: i32 = 27;
const BLEND_MODE_MULTIPLY_ALPHA: i32 = 28;

fn blend_normal(c_b: f32, c_s: f32) -> f32 {
	return c_s;
}

fn blend_multiply(c_b: f32, c_s: f32) -> f32 {
	return c_s * c_b;
}

fn blend_darken(c_b: f32, c_s: f32) -> f32 {
	return min(c_s, c_b);
}

fn blend_color_burn(c_b: f32, c_s: f32) -> f32 {
	if c_b == 1. {
		return 1.;
	} else if c_s == 0. {
		return 0.;
	}
	return 1. - min((1. - c_b) / c_s, 1.);
}

fn blend_linear_burn(c_b: f32, c_s: f32) -> f32 {
	return c_b + c_s - 1.;
}

fn blend_darker_color(color: vec4<f32>, other: vec4<f32>) -> vec4<f32> {
	let darker = select(other, color, average_rgb_channels(color) <= average_rgb_channels(other));

	return vec4<f32>(darker.rgb, other.a);
}

fn blend_screen(c_b: f32, c_s: f32) -> f32 {
	return 1. - (1. - c_s) * (1. - c_b);
}

fn blend_lighten(c_b: f32, c_s: f32) -> f32 {
	return max(c_s, c_b);
}

fn blend_color_dodge(c_b: f32, c_s: f32) -> f32 {
	if c_s == 1. {
		return 1.;
	}
	return min(c_b / (1. - c_s), 1.);
}

fn blend_linear_dodge(c_b: f32, c_s: f32) -> f32 {
	return c_b + c_s;
}

fn blend_lighter_color(color: vec4<f32>, other: vec4<f32>) -> vec4<f32> {
	let lighter = select(other, color, average_rgb_channels(color) >= average_rgb_channels(other));

	return vec4<f32>(lighter.rgb, other.a);
}

fn blend_softlight(c_b: f32, c_s: f32) -> f32 {
	if c_s <= 0.5 {
		return c_b - (1. - 2. * c_s) * c_b * (1. - c_b);
	}
	var d = 0.;
	if c_b <= 0.25 {
		d = ((16. * c_b - 12.) * c_b + 4.) * c_b;
	} else {
		d = sqrt(c_b);
	}
	return c_b + (2. * c_s - 1.) * (d - c_b);
}

fn blend_hardlight(c_b: f32, c_s: f32) -> f32 {
	if c_s <= 0.5 {
		return blend_multiply(2. * c_s, c_b);
	}
	return blend_screen(2. * c_s - 1., c_b);
}

fn blend_overlay(c_b: f32, c_s: f32) -> f32 {
	return blend_hardlight(c_s, c_b);
}

fn blend_vivid_light(c_b: f32, c_s: f32) -> f32 {
	if c_s <= 0.5 {
		return blend_color_burn(2. * c_s, c_b);
	}
	return blend_color_dodge(2. * c_s - 1., c_b);
}

fn blend_linear_light(c_b: f32, c_s: f32) -> f32 {
	if c_s <= 0.5 {
		return blend_linear_burn(2. * c_s, c_b);
	}
	return blend_linear_dodge(2. * c_s - 1., c_b);
}

fn blend_pin_light(c_b: f32, c_s: f32) -> f32 {
	if c_s <= 0.5 {
		return blend_darken(2. * c_s, c_b);
	}
	return blend_lighten(2. * c_s - 1., c_b);
}

fn blend_hard_mix(c_b: f32, c_s: f32) -> f32 {
	return select(1., 0., blend_linear_light(c_b, c_s) < 0.5);
}

fn blend_difference(c_b: f32, c_s: f32) -> f32 {
	return abs(c_b - c_s);
}

fn blend_exclusion(c_b: f32, c_s: f32) -> f32 {
	return c_b + c_s - 2. * c_b * c_s;
}

fn blend_subtract(c_b: f32, c_s: f32) -> f32 {
	return c_b - c_s;
}

fn blend_divide(c_b: f32, c_s: f32) -> f32 {
	if c_b == 0. {
		return 1.;
	}
	return c_b / c_s;
}

fn blend_channel(blend_mode: i32, c_b: f32, c_s: f32) -> f32 {
	switch blend_mode {
		case BLEND_MODE_DARKEN: {
			return blend_darken(c_b, c_s);
		}
		case BLEND_MODE_MULTIPLY: {
			return blend_multiply(c_b, c_s);
		}
		case BLEND_MODE_COLOR_BURN: {
			return blend_color_burn(c_b, c_s);
		}
		case BLEND_MODE_LINEAR_BURN: {
			return blend_linear_burn(c_b, c_s);
		}
		case BLEND_MODE_LIGHTEN: {
			return blend_lighten(c_b, c_s);
		}
		case BLEND_MODE_SCREEN: {
			return blend_screen(c_b, c_s);
		}
		case BLEND_MODE_COLOR_DODGE: {
			return blend_color_dodge(c_b, c_s);
		}
		case BLEND_MODE_LINEAR_DODGE: {
			return blend_linear_dodge(c_b, c_s);
		}
		case BLEND_MODE_OVERLAY: {
			return blend_overlay(c_b, c_s);
		}
		case BLEND_MODE_SOFT_LIGHT: {
			return blend_softlight(c_b, c_s);
		}
		case BLEND_MODE_HARD_LIGHT: {
			return blend_hardlight(c_b, c_s);
		}
		case BLEND_MODE_VIVID_LIGHT: {
			return blend_vivid_light(c_b, c_s);
		}
		case BLEND_MODE_LINEAR_LIGHT: {
			return blend_linear_light(c_b, c_s);
		}
		case BLEND_MODE_PIN_LIGHT: {
			return blend_pin_light(c_b, c_s);
		}
		case BLEND_MODE_HARD_MIX: {
			return blend_hard_mix(c_b, c_s);
		}
		case BLEND_MODE_DIFFERENCE: {
			return blend_difference(c_b, c_s);
		}
		case BLEND_MODE_EXCLUSION: {
			return blend_exclusion(c_b, c_s);
		}
		case BLEND_MODE_SUBTRACT: {
			return blend_subtract(c_b, c_s);
		}
		case BLEND_MODE_DIVIDE: {
			return blend_divide(c_b, c_s);
		}
		default: {
			return blend_normal(c_b, c_s);
		}
	}
}

fn blend_rgb(color: vec4<f32>, other: vec4<f32>, blend_mode: i32) -> vec4<f32> {
	let red = blend_channel(blend_mode, color.r, other.r);
	let green = blend_channel(blend_mode, color.g, other.g);
	let blue = blend_channel(blend_mode, color.b, other.b);

	return vec4<f32>(clamp(vec3<f32>(red, green, blue), vec3<f32>(0.), vec3<f32>(1.)), other.a);
}

fn gamma_rgb_in_range(color: vec4<f32>) -> vec3<f32> {
	return clamp(to_gamma_srgb_channels(color).rgb, vec3<f32>(0.), vec3<f32>(1.));
}

fn blend_hue(color: vec4<f32>, c_s: vec4<f32>) -> vec4<f32> {
	let backdrop = gamma_rgb_in_range(color);
	let saturated = set_saturation(gamma_rgb_in_range(c_s), channel_range(backdrop));
	let blended = set_luminosity(saturated, luma_rec_601_rounded(saturated), luma_rec_601_rounded(backdrop));

	return from_gamma_srgb_channels(vec4<f32>(blended, c_s.a));
}

fn blend_saturation(color: vec4<f32>, c_s: vec4<f32>) -> vec4<f32> {
	let backdrop = gamma_rgb_in_range(color);
	let saturated = set_saturation(backdrop, channel_range(gamma_rgb_in_range(c_s)));
	let blended = set_luminosity(saturated, luma_rec_601_rounded(saturated), luma_rec_601_rounded(backdrop));

	return from_gamma_srgb_channels(vec4<f32>(blended, c_s.a));
}

fn blend_color(color: vec4<f32>, c_s: vec4<f32>) -> vec4<f32> {
	let backdrop = gamma_rgb_in_range(color);
	let source = gamma_rgb_in_range(c_s);
	let blended = set_luminosity(source, luma_rec_601_rounded(source), luma_rec_601_rounded(backdrop));

	return from_gamma_srgb_channels(vec4<f32>(blended, c_s.a));
}

fn blend_luminosity(color: vec4<f32>, c_s: vec4<f32>) -> vec4<f32> {
	let backdrop = gamma_rgb_in_range(color);
	let source = gamma_rgb_in_range(c_s);
	let blended = set_luminosity(backdrop, luma_rec_601_rounded(backdrop), luma_rec_601_rounded(source));

	return from_gamma_srgb_channels(vec4<f32>(blended, c_s.a));
}

fn alpha_blend(color: vec4<f32>, other: vec4<f32>) -> vec4<f32> {
	let under_weight = color.a * (1. - other.a);
	let alpha = other.a + under_weight;
	if alpha == 0. {
		return vec4<f32>(0.);
	}
	return vec4<f32>((other.rgb * other.a + color.rgb * under_weight) / alpha, alpha);
}

fn alpha_add(color: vec4<f32>, other: vec4<f32>) -> vec4<f32> {
	return vec4<f32>(color.rgb, clamp(color.a + other.a, 0., 1.));
}

fn alpha_subtract(color: vec4<f32>, other: vec4<f32>) -> vec4<f32> {
	return vec4<f32>(color.rgb, clamp(color.a - other.a, 0., 1.));
}

fn alpha_multiply(color: vec4<f32>, other: vec4<f32>) -> vec4<f32> {
	return vec4<f32>(color.rgb, clamp(color.a * other.a, 0., 1.));
}

fn blend_colors(foreground: vec4<f32>, background: vec4<f32>, blend_mode: i32, opacity: f32) -> vec4<f32> {
	var applied = vec4<f32>(0.);
	switch blend_mode {
		case BLEND_MODE_ERASE: {
			applied = alpha_subtract(background, foreground);
		}
		case BLEND_MODE_RESTORE: {
			applied = alpha_add(background, foreground);
		}
		case BLEND_MODE_MULTIPLY_ALPHA: {
			applied = alpha_multiply(background, foreground);
		}
		default: {
			let target_color = apply_blend_mode(foreground, background, blend_mode);
			return alpha_blend(background, vec4<f32>(target_color.rgb, target_color.a * opacity));
		}
	}

	return vec4<f32>(background.rgb, background.a + (applied.a - background.a) * opacity);
}

fn apply_blend_mode(foreground: vec4<f32>, background: vec4<f32>, blend_mode: i32) -> vec4<f32> {
	switch blend_mode {
		case BLEND_MODE_DARKER_COLOR: {
			return blend_darker_color(background, foreground);
		}
		case BLEND_MODE_LIGHTER_COLOR: {
			return blend_lighter_color(background, foreground);
		}
		case BLEND_MODE_HUE: {
			return blend_hue(background, foreground);
		}
		case BLEND_MODE_SATURATION: {
			return blend_saturation(background, foreground);
		}
		case BLEND_MODE_COLOR: {
			return blend_color(background, foreground);
		}
		case BLEND_MODE_LUMINOSITY: {
			return blend_luminosity(background, foreground);
		}
		case BLEND_MODE_ERASE, BLEND_MODE_RESTORE, BLEND_MODE_MULTIPLY_ALPHA: {
			return foreground;
		}
		default: {
			return blend_rgb(background, foreground, blend_mode);
		}
	}
}
