fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let parameters = params();
	let axis = select(vec2<i32>(1, 0), vec2<i32>(0, 1), parameters.vertical);
	let position = select(coord.x, coord.y, parameters.vertical);
	let size = select(input_0_size().x, input_0_size().y, parameters.vertical);
	let first = max(position - parameters.radius, 0) - position;
	let last = min(position + parameters.radius, size - 1) - position;

	var dark = 1.;
	for (var offset = first; offset <= last; offset++) {
		let texel = input_0_load(coord + axis * offset);
		dark = min(dark, select(minimum_rgb_channels(clamp(texel, vec4<f32>(0.), vec4<f32>(1.))), texel.a, parameters.vertical));
	}
	return vec4<f32>(clamp(input_0_load(coord).rgb, vec3<f32>(0.), vec3<f32>(1.)), dark);
}
