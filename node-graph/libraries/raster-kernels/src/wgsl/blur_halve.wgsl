fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let parameters = params();
	var sum = vec4<f32>(0.);
	var weight_sum = 0.;
	for (var corner = 0; corner < 4; corner++) {
		let texel = coord * 2 + vec2<i32>(corner % 2, corner / 2);
		let coverage = blur_coverage(texel, parameters.full_size, parameters.scale);
		let weight = coverage.x * coverage.y;
		sum += blur_premultiply(input_0_load(texel), parameters.space) * weight;
		weight_sum += weight;
	}
	return sum / weight_sum;
}
