// The straight-alpha linear-light content, sampled where the output pixel lies, as the premultiplied gamma-encoded channels the effects are carried out in, faded by `amount`
fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let color = input_0_at(coord);
	return vec4<f32>(linear_to_srgb(max(color.rgb, vec3<f32>(0.))) * color.a, color.a) * params().amount;
}
