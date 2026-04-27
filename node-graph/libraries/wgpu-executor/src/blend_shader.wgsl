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

@group(0) @binding(0)
var t_foreground: texture_2d<f32>;
@group(0) @binding(1)
var t_background: texture_2d<f32>;
@group(0) @binding(2)
var s_linear: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let foreground = textureSample(t_foreground, s_linear, in.tex_coords);
	let background = textureSample(t_background, s_linear, in.tex_coords);

	let a = foreground.a + background.a * (1.0 - foreground.a);
	let rgb = foreground.rgb * foreground.a + background.rgb * background.a * (1.0 - foreground.a);
	return vec4<f32>(rgb, a);
}
