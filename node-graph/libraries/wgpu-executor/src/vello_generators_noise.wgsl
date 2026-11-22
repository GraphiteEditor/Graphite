struct Parameters {
	corner: vec2<f32>,
	texel: vec2<f32>,
	// Whether the noise warps its domain before it's sampled
	domain_warp: u32,
	@align(16) noise: fnl_state,
}

@group(0) @binding(0) var<uniform> parameters: Parameters;

@fragment
fn generate(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let point = parameters.corner + position.xy * parameters.texel;

	var x = point.x;
	var y = point.y;
	if parameters.domain_warp != 0u {
		fnlDomainWarp2D(parameters.noise, &x, &y);
	}
	let luminance = (fnlGetNoise2D(parameters.noise, x, y) + 1.) * 0.5;

	return vec4<f32>(vec3<f32>(linear_to_srgb(luminance)), 1.);
}
