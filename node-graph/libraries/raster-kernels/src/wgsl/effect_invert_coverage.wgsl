fn kernel(coord: vec2<i32>) -> vec4<f32> {
	return vec4<f32>(1. - input_0_load(coord).a);
}
