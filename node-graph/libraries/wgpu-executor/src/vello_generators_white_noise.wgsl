// The start of the noise shader's noise state, laid out the same way in every backend's shading language as a struct of its own
struct NoiseState {
	seed: i32,
}

struct Parameters {
	corner: vec2<f32>,
	texel: vec2<f32>,
	_domain_warp: u32,
	@align(16) noise: NoiseState,
}

@group(0) @binding(0) var<uniform> parameters: Parameters;

// A random value for each square of one unit, matching the CPU's
@fragment
fn generate(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let point = parameters.corner + position.xy * parameters.texel;

	let cell = vec2<i32>(floor(point));
	let hash = pcg3d(vec3<u32>(bitcast<vec2<u32>>(cell), bitcast<u32>(parameters.noise.seed)));

	// The top 24 bits, which an f32 holds exactly
	let luminance = f32(hash.x >> 8u) / 16777216.;

	return vec4<f32>(vec3<f32>(linear_to_srgb(luminance)), 1.);
}

// PCG3D from Jarzynski and Olano's "Hash Functions for GPU Rendering", which spreads every input bit across the result
fn pcg3d(input: vec3<u32>) -> vec3<u32> {
	var v = input * 1664525u + 1013904223u;
	v.x += v.y * v.z;
	v.y += v.z * v.x;
	v.z += v.x * v.y;
	v ^= v >> vec3<u32>(16u);
	v.x += v.y * v.z;
	v.y += v.z * v.x;
	v.z += v.x * v.y;
	return v;
}
