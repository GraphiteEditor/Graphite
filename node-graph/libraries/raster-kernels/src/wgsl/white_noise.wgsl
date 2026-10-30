fn pcg3d(input: vec3<u32>) -> vec3<u32> {
	var v = input * 1664525u + 1013904223u;
	v.x += v.y * v.z;
	v.y += v.z * v.x;
	v.z += v.x * v.y;
	v ^= v >> vec3<u32>(16u);
	v.x += v.y * v.z;
	v.y += v.z * v.x;
	v.z += v.x * v.y;
	return v;
}

fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let parameters = params();
	let point = (vec2<f32>(coord) + 0.5) * vec2<f32>(parameters.texel[0], parameters.texel[1]) + vec2<f32>(parameters.corner[0], parameters.corner[1]);
	let cell = vec2<i32>(floor(point));
	let hash = pcg3d(vec3<u32>(bitcast<vec2<u32>>(cell), parameters.seed));
	let luminance = f32(hash.x >> 8u) / 16777216.;
	return vec4<f32>(vec3<f32>(luminance), 1.);
}
