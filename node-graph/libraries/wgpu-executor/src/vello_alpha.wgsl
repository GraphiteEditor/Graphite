// Converts pixels between the straight alpha Graphite's textures hold and the premultiplied alpha Vello GPU renders and samples

struct Parameters {
	// Nonzero when the source texture's format decodes sRGB to linear on load, so it must be encoded back
	source_is_srgb: u32,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var<uniform> parameters: Parameters;

// One triangle that covers the whole target
@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
	let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
	return vec4<f32>(corner * 2. - 1., 0., 1.);
}

@fragment
fn unpremultiply(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let color = textureLoad(source, vec2<i32>(position.xy), 0);
	if color.a <= 0. {
		return vec4<f32>(0.);
	}

	return vec4<f32>(color.rgb / color.a, color.a);
}

@fragment
fn premultiply(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	var color = textureLoad(source, vec2<i32>(position.xy), 0);
	if parameters.source_is_srgb != 0u {
		color = vec4<f32>(linear_to_srgb(color.rgb), color.a);
	}

	return vec4<f32>(color.rgb * color.a, color.a);
}

fn linear_to_srgb(linear: vec3<f32>) -> vec3<f32> {
	let low = linear * 12.92;
	let high = 1.055 * pow(linear, vec3<f32>(1. / 2.4)) - 0.055;
	return select(high, low, linear <= vec3<f32>(0.0031308));
}
