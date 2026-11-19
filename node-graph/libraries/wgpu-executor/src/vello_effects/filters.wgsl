
// =======
// Filters
// =======

// The filters work on the same premultiplied gamma-encoded pixels as the passes above, sharing their bindings and parameters

// A texel's straight color with an alpha of 1 if the full-strength content (`secondary`) covers it at least `values.x`, or nothing if it doesn't or lies beyond the edges
fn visible_color_at(pixel: vec2<i32>) -> vec4<f32> {
	let size = vec2<i32>(textureDimensions(source));
	if any(pixel < vec2<i32>(0)) || any(pixel >= size) {
		return vec4<f32>(0.);
	}
	let color = textureLoad(source, pixel, 0);
	let visible = color.a > 0. && textureLoad(secondary, pixel, 0).a >= parameters.values.x;
	return select(vec4<f32>(0.), vec4<f32>(color.rgb / color.a, 1.), visible);
}

// Each texel's straight color as the filters read it, matching the CPU's `visible_colors`: its own if it's visible, otherwise that of the first visible texel `taps[i].xy` texels away,
// or failing that its own if it has any coverage. A pass after others (`values.y` 1) keeps the color they found (`content`), and only the last (`values.z` 1) falls back to the texel's own.
@fragment
fn visible_colors(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	if parameters.values.y > 0.5 {
		let found = textureLoad(content, pixel, 0);
		if found.a > 0.5 {
			return found;
		}
	}

	for (var i = 0u; i < parameters.tap_count; i++) {
		let color = visible_color_at(pixel + vec2<i32>(parameters.taps[i].xy));
		if color.a > 0.5 {
			return color;
		}
	}

	let own = textureLoad(source, pixel, 0);
	if parameters.values.z > 0.5 && own.a > 0. {
		return vec4<f32>(own.rgb / own.a, 1.);
	}
	return vec4<f32>(0.);
}

// A texel of the visible colors, with an alpha of 1 where there's one, or nothing beyond the edges
fn color_at(pixel: vec2<i32>) -> vec4<f32> {
	let size = vec2<i32>(textureDimensions(source));
	if any(pixel < vec2<i32>(0)) || any(pixel >= size) {
		return vec4<f32>(0.);
	}
	return textureLoad(source, pixel, 0);
}

// The visible colors at a position in texels from the first texel's center, interpolated between those of the four texels around it with a color, matching the CPU's `BilinearTap`
fn color_between(position: vec2<f32>) -> vec3<f32> {
	let whole = floor(position);
	let fraction = position - whole;
	let pixel = vec2<i32>(whole);

	let top = mix(color_at(pixel), color_at(pixel + vec2<i32>(1, 0)), fraction.x);
	let bottom = mix(color_at(pixel + vec2<i32>(0, 1)), color_at(pixel + vec2<i32>(1, 1)), fraction.x);
	let between = mix(top, bottom, fraction.y);
	return select(vec3<f32>(0.), between.rgb / between.a, between.a > 0.);
}

// The weighted sum of the visible colors (`source`) read `taps[i].xy` texels away, each weighted by `taps[i].z`, raised by the offset `values.x` and clamped,
// matching the CPU's `convolve` before the content's coverage returns. A pass after others (`values.y` 1) adds to their sum (`content`), and only the last (`values.z` 1) clamps it.
@fragment
fn convolve_taps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let center = vec2<f32>(pixel);

	var sum = vec3<f32>(parameters.values.x);
	if parameters.values.y > 0.5 {
		sum = textureLoad(content, pixel, 0).rgb;
	}
	for (var i = 0u; i < parameters.tap_count; i++) {
		let tap = parameters.taps[i];
		sum += color_between(center + tap.xy) * tap.z;
	}

	if parameters.values.z > 0.5 {
		return vec4<f32>(clamp(sum, vec3<f32>(0.), vec3<f32>(1.)), 1.);
	}
	return vec4<f32>(sum, 1.);
}

// Filtered opaque straight colors (`source`) given the `content` texel's coverage
@fragment
fn uncover(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let alpha = textureLoad(content, pixel, 0).a;
	return vec4<f32>(textureLoad(source, pixel, 0).rgb * alpha, alpha);
}
