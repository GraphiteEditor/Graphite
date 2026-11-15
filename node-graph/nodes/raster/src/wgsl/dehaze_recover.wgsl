fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let parameters = params();
	let first = max(coord.y - parameters.radius, 0);
	let last = min(coord.y + parameters.radius, input_0_size().y - 1);

	var sum = vec2<f32>(0.);
	for (var y = first; y <= last; y++) {
		sum += input_0_load(vec2<i32>(coord.x, y)).xy;
	}
	let coefficients = sum / f32(last - first + 1);

	let color = input_1_load(coord);
	let hazy = clamp(color.rgb, vec3<f32>(0.), vec3<f32>(1.));
	let refined = coefficients.x * luminance_rec_709(vec4<f32>(hazy, 1.)) + coefficients.y;
	let transmission = max(clamp(refined, 0., 1.), parameters.minimum_transmission);
	let light = parameters.atmospheric_light.rgb;
	return vec4<f32>(clamp((hazy - light) / transmission + light, vec3<f32>(0.), vec3<f32>(1.)), color.a);
}
