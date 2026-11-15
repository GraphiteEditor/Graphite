// =============
// VERTEX SHADER
// =============

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
	let pos = array(
		vec2f(-1.0, -1.0),
		vec2f(3.0, -1.0),
		vec2f(-1.0, 3.0),
	);
	return vec4f(pos[vertex_index], 0.0, 1.0);
}

// ===============
// FRAGMENT SHADER
// ===============

@group(0) @binding(0)
var t_composite: texture_2d<f32>;

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
	let premultiplied = textureLoad(t_composite, vec2<i32>(frag.xy), 0);
	var straight = vec3<f32>(0.0);
	if (premultiplied.a > 0.0) {
		straight = premultiplied.rgb / premultiplied.a;
	}
	return vec4<f32>(straight, premultiplied.a);
}
