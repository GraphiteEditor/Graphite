// Grows the shapes by one step along an axis, from the coverage placed at the pixel and one step to either side, so steps that double each time grow them by any distance in a few passes
fn kernel(coord: vec2<i32>) -> vec4<f32> {
	return vec4<f32>(max(input_0_at(coord).a, max(input_1_at(coord).a, input_2_at(coord).a)));
}
