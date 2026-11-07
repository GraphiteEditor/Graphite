fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let parameters = params();
	let first = -min(parameters.reach, coord.x);
	let last = min(parameters.reach, input_0_size().x - 1 - coord.x);

	var sum = vec4<f32>(0.);
	var weight_sum = 0.;
	for (var offset = first; offset <= last; offset++) {
		let texel = vec2<i32>(coord.x + offset, coord.y);
		let weight = blur_weight(offset, parameters.radius, parameters.box_blur) * blur_coverage(texel, parameters.full_size, parameters.scale).x;
		sum += blur_premultiply(input_0_load(texel), parameters.space) * weight;
		weight_sum += weight;
	}
	return sum / weight_sum;
}
