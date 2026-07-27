override CUTOFF_SIGMA: f32;

// =============
// VERTEX SHADER
// =============

struct Uniforms {
	frame_size: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexOutput {
	@builtin(position) clip_position: vec4<f32>,
	@location(0) @interpolate(flat) a: vec2<f32>,
	@location(1) @interpolate(flat) b: vec2<f32>,
	@location(2) @interpolate(flat) sigma: f32,
	@location(3) @interpolate(flat) weight: f32,
};

@vertex
fn vs_main(
	@builtin(vertex_index) vertex_index: u32,
	@location(0) a: vec2<f32>,
	@location(1) b: vec2<f32>,
	@location(2) sigma: f32,
	@location(3) weight: f32,
) -> VertexOutput {
	let cutoff = CUTOFF_SIGMA * sigma;

	let d = b - a;
	let len = length(d);
	var e = vec2f(1.0, 0.0);
	if (len > 1e-6) {
		e = d / len;
	}
	let n = vec2f(-e.y, e.x);
	let base = select(a - e * cutoff, b + e * cutoff, (vertex_index & 1u) == 1u);
	let normal_sign = select(-1.0, 1.0, vertex_index >= 2u);
	let corner = base + n * (cutoff * normal_sign);

	let ndc = vec2f(corner.x / uniforms.frame_size.x * 2.0 - 1.0, 1.0 - corner.y / uniforms.frame_size.y * 2.0);

	var out: VertexOutput;
	out.clip_position = vec4f(ndc, 0.0, 1.0);
	out.a = a;
	out.b = b;
	out.sigma = sigma;
	out.weight = weight;
	return out;
}

// ===============
// FRAGMENT SHADER
// ===============

const PI: f32 = 3.14159265358979;
const SQRT2: f32 = 1.4142135623;

// Error function approximation see https://arxiv.org/abs/1201.1320
fn erf(x: f32) -> f32 {
	let a = 0.147;
	let x2 = x * x;
	let inner = (4.0 / PI + a * x2) / (1.0 + a * x2);
	return sign(x) * sqrt(1.0 - exp(-x2 * inner));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) f32 {
	let p = in.clip_position.xy;
	let sigma = in.sigma;
	let d = in.b - in.a;
	let len = length(d);
	if (len < 1e-6) {
		let dab = exp(-dot(p - in.a, p - in.a) / (2.0 * sigma * sigma));
		return in.weight * dab;
	}
	let e = d / len;
	let rel = p - in.a;
	let along = dot(rel, e);
	let perp2 = max(dot(rel, rel) - along * along, 0.0);

	let cutoff = CUTOFF_SIGMA * sigma;
	if (perp2 > cutoff * cutoff || along < -cutoff || along > len + cutoff) {
		return 0.0;
	}
	let radial = exp(-perp2 / (2.0 * sigma * sigma));
	let envelope = 0.5 * (erf(along / (SQRT2 * sigma)) + erf((len - along) / (SQRT2 * sigma)));
	return in.weight * (radial * envelope);
}
