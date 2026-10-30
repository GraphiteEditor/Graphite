// The blend modes work on straight linear colors, as they do for a solid color overlay
fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let parameters = params();
	let image = input_0_load(coord);
	let paint = input_1_load(coord);
	if image.a <= 0. {
		return vec4<f32>(0.);
	}
	if paint.a <= 0. {
		return image;
	}

	let backdrop = vec4<f32>(srgb_to_linear(image.rgb / image.a), 1.);
	let blended = apply_blend_mode(vec4<f32>(paint.rgb, 1.), backdrop, parameters.blend_mode);
	let mixed = mix(backdrop.rgb, blended.rgb, parameters.opacity * paint.a);

	return vec4<f32>(linear_to_srgb(mixed) * image.a, image.a);
}
