fn levels_curve_apply(curve: LevelsCurve, value: f32) -> f32 {
	let t = clamp((value * 255. - curve.black) * 255. / (curve.white - curve.black), 0., 255.);
	var y: f32;
	if t < curve.toe_end {
		let u = t / curve.toe_end;
		let hermite_start = u * u * u - 2. * u * u + u;
		let hermite_end_value = 3. * u * u - 2. * u * u * u;
		let hermite_end_slope = u * u * u - u * u;
		y = hermite_start * curve.toe_end * curve.toe_slope_start + hermite_end_value * curve.toe_value + hermite_end_slope * curve.toe_end * curve.toe_slope_end;
	} else {
		y = 255. * pow(t / 255., curve.exponent);
	}
	return y / 255.;
}
