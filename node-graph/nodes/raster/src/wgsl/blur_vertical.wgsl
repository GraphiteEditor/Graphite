struct BlurSum {
	color: vec4<f32>,
	weight: f32,
}

fn blur_tap_weight(coord: vec2<i32>, offset: i32) -> f32 {
	let parameters = params();
	let coverage = blur_coverage(coord + vec2<i32>(0, offset), parameters.full_size, parameters.scale).y;
	return blur_weight(abs(offset), parameters.radius, parameters.box_blur) * coverage;
}

fn blur_side(coord: vec2<i32>, direction: i32, taps: i32) -> BlurSum {
	var sum = BlurSum(vec4<f32>(0.), 0.);
	for (var near = 1; near <= taps; near += 2) {
		let near_weight = blur_tap_weight(coord, direction * near);
		if near + 1 > taps {
			sum.color += input_0_load(coord + vec2<i32>(0, direction * near)) * near_weight;
			sum.weight += near_weight;
			break;
		}
		let far_weight = blur_tap_weight(coord, direction * (near + 1));
		let weight = near_weight + far_weight;
		let offset = f32(near) + far_weight / weight;
		sum.color += input_0_sample(vec2<f32>(coord) + 0.5 + vec2<f32>(0., f32(direction) * offset)) * weight;
		sum.weight += weight;
	}
	return sum;
}

fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let parameters = params();
	let above = blur_side(coord, -1, min(parameters.reach, coord.y));
	let below = blur_side(coord, 1, min(parameters.reach, input_0_size().y - 1 - coord.y));
	let center_weight = blur_tap_weight(coord, 0);
	let sum = input_0_load(coord) * center_weight + above.color + below.color;
	return blur_unpremultiply(sum / (center_weight + above.weight + below.weight), parameters.space);
}
