fn map_color(color: vec4<f32>) -> vec4<f32> {
	let parameters = params();
	let channels = to_gamma_srgb_channels(color);
	return from_gamma_srgb_channels(vec4<f32>(levels_curve_apply(parameters.red, channels.r), levels_curve_apply(parameters.green, channels.g), levels_curve_apply(parameters.blue, channels.b), channels.a));
}
