fn map_color(color: vec4<f32>) -> vec4<f32> {
	let parameters = params();
	let domain_min = vec3<f32>(parameters.domain_min[0], parameters.domain_min[1], parameters.domain_min[2]);
	let domain_max = vec3<f32>(parameters.domain_max[0], parameters.domain_max[1], parameters.domain_max[2]);
	let channels = to_gamma_srgb_channels(color);
	let span = max(domain_max - domain_min, vec3<f32>(1.1920929e-7));
	let normalized = clamp((channels.rgb - domain_min) / span, vec3<f32>(0.), vec3<f32>(1.));

	let last = table_size() - 1;
	let scaled = normalized * f32(last);
	let lower = min(vec3<i32>(floor(scaled)), vec3<i32>(last));
	let upper = min(lower + 1, vec3<i32>(last));
	let f = scaled - vec3<f32>(lower);

	var first: vec3<i32>;
	var second: vec3<i32>;
	var t: vec3<f32>;
	if f.r >= f.g && f.g >= f.b {
		first = vec3<i32>(upper.r, lower.g, lower.b);
		second = vec3<i32>(upper.r, upper.g, lower.b);
		t = vec3<f32>(f.r, f.g, f.b);
	} else if f.r >= f.b && f.b >= f.g {
		first = vec3<i32>(upper.r, lower.g, lower.b);
		second = vec3<i32>(upper.r, lower.g, upper.b);
		t = vec3<f32>(f.r, f.b, f.g);
	} else if f.b >= f.r && f.r >= f.g {
		first = vec3<i32>(lower.r, lower.g, upper.b);
		second = vec3<i32>(upper.r, lower.g, upper.b);
		t = vec3<f32>(f.b, f.r, f.g);
	} else if f.g >= f.r && f.r >= f.b {
		first = vec3<i32>(lower.r, upper.g, lower.b);
		second = vec3<i32>(upper.r, upper.g, lower.b);
		t = vec3<f32>(f.g, f.r, f.b);
	} else if f.g >= f.b && f.b >= f.r {
		first = vec3<i32>(lower.r, upper.g, lower.b);
		second = vec3<i32>(lower.r, upper.g, upper.b);
		t = vec3<f32>(f.g, f.b, f.r);
	} else {
		first = vec3<i32>(lower.r, lower.g, upper.b);
		second = vec3<i32>(lower.r, upper.g, upper.b);
		t = vec3<f32>(f.b, f.g, f.r);
	}

	let c000 = table_load(lower).rgb;
	let c111 = table_load(upper).rgb;
	let first_entry = table_load(first).rgb;
	let second_entry = table_load(second).rgb;
	let result = c000 + (t.x * (first_entry - c000) + t.y * (second_entry - first_entry) + t.z * (c111 - second_entry));
	return from_gamma_srgb_channels(vec4<f32>(result, channels.a));
}
