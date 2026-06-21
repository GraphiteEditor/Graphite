// =============
// VERTEX SHADER
// =============

struct VertexOutput {
	@builtin(position) clip_position: vec4<f32>,
	@location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
	var out: VertexOutput;
	let pos = array(
		vec2f(-1.0, -1.0),
		vec2f(3.0, -1.0),
		vec2f(-1.0, 3.0),
	);
	let xy = pos[vertex_index];
	out.clip_position = vec4f(xy, 0.0, 1.0);
	let coords = xy / 2. + 0.5;
	out.tex_coords = vec2f(coords.x, 1. - coords.y);
	return out;
}

// ===============
// FRAGMENT SHADER
// ===============

@group(0) @binding(0)
var t_source: texture_2d<f32>;

struct Params {
	matrix: mat2x2<f32>,
	translation: vec2<f32>,
	_pad: vec2<f32>,
};

// We need to use a uniform buffer for the params because push constants are not supported on web
@group(0) @binding(1)
var<uniform> params: Params;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let position = params.matrix * in.tex_coords + params.translation;
	let texel = vec2<i32>(floor(position));
	let texture_size = vec2<i32>(textureDimensions(t_source));
	if (texel.x >= 0 && texel.x < texture_size.x && texel.y >= 0 && texel.y < texture_size.y) {
		return textureLoad(t_source, texel, 0);
	}
	return vec4<f32>(0.0);
}
