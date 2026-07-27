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

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var t_density: texture_2d<f32>;

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
	let density = textureLoad(t_density, vec2<i32>(frag.xy + uniforms.density_offset), 0).r;
	let alpha = clamp(1.0 - exp(-density), 0.0, 1.0) * uniforms.color.a;
	return vec4<f32>(uniforms.color.rgb * alpha, alpha);
}
