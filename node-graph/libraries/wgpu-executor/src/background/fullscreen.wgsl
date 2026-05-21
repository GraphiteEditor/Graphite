@group(0) @binding(0)
var foreground_sampler: sampler;

@group(0) @binding(1)
var foreground_texture: texture_2d<f32>;

struct VertexOutput {
	@builtin(position) position: vec4<f32>,
	@location(0) tex_coord: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
	let positions = array<vec2<f32>, 3>(
		vec2<f32>(-1.0, -1.0),
		vec2<f32>(-1.0,  3.0),
		vec2<f32>( 3.0, -1.0),
	);

	let tex_coords = array<vec2<f32>, 3>(
		vec2<f32>(0.0, 1.0),
		vec2<f32>(0.0, -1.0),
		vec2<f32>(2.0, 1.0),
	);

	var vertex_out: VertexOutput;
	vertex_out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
	vertex_out.tex_coord = tex_coords[vertex_index];
	return vertex_out;
}

@fragment
fn fs_main(fragment_in: VertexOutput) -> @location(0) vec4<f32> {
	return textureSample(foreground_texture, foreground_sampler, fragment_in.tex_coord);
}
