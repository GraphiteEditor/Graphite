const RELATIVE: u32 = 0u;

const REDS: u32 = 0u;
const YELLOWS: u32 = 1u;
const GREENS: u32 = 2u;
const CYANS: u32 = 3u;
const BLUES: u32 = 4u;
const MAGENTAS: u32 = 5u;
const WHITES: u32 = 6u;
const NEUTRALS: u32 = 7u;
const BLACKS: u32 = 8u;

fn pixel_color_range(choice: u32, rgb: vec3<f32>, max_channel: f32, min_channel: f32) -> bool {
	switch choice {
		case REDS: {
			return max_channel == rgb.r;
		}
		case YELLOWS: {
			return min_channel == rgb.b;
		}
		case GREENS: {
			return max_channel == rgb.g;
		}
		case CYANS: {
			return min_channel == rgb.r;
		}
		case BLUES: {
			return max_channel == rgb.b;
		}
		case MAGENTAS: {
			return min_channel == rgb.g;
		}
		case WHITES: {
			return all(rgb > vec3<f32>(0.5));
		}
		case BLACKS: {
			return all(rgb < vec3<f32>(0.5));
		}
		default: {
			return true;
		}
	}
}

fn color_parameter_group_scale_factor(choice: u32, max_channel: f32, med_channel: f32, min_channel: f32) -> f32 {
	switch choice {
		case REDS, GREENS, BLUES: {
			return max_channel - med_channel;
		}
		case CYANS, MAGENTAS, YELLOWS: {
			return med_channel - min_channel;
		}
		case WHITES: {
			return min_channel * 2. - 1.;
		}
		case NEUTRALS: {
			return 1. - (abs(max_channel - 0.5) + abs(min_channel - 0.5));
		}
		default: {
			return 1. - max_channel * 2.;
		}
	}
}

fn map_color(color: vec4<f32>) -> vec4<f32> {
	let parameters = params();
	let channels = to_gamma_srgb_channels(color);
	let rgb = channels.rgb;

	let max_channel = max(max(rgb.r, rgb.g), rgb.b);
	let min_channel = min(min(rgb.r, rgb.g), rgb.b);
	let med_channel = rgb.r + rgb.g + rgb.b - min_channel - max_channel;

	let slope = select(vec3<f32>(-1.), rgb - 1., parameters.mode == RELATIVE);

	var groups = parameters.groups;
	var sum = vec3<f32>(0.);
	for (var index = 0u; index < 9u; index++) {
		let group = groups[index];
		let cmyk = vec4<f32>(group.cmyk[0], group.cmyk[1], group.cmyk[2], group.cmyk[3]);
		if all(cmyk == vec4<f32>(0.)) || !pixel_color_range(group.color_parameter_group, rgb, max_channel, min_channel) {
			continue;
		}

		let inks = vec3<f32>(group.inks[0], group.inks[1], group.inks[2]);
		let scale_factor = color_parameter_group_scale_factor(group.color_parameter_group, max_channel, med_channel, min_channel);
		sum += clamp(inks * slope, -rgb, -rgb + 1.) * scale_factor;
	}

	return from_gamma_srgb_channels(vec4<f32>(clamp(sum + rgb, vec3<f32>(0.), vec3<f32>(1.)), channels.a));
}
