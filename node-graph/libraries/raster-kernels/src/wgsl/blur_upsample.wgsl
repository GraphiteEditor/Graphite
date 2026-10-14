fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let parameters = params();
	return blur_unpremultiply(input_0_sample((vec2<f32>(coord) + 0.5) / parameters.scale), parameters.space);
}
