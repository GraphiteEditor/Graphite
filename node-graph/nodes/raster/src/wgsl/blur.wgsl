const BLUR_LINEAR: u32 = 1u;
const BLUR_GAMMA: u32 = 2u;

fn blur_weight(offset: i32, radius: f32, box_blur: bool) -> f32 {
	let distance = f32(offset);
	if box_blur {
		return clamp(radius + 1. - abs(distance), 0., 1.);
	}
	return exp(-(distance * distance) / (2. * radius * radius));
}

fn blur_coverage(texel: vec2<i32>, full_size: array<i32, 2>, scale: i32) -> vec2<f32> {
	let covered = clamp(vec2<i32>(full_size[0], full_size[1]) - texel * scale, vec2<i32>(0), vec2<i32>(scale));
	return vec2<f32>(covered) / f32(scale);
}

fn blur_premultiply(color: vec4<f32>, space: u32) -> vec4<f32> {
	if space == BLUR_LINEAR {
		return vec4<f32>(color.rgb * color.a, color.a);
	}
	if space == BLUR_GAMMA {
		return vec4<f32>(linear_to_srgb(color.rgb) * color.a, color.a);
	}
	return color;
}

fn blur_unpremultiply(color: vec4<f32>, space: u32) -> vec4<f32> {
	if space == BLUR_LINEAR {
		if color.a == 0. {
			return color;
		}
		return vec4<f32>(color.rgb * (1. / color.a), color.a);
	}
	if space == BLUR_GAMMA {
		if color.a <= 0. {
			return vec4<f32>(0.);
		}
		return vec4<f32>(srgb_to_linear(color.rgb * (1. / color.a)), color.a);
	}
	return color;
}
