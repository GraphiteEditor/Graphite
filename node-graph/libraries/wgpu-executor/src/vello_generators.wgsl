// Renders a tile of a generator's pattern, sampled at the centers of the tile's pixels, as opaque gamma-encoded color like the CPU's tiles

const PATTERN_NOISE: u32 = 0u;
const PATTERN_WHITE_NOISE: u32 = 1u;
const PATTERN_MANDELBROT: u32 = 2u;

// The most iterations a point is tested for escaping the Mandelbrot set, beyond which it counts as inside
const MANDELBROT_ITERATIONS: u32 = 255u;

struct Parameters {
	// Where the tile's top left corner lies in the space the pattern is sampled in, and the size of one of its pixels there
	corner: vec2<f32>,
	texel: vec2<f32>,
	pattern: u32,
	// Whether the noise warps its domain before it's sampled
	domain_warp: u32,
	@align(16) noise: fnl_state,
}

@group(0) @binding(0) var<uniform> parameters: Parameters;

// One triangle that covers the whole target
@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
	let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
	return vec4<f32>(corner * 2. - 1., 0., 1.);
}

@fragment
fn generate(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let point = parameters.corner + position.xy * parameters.texel;

	var luminance: f32;
	switch parameters.pattern {
		case PATTERN_NOISE: {
			luminance = noise(point);
		}
		case PATTERN_WHITE_NOISE: {
			luminance = white_noise(point);
		}
		case PATTERN_MANDELBROT, default: {
			luminance = mandelbrot(point);
		}
	}

	return vec4<f32>(vec3<f32>(linear_to_srgb(luminance)), 1.);
}

fn noise(point: vec2<f32>) -> f32 {
	var x = point.x;
	var y = point.y;
	if parameters.domain_warp != 0u {
		fnlDomainWarp2D(parameters.noise, &x, &y);
	}
	return (fnlGetNoise2D(parameters.noise, x, y) + 1.) * 0.5;
}

// A random value for each square of one unit, matching the CPU's
fn white_noise(point: vec2<f32>) -> f32 {
	let cell = vec2<i32>(floor(point));
	let hash = pcg3d(vec3<u32>(bitcast<vec2<u32>>(cell), bitcast<u32>(parameters.noise.seed)));

	// The top 24 bits, which an f32 holds exactly
	return f32(hash.x >> 8u) / 16777216.;
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

// The fraction of the iteration limit that `c` takes to escape the Mandelbrot set, or 1 if it doesn't
fn mandelbrot(c: vec2<f32>) -> f32 {
	var z = vec2<f32>(0.);
	for (var iteration = 0u; iteration < MANDELBROT_ITERATIONS; iteration++) {
		z = vec2<f32>(z.x * z.x - z.y * z.y, 2. * z.x * z.y) + c;
		if dot(z, z) > 4. {
			return f32(iteration) / f32(MANDELBROT_ITERATIONS);
		}
	}
	return 1.;
}

// Clamped to the range the CPU encodes, which takes NaN as 0
fn linear_to_srgb(unclamped: f32) -> f32 {
	let linear = select(0., min(unclamped, 1.), unclamped >= 0.);
	let low = linear * 12.92;
	let high = 1.055 * pow(linear, 1. / 2.4) - 0.055;
	return select(high, low, linear <= 0.0031308);
}
