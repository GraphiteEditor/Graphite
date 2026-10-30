const TOE_END: f32 = 0.05568117;

fn decode(value: vec3<f32>) -> vec3<f32> {
	return select(pow(value, vec3<f32>(2.2)), value / 32., value < vec3<f32>(TOE_END));
}

fn encode(linear: vec3<f32>) -> vec3<f32> {
	return select(pow(linear, vec3<f32>(1. / 2.2)), linear * 32., linear < vec3<f32>(TOE_END / 32.));
}

fn map_color(color: vec4<f32>) -> vec4<f32> {
	let parameters = params();
	let channels = to_gamma_srgb_channels(color);
	let linear = decode(channels.rgb) * exp2(parameters.exposure) + parameters.offset;
	let adjusted = encode(min(pow(max(linear, vec3<f32>(0.)), vec3<f32>(1. / parameters.gamma_correction)), vec3<f32>(1.)));
	return from_gamma_srgb_channels(vec4<f32>(adjusted, channels.a));
}
