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

struct Immediates {
	content_origin: vec2<f32>,
	content_size: vec2<f32>,
};

var<immediate> immediates: Immediates;

@group(0) @binding(0)
var t_frame: texture_2d<f32>;
@group(0) @binding(1)
var s_frame: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let sample_pos = in.tex_coords * immediates.content_size;
	let nearest = floor(sample_pos - 0.5) + 0.5;
	let t = sample_pos - nearest;

	let weight_before = t * (-0.5 + t * (1.0 - 0.5 * t));
	let weight_nearest = 1.0 + t * t * (-2.5 + 1.5 * t);
	let weight_next = t * (0.5 + t * (2.0 - 1.5 * t));
	let weight_after = t * t * (-0.5 + 0.5 * t);

	let weight_middle = weight_nearest + weight_next;
	let middle = nearest + weight_next / weight_middle;

	let frame_size = vec2<f32>(textureDimensions(t_frame));
	let content_min = vec2<f32>(0.5);
	let content_max = immediates.content_size - 0.5;
	let uv_before = (immediates.content_origin + clamp(nearest - 1.0, content_min, content_max)) / frame_size;
	let uv_middle = (immediates.content_origin + clamp(middle, content_min, content_max)) / frame_size;
	let uv_after = (immediates.content_origin + clamp(nearest + 2.0, content_min, content_max)) / frame_size;

	var color = vec4<f32>(0.0);
	color += textureSampleLevel(t_frame, s_frame, vec2<f32>(uv_before.x, uv_before.y), 0.0) * weight_before.x * weight_before.y;
	color += textureSampleLevel(t_frame, s_frame, vec2<f32>(uv_middle.x, uv_before.y), 0.0) * weight_middle.x * weight_before.y;
	color += textureSampleLevel(t_frame, s_frame, vec2<f32>(uv_after.x, uv_before.y), 0.0) * weight_after.x * weight_before.y;
	color += textureSampleLevel(t_frame, s_frame, vec2<f32>(uv_before.x, uv_middle.y), 0.0) * weight_before.x * weight_middle.y;
	color += textureSampleLevel(t_frame, s_frame, vec2<f32>(uv_middle.x, uv_middle.y), 0.0) * weight_middle.x * weight_middle.y;
	color += textureSampleLevel(t_frame, s_frame, vec2<f32>(uv_after.x, uv_middle.y), 0.0) * weight_after.x * weight_middle.y;
	color += textureSampleLevel(t_frame, s_frame, vec2<f32>(uv_before.x, uv_after.y), 0.0) * weight_before.x * weight_after.y;
	color += textureSampleLevel(t_frame, s_frame, vec2<f32>(uv_middle.x, uv_after.y), 0.0) * weight_middle.x * weight_after.y;
	color += textureSampleLevel(t_frame, s_frame, vec2<f32>(uv_after.x, uv_after.y), 0.0) * weight_after.x * weight_after.y;

	return clamp(color, vec4<f32>(0.0), vec4<f32>(1.0));
}
