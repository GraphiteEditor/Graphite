const LUMINANCE_REC_709: u32 = 0u;
const LUMA_REC_709: u32 = 1u;
const LUMA_REC_601: u32 = 2u;
const LIGHTNESS_OK_LAB: u32 = 3u;
const CHANNELS_AVERAGE: u32 = 4u;
const CHANNELS_MINIMUM: u32 = 5u;
const CHANNELS_MAXIMUM: u32 = 6u;
const LIGHTNESS_HSL: u32 = 7u;

fn map_color(color: vec4<f32>) -> vec4<f32> {
	var luminance: f32;
	switch params().method {
		case LUMA_REC_709: {
			let channels = to_gamma_srgb_channels(color);
			luminance = srgb_to_linear(vec3<f32>(0.2126 * channels.r + 0.7152 * channels.g + 0.0722 * channels.b)).x;
		}
		case LUMA_REC_601: {
			let channels = to_gamma_srgb_channels(color);
			luminance = srgb_to_linear(vec3<f32>(0.299 * channels.r + 0.587 * channels.g + 0.114 * channels.b)).x;
		}
		case LIGHTNESS_OK_LAB: {
			let lightness = lightness_oklab(color);
			luminance = lightness * lightness * lightness;
		}
		case CHANNELS_AVERAGE: {
			luminance = average_rgb_channels(color);
		}
		case CHANNELS_MINIMUM: {
			luminance = minimum_rgb_channels(color);
		}
		case CHANNELS_MAXIMUM: {
			luminance = maximum_rgb_channels(color);
		}
		case LIGHTNESS_HSL: {
			let extremes = linear_to_srgb(vec3<f32>(maximum_rgb_channels(color), minimum_rgb_channels(color), 0.));
			luminance = srgb_to_linear(vec3<f32>((extremes.x + extremes.y) / 2.)).x;
		}
		case LUMINANCE_REC_709, default: {
			luminance = luminance_rec_709(color);
		}
	}
	return vec4<f32>(vec3<f32>(luminance), color.a);
}
