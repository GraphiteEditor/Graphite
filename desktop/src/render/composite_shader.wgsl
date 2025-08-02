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
var t_viewport: texture_2d<f32>;
@group(0) @binding(1)
var t_overlays: texture_2d<f32>;
@group(0) @binding(2)
var t_ui: texture_2d<f32>;
@group(0) @binding(3)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let ui = textureSample(t_ui, s_diffuse, in.tex_coords);
	if (ui.a >= 0.999) {
		return ui;
	}

	let vp_cord = (in.tex_coords - constants.viewport_offset) * constants.viewport_scale;

	let ov = textureSample(t_overlays, s_diffuse, vp_cord);
	let vp = textureSample(t_viewport, s_diffuse, vp_cord);

	if (ov.a < 0.001) {
		return blend(ui, vp);
	}

	let comp = blend(ov, vp);
	return blend(ui, comp);
}

fn blend(fg: vec4<f32>, bg: vec4<f32>) -> vec4<f32> {
	let a = fg.a + bg.a * (1.0 - fg.a);
	let rgb = fg.rgb * fg.a + bg.rgb * bg.a * (1.0 - fg.a);
	return vec4<f32>(rgb, a);
}
