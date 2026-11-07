
@group(0) @binding(BINDING) var INPUT_texture: texture_2d<f32>;

fn INPUT_size() -> vec2<i32> {
	return vec2<i32>(kernel_frame[PLACEMENT].zw);
}

fn INPUT_load(coord: vec2<i32>) -> vec4<f32> {
	return textureLoad(INPUT_texture, clamp(coord, vec2<i32>(0), INPUT_size() - 1), 0);
}

fn INPUT_premultiplied(texel: vec2<i32>) -> vec4<f32> {
	if any(texel < vec2<i32>(0)) || any(texel >= INPUT_size()) {
		return vec4<f32>(0.);
	}
	let color = textureLoad(INPUT_texture, texel, 0);
	return vec4<f32>(color.rgb * color.a, color.a);
}

fn INPUT_at(coord: vec2<i32>) -> vec4<f32> {
	let axes = kernel_frame[AXES];
	let center = vec2<f32>(coord) + 0.5;
	let texel = axes.xy * center.x + axes.zw * center.y + kernel_frame[PLACEMENT].xy - 0.5;
	let base = floor(texel);
	let t = texel - base;
	let corner = vec2<i32>(base);
	let top = mix(INPUT_premultiplied(corner), INPUT_premultiplied(corner + vec2<i32>(1, 0)), t.x);
	let bottom = mix(INPUT_premultiplied(corner + vec2<i32>(0, 1)), INPUT_premultiplied(corner + vec2<i32>(1, 1)), t.x);
	let premultiplied = mix(top, bottom, t.y);
	if premultiplied.a <= 0. {
		return vec4<f32>(0.);
	}
	return vec4<f32>(premultiplied.rgb / premultiplied.a, premultiplied.a);
}
