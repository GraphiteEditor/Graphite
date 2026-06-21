struct Globals {
	viewport_size: vec2<f32>,
	_pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> globals: Globals;

struct VertexOutput {
	@builtin(position) clip_position: vec4<f32>,
	@location(0) center_px: vec2<f32>,
	@location(1) radius_px: f32,
};

@vertex
fn vs_main(
	@builtin(vertex_index) vertex_index: u32,
	@location(0) center_px: vec2<f32>,
	@location(1) radius_px: f32,
) -> VertexOutput {
	let viewport = globals.viewport_size;

	let offset_x = f32(i32(vertex_index) - 1) * 2.0;
	let offset_y = f32(i32(vertex_index & 1u) * 2 - 1) * 2.0;

	let pos_px = center_px + vec2<f32>(offset_x, offset_y) * radius_px;

	let clip = vec2<f32>(
		2.0 * pos_px.x / viewport.x - 1.0,
		1.0 - 2.0 * pos_px.y / viewport.y,
	);

	var out: VertexOutput;
	out.clip_position = vec4<f32>(clip, 0.0, 1.0);
	out.center_px = center_px;
	out.radius_px = radius_px;
	return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let frag_px = in.clip_position.xy;
	let d = distance(frag_px, in.center_px);
	if (d > in.radius_px) {
		discard;
	}
	let t = d / in.radius_px;
	let alpha = 1.0 - smoothstep(0.0, 1.0, t);
	return vec4<f32>(0.0, 0.0, 0.0, alpha);
}
