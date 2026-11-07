fn levels_stage_apply(stage: LevelsStage, value: f32) -> f32 {
	return levels_curve_apply(stage.curve, value) * (stage.output_maximum - stage.output_minimum) + stage.output_minimum;
}

fn levels_chain_apply(chain: LevelsChain, value: f32) -> f32 {
	let first = levels_stage_apply(chain.first, value);
	return select(first, levels_stage_apply(chain.second, first), chain.two_stages);
}

fn map_color(color: vec4<f32>) -> vec4<f32> {
	let parameters = params();
	let channels = to_gamma_srgb_channels(color);

	return from_gamma_srgb_channels(vec4<f32>(levels_chain_apply(parameters.red, channels.r), levels_chain_apply(parameters.green, channels.g), levels_chain_apply(parameters.blue, channels.b), levels_stage_apply(parameters.alpha, channels.a)));
}
