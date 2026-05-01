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
    let document_corners = array<vec2<f32>, 6>(
        uniforms.rect_min,
        vec2<f32>(uniforms.rect_max.x, uniforms.rect_min.y),
        vec2<f32>(uniforms.rect_min.x, uniforms.rect_max.y),
        vec2<f32>(uniforms.rect_min.x, uniforms.rect_max.y),
        vec2<f32>(uniforms.rect_max.x, uniforms.rect_min.y),
        uniforms.rect_max,
    );
    let document_position = document_corners[vertex_index];

    let transformed = uniforms.transform_x * document_position.x + uniforms.transform_y * document_position.y + uniforms.transform_translation;
    let normalized = transformed / uniforms.viewport_size;
    let clip = vec2<f32>(normalized.x * 2.0 - 1.0, 1.0 - normalized.y * 2.0);

    var out: VertexOutput;
    out.position = vec4<f32>(clip, 0.0, 1.0);
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
