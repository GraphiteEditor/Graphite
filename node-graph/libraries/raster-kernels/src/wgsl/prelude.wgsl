fn srgb_to_linear(channel: vec3<f32>) -> vec3<f32> {
	return select(pow((channel + 0.055) / 1.055, vec3<f32>(2.4)), channel / 12.92, channel <= vec3<f32>(0.04045));
}

fn linear_to_srgb(channel: vec3<f32>) -> vec3<f32> {
	return select(1.055 * pow(channel, vec3<f32>(1. / 2.4)) - 0.055, channel * 12.92, channel <= vec3<f32>(0.0031308));
}

fn to_gamma_srgb_channels(color: vec4<f32>) -> vec4<f32> {
	return vec4<f32>(linear_to_srgb(color.rgb), color.a);
}

fn from_gamma_srgb_channels(channels: vec4<f32>) -> vec4<f32> {
	return vec4<f32>(srgb_to_linear(channels.rgb), channels.a);
}

fn cbrt(value: f32) -> f32 {
	return sign(value) * pow(abs(value), 1. / 3.);
}

fn average_rgb_channels(color: vec4<f32>) -> f32 {
	return (color.r + color.g + color.b) / 3.;
}

fn minimum_rgb_channels(color: vec4<f32>) -> f32 {
	return min(min(color.r, color.g), color.b);
}

fn maximum_rgb_channels(color: vec4<f32>) -> f32 {
	return max(max(color.r, color.g), color.b);
}

fn luminance_rec_709(color: vec4<f32>) -> f32 {
	return 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b;
}

fn lightness_oklab(color: vec4<f32>) -> f32 {
	let long = 0.41222147 * color.r + 0.53633254 * color.g + 0.05144599 * color.b;
	let medium = 0.2119035 * color.r + 0.6806995 * color.g + 0.10739696 * color.b;
	let short = 0.08830246 * color.r + 0.28171884 * color.g + 0.6299787 * color.b;
	return 0.21045426 * cbrt(long) + 0.7936178 * cbrt(medium) - 0.004072047 * cbrt(short);
}

fn apply_gamma_exponent(color: vec4<f32>, exponent: f32) -> vec4<f32> {
	let inverse = 1. / max(exponent, 0.0001);
	return vec4<f32>(pow(color.rgb, vec3<f32>(inverse)), color.a);
}

fn pull_toward_luminosity(channels: vec3<f32>, luminosity: f32, scale: f32) -> vec3<f32> {
	return luminosity + (channels - luminosity) * scale;
}

fn set_luminosity(rgb: vec3<f32>, luma: f32, luminosity: f32) -> vec3<f32> {
	let shift = luminosity - luma;
	var channels = rgb + shift;

	let low = min(min(channels.r, channels.g), channels.b);
	if low < 0. {
		channels = pull_toward_luminosity(channels, luminosity, luminosity / (luminosity - low));
	}
	let high = max(max(channels.r, channels.g), channels.b);
	if high > 1. {
		channels = pull_toward_luminosity(channels, luminosity, (1. - luminosity) / (high - luminosity));
	}

	return clamp(channels, vec3<f32>(0.), vec3<f32>(1.));
}

fn luma_rec_601_rounded(channels: vec3<f32>) -> f32 {
	return 0.3 * channels.r + 0.59 * channels.g + 0.11 * channels.b;
}

fn channel_range(channels: vec3<f32>) -> f32 {
	return max(max(channels.r, channels.g), channels.b) - min(min(channels.r, channels.g), channels.b);
}

fn set_saturation(channels: vec3<f32>, saturation: f32) -> vec3<f32> {
	let low = min(min(channels.r, channels.g), channels.b);
	let range = channel_range(channels);
	if range <= 0. {
		return vec3<f32>(0.);
	}

	let scale = saturation / range;
	return (channels - low) * scale;
}
