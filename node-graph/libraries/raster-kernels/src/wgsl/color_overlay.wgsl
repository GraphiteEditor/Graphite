fn map_color(color: vec4<f32>) -> vec4<f32> {
	let parameters = params();
	let overlay = apply_blend_mode(parameters.color, color, parameters.blend_mode);

	return vec4<f32>(color.rgb + (overlay.rgb - color.rgb) * parameters.opacity, color.a);
}
