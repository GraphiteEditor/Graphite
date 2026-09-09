struct Metadata {
    patch_count: u32,
    interpolation_space: u32,
    interpolation_method: u32,
}

struct VertexInput {
    @location(0) @interpolate(flat) patch_index: u32,
    @location(1) uv: vec2<f32>,
    @location(2) position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) patch_index: u32,
    @location(1) uv: vec2<f32>,
};

@group(0) @binding(0)
var<storage, read> color_data: array<vec4<f32>>;
@group(0) @binding(1)
var<uniform> metadata: Metadata;

fn evaluate_bilinear_color_from_buffer(base_index: u32, u: f32, v: f32) -> vec4<f32> {
    let top = mix(color_data[base_index], color_data[base_index + 1], u);
    let bottom = mix(color_data[base_index + 2], color_data[base_index + 3], u);
    return mix(top, bottom, v);
};

fn evaluate_cubic_bezier_color_from_buffer(base_index: u32, time: f32) -> vec4<f32> {
    let p0 = color_data[base_index];
    let p1 = color_data[base_index + 1];
    let p2 = color_data[base_index + 2];
    let p3 = color_data[base_index + 3];
    let one_minus_time = 1.0 - time;
    return p0 * one_minus_time * one_minus_time * one_minus_time + p1 * 3.0 * time * one_minus_time * one_minus_time + p2 * 3 * time * time * one_minus_time + p3 * time * time * time;
};

fn evaluate_cubic_bezier_color(control_points: array<vec4<f32>, 4>, time: f32) -> vec4<f32> {
    let p0 = control_points[0];
    let p1 = control_points[1];
    let p2 = control_points[2];
    let p3 = control_points[3];
    let one_minus_time = 1.0 - time;
    return p0 * one_minus_time * one_minus_time * one_minus_time + p1 * 3.0 * time * one_minus_time * one_minus_time + p2 * 3 * time * time * one_minus_time + p3 * time * time * time;
};

fn evaluate_bicubic_bezier_color_from_buffer(base_index: u32, u: f32, v: f32) -> vec4<f32> {
    let row0 = evaluate_cubic_bezier_color_from_buffer(base_index, u);
    let row1 = evaluate_cubic_bezier_color_from_buffer(base_index + 4, u);
    let row2 = evaluate_cubic_bezier_color_from_buffer(base_index + 8, u);
    let row3 = evaluate_cubic_bezier_color_from_buffer(base_index + 12, u);
    return evaluate_cubic_bezier_color(array(row0, row1, row2, row3), v);
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    let clip_position = vec2<f32>(
        input.position.x * 2.0 - 1.0,
        1.0 - input.position.y * 2.0
    );

    // Following PostScript's priority order (from higher): Patch index -> Local v -> Local u
    let local_priority = (input.uv.y * 1000 + input.uv.x) / 1002;
    let z_position = (f32(input.patch_index) + local_priority) / f32(metadata.patch_count);
    output.position = vec4<f32>(clip_position, z_position, 1.0);
    output.patch_index = input.patch_index;
    output.uv = input.uv;

    return output;
};

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let u = input.uv.x;
    let v = input.uv.y;
    let patch_index = input.patch_index;

    switch metadata.interpolation_method {
        // Stepped
        case 0: {
            return color_data[patch_index];
        }
        // Linear
        case 1: {
            let base_index = patch_index * 4;
            return evaluate_bilinear_color_from_buffer(base_index, u, v);
        }
        // Bicubic
        case 2: {
            let base_index = patch_index * 16;
            return evaluate_bicubic_bezier_color_from_buffer(base_index, u, v);
        }
        default {
            return color_data[patch_index];
        }
    }
};

// FIXME: only for debug
@fragment
fn fs_debug_outline() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
};
