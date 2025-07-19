struct VertexOutput {
	@builtin(position) clip_position: vec4<f32>,
	@location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
	var out: VertexOutput;

	let pos = array(
		// 1st triangle
		vec2f( -1.0, -1.0), // center
		vec2f( 1.0, -1.0), // right, center
		vec2f( -1.0, 1.0), // center, top

		// 2nd triangle
		vec2f( -1.0, 1.0), // center, top
		vec2f( 1.0, -1.0), // right, center
		vec2f( 1.0, 1.0), // right, top
	);
	let xy = pos[vertex_index];
	out.clip_position = vec4f(xy , 0.0, 1.0);
	let coords = (xy / 2. + 0.5);
	out.tex_coords = vec2f(coords.x, 1. - coords.y);
	return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}
