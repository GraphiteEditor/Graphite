// The passes of the layer styles' finer options, compiled together with `vello_effects.wgsl` and sharing its inputs and helpers

// The last of the gradient's `GRADIENT_TABLE_SIZE` samples
const LAST_GRADIENT_SAMPLE: f32 = 255.;

// Where each texel lies across an outline for a contour gradient starting `values.x` texels past the edge and spanning `values.y` inward,
// from the distances out from the edge (`source`) and in from it (`secondary`), rounded to the gradient's 256 samples, matching `contour_gradient_position`
@fragment
fn contour_gradient_position(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let outward = textureLoad(source, pixel, 0).r;
	let inward = textureLoad(secondary, pixel, 0).r;

	// Each texel's center lies half a texel short of its distance from the edge, inside or out
	let signed_distance = max(outward - 0.5, 0.) - max(inward - 0.5, 0.);
	return vec4<f32>(floor(clamp((parameters.values.x - signed_distance) / parameters.values.y, 0., 1.) * LAST_GRADIENT_SAMPLE + 0.5) / LAST_GRADIENT_SAMPLE);
}

// The band's coverage (`source`) of a premultiplied paint (`secondary`)
@fragment
fn paint_band(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	return textureLoad(secondary, pixel, 0) * textureLoad(source, pixel, 0).r;
}

// The band's coverage (`content`) of the gradient `secondary` holds in a row of straight colors from its start to its end, at each texel's place across the outline (`source`),
// faded by the opacity `amount`, matching the CPU's `contour_gradient_colors`
@fragment
fn contour_gradient_band(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let place = textureLoad(source, pixel, 0).r;

	// The gradient's samples span its ends, and each place lies on one of them
	let last = i32(textureDimensions(secondary).x) - 1;
	let color = textureLoad(secondary, vec2<i32>(clamp(i32(round(place * f32(last))), 0, last), 0), 0);

	let alpha = color.a * parameters.amount * textureLoad(content, pixel, 0).r;
	return vec4<f32>(color.rgb * alpha, alpha);
}
