@group(0) @binding(BINDING) var LUT_texture: texture_2d<f32>;

fn LUT(t: f32) -> vec4<f32> {
	let last = i32(textureDimensions(LUT_texture).x) - 1;
	let scaled = clamp(t, 0., 1.) * f32(last);
	let lower = min(i32(floor(scaled)), last);
	let upper = min(lower + 1, last);
	let lower_entry = textureLoad(LUT_texture, vec2<i32>(lower, 0), 0);
	let upper_entry = textureLoad(LUT_texture, vec2<i32>(upper, 0), 0);
	return lower_entry + (scaled - f32(lower)) * (upper_entry - lower_entry);
}

fn LUT_stepped(t: f32) -> vec4<f32> {
	let last = i32(textureDimensions(LUT_texture).x) - 1;
	return textureLoad(LUT_texture, vec2<i32>(min(i32(floor(clamp(t, 0., 1.) * f32(last))), last), 0), 0);
}
