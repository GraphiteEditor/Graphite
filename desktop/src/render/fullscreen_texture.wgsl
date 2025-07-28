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

struct Constants {
	viewport_scale: vec2<f32>,
    viewport_offset: vec2<f32>,
};

var<push_constant> constants: Constants;

@group(0) @binding(0)
var t_ui: texture_2d<f32>;
@group(0) @binding(1)
var t_viewport: texture_2d<f32>;
@group(0) @binding(2)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let ui_color: vec4<f32> = textureSample(t_ui, s_diffuse, in.tex_coords);
	if (ui_color.a == 1.0) {
		return ui_color;
	}
	let viewport_tex_coords = (in.tex_coords - constants.viewport_offset) * constants.viewport_scale;
	let viewport_color: vec4<f32> = textureSample(t_viewport, s_diffuse, viewport_tex_coords);
	return ui_color * ui_color.a + viewport_color * (1.0 - ui_color.a);
}
