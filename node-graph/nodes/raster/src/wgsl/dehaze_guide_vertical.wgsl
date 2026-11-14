fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let parameters = params();
	let first = max(coord.y - parameters.radius, 0);
	let last = min(coord.y + parameters.radius, input_0_size().y - 1);

	var sum = vec4<f32>(0.);
	for (var y = first; y <= last; y++) {
		sum += input_0_load(vec2<i32>(coord.x, y));
	}
	let mean = sum / f32(last - first + 1);

	let variance = mean.z - mean.x * mean.x;
	let covariance = mean.w - mean.x * mean.y;
	let a = covariance / (variance + parameters.epsilon);
	return vec4<f32>(a, mean.y - a * mean.x, 0., 0.);
}
