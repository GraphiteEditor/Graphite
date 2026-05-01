struct CompositeUniforms {
	transform_x: vec2<f32>,
	transform_y: vec2<f32>,
	transform_translation: vec2<f32>,
	rect_min: vec2<f32>,
	rect_max: vec2<f32>,
	viewport_size: vec2<f32>,
	pattern_origin: vec2<f32>,
	checker_size: f32,
	_pad: f32,
};

@group(0) @binding(0)
var<uniform> uniforms: CompositeUniforms;

struct VertexOutput {
	@builtin(position) position: vec4<f32>,
	@location(0) document_position: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(-1.0,  3.0),
        vec2<f32>( 3.0, -1.0),
    );
    let position = positions[vertex_index];

    let screen_position = vec2<f32>((position.x + 1.0) * 0.5 * uniforms.viewport_size.x, (1.0 - position.y) * 0.5 * uniforms.viewport_size.y);
    let document_position = uniforms.transform_x * screen_position.x + uniforms.transform_y * screen_position.y + uniforms.transform_translation;

	var out: VertexOutput;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.document_position = document_position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tile = floor((in.document_position - uniforms.pattern_origin) / uniforms.checker_size);
    let parity = i32(tile.x + tile.y) & 1;
    let luminance = vec3<f32>(select(1.0, 0.8, parity == 1));
    return vec4<f32>(luminance, 1.0);
}
