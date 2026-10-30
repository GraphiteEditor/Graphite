@group(0) @binding(BINDING) var LUT_texture: texture_3d<f32>;

fn LUT_size() -> i32 {
	return i32(textureDimensions(LUT_texture).x);
}

fn LUT_load(index: vec3<i32>) -> vec4<f32> {
	return textureLoad(LUT_texture, clamp(index, vec3<i32>(0), vec3<i32>(LUT_size() - 1)), 0);
}
