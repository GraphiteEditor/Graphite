fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let parameters = params();
	let noise = parameters.noise;
	let state = fnl_state(
		noise.seed,
		noise.frequency,
		noise.noise_type,
		noise.rotation_type_3d,
		noise.fractal_type,
		noise.octaves,
		noise.lacunarity,
		noise.gain,
		noise.weighted_strength,
		noise.ping_pong_strength,
		noise.cellular_distance_func,
		noise.cellular_return_type,
		noise.cellular_jitter_mod,
		noise.domain_warp_type,
		noise.domain_warp_amp,
	);

	var x = f32(coord.x) * parameters.texel[0] + parameters.corner[0];
	var y = f32(coord.y) * parameters.texel[1] + parameters.corner[1];
	if parameters.domain_warp {
		fnlDomainWarp2D(state, &x, &y);
	}

	let luminance = (fnlGetNoise2D(state, x, y) + 1.) * 0.5;
	return vec4<f32>(vec3<f32>(luminance), 1.);
}
