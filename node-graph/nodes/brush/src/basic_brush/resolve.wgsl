// =============
// VERTEX SHADER
// =============

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
	let pos = array(
		vec2f(-1.0, -1.0),
		vec2f(3.0, -1.0),
		vec2f(-1.0, 3.0),
	);
	return vec4f(pos[vertex_index], 0.0, 1.0);
}

// ===============
// FRAGMENT SHADER
// ===============

struct Uniforms {
	color: vec4<f32>,
	density_offset: vec2<f32>,
	_pad: vec2<f32>,
};

override RIDGE_GAIN: f32;
override RIDGE_NORM: f32;

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var t_density: texture_2d<f32>;

@group(0) @binding(2)
var t_stamp: texture_2d<f32>;

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
	let texel = vec2<i32>(frag.xy + uniforms.density_offset);
	let field = max(textureLoad(t_density, texel, 0).r, textureLoad(t_stamp, texel, 0).r);
	let alpha = clamp((1.0 - exp(-field * RIDGE_GAIN)) * RIDGE_NORM, 0.0, 1.0) * uniforms.color.a;
	return vec4<f32>(uniforms.color.rgb * alpha, alpha);
}
