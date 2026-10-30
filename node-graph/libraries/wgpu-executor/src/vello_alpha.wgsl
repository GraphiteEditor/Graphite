// Converts pixels between the straight alpha Graphite's textures hold and the premultiplied alpha Vello GPU renders and samples

struct Parameters {
	// Nonzero when the source texture's format decodes sRGB to linear on load, so it must be encoded back
	source_is_srgb: u32,
	// WebGL2 needs uniform structs to span a multiple of 16 bytes
	_pad_1: u32,
	_pad_2: u32,
	_pad_3: u32,
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
fn unpremultiply_to_linear(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let color = textureLoad(source, vec2<i32>(position.xy), 0);
	if color.a <= 0. {
		return vec4<f32>(0.);
	}

	return vec4<f32>(srgb_to_linear(color.rgb / color.a), color.a);
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

fn srgb_to_linear(srgb: vec3<f32>) -> vec3<f32> {
	let low = srgb / 12.92;
	let high = pow((srgb + 0.055) / 1.055, vec3<f32>(2.4));
	return select(high, low, srgb <= vec3<f32>(0.04045));
}

// A mipmap level from the one above it, each texel averaging a block of two by two, which grows to three along an odd side's last texel
@fragment
fn halve(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let size = vec2<i32>(textureDimensions(source));
	let half_size = max(size / 2, vec2<i32>(1));
	let index = vec2<i32>(position.xy);
	let start = index * 2;
	// Selecting per axis, since WebGL2 can't select between integer vectors
	let last = index + 1 == half_size;
	let end = clamp(vec2<i32>(select(start.x + 2, size.x, last.x), select(start.y + 2, size.y, last.y)), start + 1, size);

	var sum = vec4<f32>(0.);
	for (var y = start.y; y < end.y; y++) {
		for (var x = start.x; x < end.x; x++) {
			sum += textureLoad(source, vec2<i32>(x, y), 0);
		}
	}
	let count = end - start;
	return sum / f32(count.x * count.y);
}
