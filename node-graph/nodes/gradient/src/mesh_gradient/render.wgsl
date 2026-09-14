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

// =======
// Shaders
// =======

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

    let color_in_selected_space = evaluate_channels(patch_index, u, v);
    return convert_to_gamma_srgb(color_in_selected_space);
};

// FIXME: only for debug
@fragment
fn fs_debug_outline() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
};

// ================
// Color evaluators
// ================

fn evaluate_channels(patch_index: u32, u: f32, v: f32) -> vec4<f32> {
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

// ======================
// Color space converters
// ======================

fn convert_to_gamma_srgb(color: vec4<f32>) -> vec4<f32> {
    var linear_srgb: vec3<f32>;
    switch metadata.interpolation_space {
        // Gamma sRGB
        case 0: {
            return color;
        }
        // Linear sRGB
        case 1: {
            linear_srgb = color.rgb;
        }
        // OKLab
        case 2: {
            linear_srgb = oklab_to_linear_srgb(color.rgb);
        }
        // Lab
        case 3: {
            linear_srgb = lab_to_linear_srgb(color.rgb);
        }
        default: {
            return color;
        }
    }
    return vec4<f32>(linear_srgb_to_gamma_srgb(linear_srgb), color.a);
}

// The following color-conversion functions are adapted from
// color 0.3.3's colorspace.rs:
// https://github.com/linebender/color
//
// Copyright 2024 the Color Authors.
// Licensed under Apache-2.0.
// Modifications: Translated from Rust to WGSL and adapted for Graphite.

const OKLAB_LAB_TO_LMS = mat3x3<f32>(
    vec3<f32>(1.0, 1.0, 1.0),
    vec3<f32>(0.39633778, -0.105561346, -0.08948418),
    vec3<f32>(0.21580376, -0.06385417, -1.2914855),
);

const OKLAB_LMS_TO_SRGB = mat3x3<f32>(
    vec3<f32>(4.0767417, -1.268438, -0.0041960863),
    vec3<f32>(-3.3077116, 2.6097574, -0.7034186),
    vec3<f32>(0.23096994, -0.34131938, 1.7076147),
);

const LAB_XYZ_TO_SRGB = mat3x3<f32>(
    vec3<f32>(3.0222337, -0.94384825, 0.06938627),
    vec3<f32>(-1.617386, 1.9162544, -0.22897676),
    vec3<f32>(-0.40484765, 0.027593868, 1.1595905),
);

const LAB_KAPPA: f32 = 24389.0 / 27.0;
const LAB_EPSILON_CBRT: f32 = 0.20689656;

fn oklab_to_linear_srgb(src: vec3<f32>) -> vec3<f32> {
    var lms = OKLAB_LAB_TO_LMS * src;
    lms = lms * lms * lms;
    return OKLAB_LMS_TO_SRGB * lms;
};

fn lab_to_linear_srgb(src: vec3<f32>) -> vec3<f32> {
    let f1 = src.x * (1.0 / 116.0) + (16.0 / 116.0);
    let f0 = src.y * (1.0 / 500.0) + f1;
    let f2 = f1 - src.z * (1.0 / 200.0);
    let f = vec3<f32>(f0, f1, f2);
    let xyz = select(
        (116.0 / LAB_KAPPA) * f - vec3<f32>(16.0 / LAB_KAPPA),
        f * f * f,
        f > vec3<f32>(LAB_EPSILON_CBRT),
    );
    return LAB_XYZ_TO_SRGB * xyz;
}

fn linear_srgb_to_gamma_srgb(src: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        lin_to_srgb(src.x),
        lin_to_srgb(src.y),
        lin_to_srgb(src.z),
    );
}

fn lin_to_srgb(x: f32) -> f32 {
    if abs(x) <= 0.0031308 {
        return x * 12.92;
    } else {
        let magnitude = 1.055 * pow(abs(x), 1.0 / 2.4) - 0.055;
        return select(abs(magnitude), -abs(magnitude), x < 0.0);
    }
}
