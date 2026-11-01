fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let parameters = params();
	let first = max(coord.x - parameters.radius, 0);
	let last = min(coord.x + parameters.radius, input_0_size().x - 1);

	var sum = vec4<f32>(0.);
	for (var x = first; x <= last; x++) {
		let texel = input_0_load(vec2<i32>(x, coord.y));
		let guidance = luminance_rec_709(texel);
		let transmission = 1. - parameters.strength * texel.a;
		sum += vec4<f32>(guidance, transmission, guidance * guidance, guidance * transmission);
	}
	return sum / f32(last - first + 1);
}
