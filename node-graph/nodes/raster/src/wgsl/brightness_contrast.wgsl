fn apply_levels(value: f32, input_shadows: f32, input_highlights: f32, inverse_gamma: f32, output_minimum: f32, output_maximum: f32) -> f32 {
	let highlights_minus_shadows = clamp(input_highlights - input_shadows, 1.1920929e-7, 1.);
	let clamped = min(max(value - input_shadows, 0.) / highlights_minus_shadows, 1.);
	let curved = pow(clamped, inverse_gamma);

	return curved * (output_maximum - output_minimum) + output_minimum;
}

fn brightness_contrast_classic(value: f32, brightness: f32, contrast: f32, pivot: f32) -> f32 {
	if contrast >= 1. {
		return select(0., 1., value + brightness >= pivot - 1. / 65536.);
	}

	var result: f32;
	if contrast > 0. {
		let input_shadows = pivot * contrast - brightness;
		result = apply_levels(value, input_shadows, input_shadows + 1. - contrast, 1., 0., 1.);
	} else {
		let output_minimum = brightness - pivot * contrast;
		result = apply_levels(value, 0., 1., 1., output_minimum, output_minimum + 1. + contrast);
	}

	return clamp(result, 0., 1.);
}

fn brightness_curve_hermite(curve: BrightnessCurve, t: f32) -> vec2<f32> {
	let length = 1. - curve.knee;
	let start_tangent = length * curve.slope;
	let end_tangent = length * curve.end_slope;
	let t2 = t * t;
	let t3 = t2 * t;

	let value = (2. * t3 - 3. * t2 + 1.) * 0.5 + (t3 - 2. * t2 + t) * start_tangent + (-2. * t3 + 3. * t2) + (t3 - t2) * end_tangent;
	let derivative = ((6. * t2 - 6. * t) * 0.5 + (3. * t2 - 4. * t + 1.) * start_tangent + (-6. * t2 + 6. * t) + (3. * t2 - 2. * t) * end_tangent) / length;

	return vec2<f32>(value, derivative);
}

fn brightness_curve_forward(curve: BrightnessCurve, x: f32) -> f32 {
	if x < curve.knee {
		return curve.slope * x;
	}

	let t = min((x - curve.knee) / (1. - curve.knee), 1.);
	return min(brightness_curve_hermite(curve, t).x, 1.);
}

fn brightness_curve_inverse(curve: BrightnessCurve, y: f32) -> f32 {
	if y <= 0.5 {
		return y / curve.slope;
	}

	var low = 0.;
	var high = 1.;
	var t = (y - 0.5) * 2.;
	for (var i = 0; i < 8; i++) {
		let hermite = brightness_curve_hermite(curve, t);
		let error = hermite.x - y;
		if error > 0. {
			high = t;
		} else {
			low = t;
		}

		let step = t - error / (hermite.y * (1. - curve.knee));
		t = select((low + high) * 0.5, step, step >= low && step <= high);
	}

	return curve.knee + t * (1. - curve.knee);
}

fn adjust(c: f32, parameters: Params) -> f32 {
	if parameters.use_classic {
		return brightness_contrast_classic(c, parameters.brightness / 255., parameters.contrast, parameters.classic_pivot);
	}

	var brightened: f32;
	if parameters.brightness >= 0. {
		brightened = brightness_curve_forward(parameters.second_curve, brightness_curve_forward(parameters.first_curve, c));
	} else {
		brightened = brightness_curve_inverse(parameters.first_curve, brightness_curve_inverse(parameters.second_curve, c));
	}

	let contrasted = brightened + 0.76 * parameters.contrast * (2. * brightened - 1.) * min(brightened, 1. - brightened);
	return clamp(contrasted, 0., 1.);
}

fn map_color(color: vec4<f32>) -> vec4<f32> {
	let parameters = params();
	let channels = to_gamma_srgb_channels(color);
	let adjusted = vec3<f32>(
		adjust(channels.r, parameters),
		adjust(channels.g, parameters),
		adjust(channels.b, parameters),
	);
	return from_gamma_srgb_channels(vec4<f32>(adjusted, channels.a));
}
