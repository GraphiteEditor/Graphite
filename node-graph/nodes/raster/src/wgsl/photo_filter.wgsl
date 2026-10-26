const SRGB_TO_XYZ_D50 = mat3x3<f32>(vec3<f32>(0.43607, 0.38515, 0.14307), vec3<f32>(0.22249, 0.71687, 0.06061), vec3<f32>(0.01392, 0.09708, 0.71410));
const XYZ_D50_TO_SRGB = mat3x3<f32>(vec3<f32>(3.134096, -1.6174, -0.490638), vec3<f32>(-0.978793, 1.916295, 0.033454), vec3<f32>(0.071971, -0.228987, 1.40538));

fn multiply_matrix(matrix: mat3x3<f32>, vector: vec3<f32>) -> vec3<f32> {
	return vector * matrix;
}

fn map_color(color: vec4<f32>) -> vec4<f32> {
	let parameters = params();
	let channels = to_gamma_srgb_channels(color);
	let xyz = multiply_matrix(SRGB_TO_XYZ_D50, srgb_to_linear(channels.rgb));
	let filtered = multiply_matrix(XYZ_D50_TO_SRGB, xyz * vec3<f32>(parameters.factor[0], parameters.factor[1], parameters.factor[2]));
	var rgb = linear_to_srgb(clamp(filtered, vec3<f32>(0.), vec3<f32>(1.)));

	if parameters.preserve_luminosity {
		let luma_in = luma_rec_601_fixed_point(clamp(channels.rgb, vec3<f32>(0.), vec3<f32>(1.)));
		rgb = set_luminosity(rgb, luma_rec_601_fixed_point(rgb), luma_in);
	}

	return from_gamma_srgb_channels(vec4<f32>(rgb, channels.a));
}
