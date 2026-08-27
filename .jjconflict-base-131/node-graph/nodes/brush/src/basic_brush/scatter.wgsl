override CUTOFF_SIGMA: f32;

override LUT_SIZE: f32;
override LUT_V_MAX: f32;
override LUT_T_MAX: f32;

// =============
// VERTEX SHADER
// =============

struct Uniforms {
	frame_size: vec2<f32>,
	kernel_scale: f32,
	kernel_exponent: f32,
	kernel_section_scale: f32,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var t_kernel: texture_2d<f32>;

@group(0) @binding(2)
var s_kernel: sampler;

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

fn sweep(v: f32, t: f32) -> f32 {
	let texel = (LUT_SIZE - 1.0) / LUT_SIZE;
	let uv = vec2f(
		((t + LUT_T_MAX) / (2.0 * LUT_T_MAX)) * texel + 0.5 / LUT_SIZE,
		(v / LUT_V_MAX) * texel + 0.5 / LUT_SIZE,
	);
	return textureSampleLevel(t_kernel, s_kernel, uv, 0.0).r;
}

fn section(r2: f32) -> f32 {
	// Max avoids pow undefined log at zero.
	return exp(-pow(max(r2 * 0.5, 1e-20), uniforms.kernel_exponent));
}

struct FragmentOutput {
	@location(0) density: f32,
	@location(1) stamp: f32,
};

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
	let p = in.clip_position.xy;
	let inv_section = uniforms.kernel_section_scale / in.sigma;
	let d = in.b - in.a;
	let len = length(d);
	if (len < 1e-6) {
		let dab = in.weight * section(dot(p - in.a, p - in.a) * inv_section * inv_section);
		return FragmentOutput(dab, dab);
	}
	let e = d / len;
	let rel = p - in.a;
	let along = dot(rel, e);
	let perp2 = max(dot(rel, rel) - along * along, 0.0);

	let cutoff = CUTOFF_SIGMA * in.sigma;
	if (perp2 > cutoff * cutoff || along < -cutoff || along > len + cutoff) {
		return FragmentOutput(0.0, 0.0);
	}
	let inv_sp = uniforms.kernel_scale / in.sigma;
	let v = sqrt(perp2) * inv_sp;
	let ridge = sweep(v, along * inv_sp) - sweep(v, (along - len) * inv_sp);

	let overhang = max(max(-along, along - len), 0.0);
	let stamp = in.weight * section((perp2 + overhang * overhang) * inv_section * inv_section);

	return FragmentOutput(in.weight * max(ridge, 0.0), stamp);
}
