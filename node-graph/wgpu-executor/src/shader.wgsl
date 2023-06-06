// Vertex shader

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coords = model.tex_coords;
    out.clip_position = vec4<f32>(model.position, 1.0);
    return out;
}

// Fragment shader

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0)@binding(1)
var s_diffuse: sampler;

fn linearToSRGB(color: vec3<f32>) -> vec3<f32> {
    let a = 0.055;
    return select(pow(color, vec3<f32>(1.0 / 2.2)) * (1.0 + a) - a,
                  color / 12.92,
                  color <= vec3<f32>(0.0031308));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color =  textureSample(t_diffuse, s_diffuse, in.tex_coords);
	var linearColor = color.rgb;
    var srgbColor = linearToSRGB(linearColor);
    return vec4<f32>(srgbColor, color.a);
}
