// Premultiplied gamma-encoded channels back into a straight-alpha linear-light color
fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let color = input_0_load(coord);
	if color.a <= 0. {
		return vec4<f32>(0.);
	}
	return vec4<f32>(srgb_to_linear(max(color.rgb / color.a, vec3<f32>(0.))), color.a);
}
