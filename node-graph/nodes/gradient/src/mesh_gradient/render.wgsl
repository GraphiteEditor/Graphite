struct InterpolationSetting {
    space: u32,
    method: u32,
}

struct PatchData {
    colors: array<vec4<f32>, 4>,
    color_u_derivatives: array<vec4<f32>, 4>,
    color_v_derivatives: array<vec4<f32>, 4>,
};

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
var<storage, read> patches: array<PatchData>;
@group(0) @binding(1)
var<uniform> interpolation_setting: InterpolationSetting;

fn evaluate_color(patch_index: u32, uv: vec2<f32>) -> vec4<f32> {
    let patch_data = patches[patch_index];
    let top = mix(patch_data.colors[0], patch_data.colors[1], uv.x);
    let bottom = mix(patch_data.colors[2], patch_data.colors[3], uv.x);
    return mix(top, bottom, uv.y);
};

fn hermite(a: vec4<f32>, ma: vec4<f32>, b: vec4<f32>, mb: vec4<f32>, t: f32) -> vec4<f32> {
    let t_power_2 = t * t;
    let t_power_3 = t_power_2 * t;

    let h1 = 2. * t_power_3 - 3. * t_power_2 + 1.;
    let h2 = -2. * t_power_3 + 3. * t_power_2;
    let h3 = t_power_3 - 2. * t_power_2 + t;
    let h4 = t_power_3 - t_power_2;

    return ma * h3 + a * h1 + b * h2 + mb * h4;
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    let clip_position = vec2<f32>(
        input.position.x * 2.0 - 1.0,
        1.0 - input.position.y * 2.0
    );

    // Following PostScript's priority order (from higher): Patch index -> Local u -> Local v
    let local_priority = (input.uv.y * 100 + input.uv.x) / 102;
    let z_position = (f32(input.patch_index) + local_priority) / f32(arrayLength(&patches));
    output.position = vec4<f32>(clip_position, z_position, 1.0);
    output.patch_index = input.patch_index;
    output.uv = input.uv;

    return output;
};

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let u = input.uv.x;
    let v = input.uv.y;
    let patch_data = patches[input.patch_index];
    let colors = patch_data.colors;
    let u_deriv = patch_data.color_u_derivatives;
    let v_deriv = patch_data.color_v_derivatives;

    let top = hermite(colors[0], u_deriv[0], colors[1], u_deriv[1], u);
    let top_deriv = hermite(v_deriv[0], vec4(0), v_deriv[1], vec4(0), u);
    let bottom = hermite(colors[2], u_deriv[2], colors[3], u_deriv[3], u);
    let bottom_deriv = hermite(v_deriv[2], vec4(0), v_deriv[3], vec4(0), u);

    return hermite(top, top_deriv, bottom, bottom_deriv, v);
};

// FIXME: only for debug
@fragment
fn fs_debug_outline() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
};
