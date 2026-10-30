// The first image, emptied wherever the second is empty
fn kernel(coord: vec2<i32>) -> vec4<f32> {
	if input_1_load(coord).a <= 0. {
		return vec4<f32>(0.);
	}
	return input_0_load(coord);
}
