fn map_color(color: vec4<f32>) -> vec4<f32> {
	let parameters = params();
	let channels = to_gamma_srgb_channels(color);
	let r = channels.r;
	let g = channels.g;
	let b = channels.b;

	var mixed = vec3<f32>(0.);
	if parameters.monochrome {
		let gray = clamp(r * parameters.monochrome_r + g * parameters.monochrome_g + b * parameters.monochrome_b + parameters.monochrome_c, 0., 1.);

		mixed = vec3<f32>(gray);
	} else {
		let red = clamp(r * parameters.red_r + g * parameters.red_g + b * parameters.red_b + parameters.red_c, 0., 1.);
		let green = clamp(r * parameters.green_r + g * parameters.green_g + b * parameters.green_b + parameters.green_c, 0., 1.);
		let blue = clamp(r * parameters.blue_r + g * parameters.blue_g + b * parameters.blue_b + parameters.blue_c, 0., 1.);

		mixed = vec3<f32>(red, green, blue);
	}

	return from_gamma_srgb_channels(vec4<f32>(mixed, channels.a));
}
