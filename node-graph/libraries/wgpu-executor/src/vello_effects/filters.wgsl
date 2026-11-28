
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
// or failing that its own if it has any coverage. A pass after others (`values.z` 1) keeps the color they found (`content`), and only the last (`values.w` 1) falls back to the texel's own.
@fragment
fn visible_colors(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	if parameters.values.z > 0.5 {
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
	if parameters.values.w > 0.5 && own.a > 0. {
		return vec4<f32>(own.rgb / own.a, 1.);
	}
	return vec4<f32>(0.);
}

// The content composited over an opaque gray of level `values.z`, which a matted filter reads
@fragment
fn matte_colors(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let color = textureLoad(source, vec2<i32>(position.xy), 0);
	return vec4<f32>(color.rgb + parameters.values.z * (1. - color.a), 1.);
}

// A texel of the colors a filter reads, with an alpha of 1 where there's one, or beyond the edges nothing, or with `values.y` at or above 0 the opaque gray of that level
fn color_at(pixel: vec2<i32>) -> vec4<f32> {
	let size = vec2<i32>(textureDimensions(source));
	if any(pixel < vec2<i32>(0)) || any(pixel >= size) {
		return select(vec4<f32>(0.), vec4<f32>(vec3<f32>(parameters.values.y), 1.), parameters.values.y >= 0.);
	}
	return textureLoad(source, pixel, 0);
}

// The colors at a position in texels from the first texel's center, interpolated between those of the four texels around it with a color, matching the CPU's `BilinearTap`
fn color_between(position: vec2<f32>) -> vec3<f32> {
	let whole = floor(position);
	let fraction = position - whole;
	let pixel = vec2<i32>(whole);

	let top = mix(color_at(pixel), color_at(pixel + vec2<i32>(1, 0)), fraction.x);
	let bottom = mix(color_at(pixel + vec2<i32>(0, 1)), color_at(pixel + vec2<i32>(1, 1)), fraction.x);
	let between = mix(top, bottom, fraction.y);
	return select(vec3<f32>(0.), between.rgb / between.a, between.a > 0.);
}

// The weighted sum of the colors (`source`) read `taps[i].xy` texels away, each weighted by `taps[i].z`, raised by the offset `values.x` and clamped,
// matching the CPU's `convolve` before the content's coverage returns. A pass after others (`values.z` 1) adds to their sum (`content`), and only the last (`values.w` 1) clamps it.
@fragment
fn convolve_taps(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let center = vec2<f32>(pixel);

	var sum = vec3<f32>(parameters.values.x);
	if parameters.values.z > 0.5 {
		sum = textureLoad(content, pixel, 0).rgb;
	}
	for (var i = 0u; i < parameters.tap_count; i++) {
		let tap = parameters.taps[i];
		sum += color_between(center + tap.xy) * tap.z;
	}

	if parameters.values.w > 0.5 {
		return vec4<f32>(clamp(sum, vec3<f32>(0.), vec3<f32>(1.)), 1.);
	}
	return vec4<f32>(sum, 1.);
}

// For PSD interop, each channel of the colors matted over white (`source`) darkened by the length of its Sobel gradient, matching the CPU's `find_edges`.
// Each tap `taps[i].xy` texels away weighs `taps[i].z` across and `taps[i].w` down.
@fragment
fn find_edges(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let center = vec2<f32>(vec2<i32>(position.xy));

	var across = vec3<f32>(0.);
	var down = vec3<f32>(0.);
	for (var i = 0u; i < parameters.tap_count; i++) {
		let tap = parameters.taps[i];
		let color = color_between(center + tap.xy);
		across += color * tap.z;
		down += color * tap.w;
	}
	return vec4<f32>(clamp(1. - sqrt(across * across + down * down), vec3<f32>(0.), vec3<f32>(1.)), 1.);
}

// Filtered opaque colors (`source`) given the `content` texel's coverage, matching the CPU's `uncovered`: as straight colors, or with `values.z` at or above 0, lifted off the gray of that level they were filtered over
@fragment
fn uncover(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let colors = textureLoad(source, pixel, 0).rgb;
	let alpha = textureLoad(content, pixel, 0).a;

	let gray = parameters.values.z;
	if gray < 0. {
		return vec4<f32>(colors * alpha, alpha);
	}
	return vec4<f32>(clamp(colors - gray * (1. - alpha), vec3<f32>(0.), vec3<f32>(alpha)), alpha);
}

// The colors and coverage a neighborhood maximum (with `values.z` 1) or minimum works on, matching the CPU's `neighborhood_extreme`:
// the straight colors of covered texels for the maximum, black elsewhere, or for the minimum the colors matted over white
@fragment
fn neighborhood_prepare(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let color = textureLoad(source, vec2<i32>(position.xy), 0);
	if parameters.values.z > 0.5 {
		return select(vec4<f32>(0.), vec4<f32>(color.rgb / color.a, color.a), color.a > 0.);
	}
	return vec4<f32>(color.rgb + 1. - color.a, color.a);
}

// The colors' extreme of two texels (the maximum with `values.z` 1, otherwise the minimum), and the opposite extreme of their coverages
fn neighborhood_extremes(first: vec4<f32>, second: vec4<f32>) -> vec4<f32> {
	if parameters.values.z > 0.5 {
		return vec4<f32>(max(first.rgb, second.rgb), min(first.a, second.a));
	}
	return vec4<f32>(min(first.rgb, second.rgb), max(first.a, second.a));
}

// A texel of what a neighborhood works on (`source`), or beyond the edges, the colors at their identity and no coverage
fn neighborhood_texel(pixel: vec2<i32>) -> vec4<f32> {
	let size = vec2<i32>(textureDimensions(source));
	if any(pixel < vec2<i32>(0)) || any(pixel >= size) {
		return vec4<f32>(vec3<f32>(select(1., 0., parameters.values.z > 0.5)), 0.);
	}
	return textureLoad(source, pixel, 0);
}

// The extremes over `values.x` texels each way along `step`, read one by one, matching the CPU's `window_extremes`
@fragment
fn neighborhood_extreme(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let step = vec2<i32>(parameters.step);
	let radius = i32(parameters.values.x);

	var extremes = textureLoad(source, pixel, 0);
	for (var offset = 1; offset <= radius; offset++) {
		extremes = neighborhood_extremes(extremes, neighborhood_texel(pixel - step * offset));
		extremes = neighborhood_extremes(extremes, neighborhood_texel(pixel + step * offset));
	}
	return extremes;
}

// The extremes (`source`) widened by `values.x` texels each way along `step`, from the two windows that far to either side, which span the wider one if `values.x` is at most their reach.
// One past an edge reads the window at the edge instead, which lies within the wider one and also reaches beyond the edge.
@fragment
fn neighborhood_widen(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let last = vec2<i32>(textureDimensions(source)) - 1;
	let shift = vec2<i32>(parameters.step) * i32(parameters.values.x);

	let before = textureLoad(source, clamp(pixel - shift, vec2<i32>(0), last), 0);
	let after = textureLoad(source, clamp(pixel + shift, vec2<i32>(0), last), 0);
	return neighborhood_extremes(before, after);
}

// The extremes of a maximum (with `values.z` 1) or minimum back as premultiplied colors, the minimum's lifted back off white within the grown coverage
fn premultiplied_extremes(extremes: vec4<f32>) -> vec4<f32> {
	if parameters.values.z > 0.5 {
		return vec4<f32>(extremes.rgb * extremes.a, extremes.a);
	}
	return vec4<f32>(clamp(extremes.rgb - (1. - extremes.a), vec3<f32>(0.), vec3<f32>(extremes.a)), extremes.a);
}

// The extremes (`source`) back as premultiplied colors, mixed toward those of the next whole radius (`secondary`) by the fraction `values.x`, matching the CPU's `neighborhood_extreme`
@fragment
fn neighborhood_finish(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let near = premultiplied_extremes(textureLoad(source, pixel, 0));
	let far = premultiplied_extremes(textureLoad(secondary, pixel, 0));
	return mix(near, far, parameters.values.x);
}
