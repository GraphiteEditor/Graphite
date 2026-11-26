// The passes that mask content by another graphic, appended to `vello_effects.wgsl` and sharing its bindings and parameters

// SVG's `luminanceToAlpha` weights, taken on gamma-encoded channels, matching `MASK_LUMINANCE_WEIGHTS`
const MASK_LUMINANCE_WEIGHTS: vec3<f32> = vec3<f32>(0.2125, 0.7154, 0.0721);

// How much the mask graphic in `source`, premultiplied, shows the content: its luminance (times its alpha) with `values.x` 0, or its alpha with 1
@fragment
fn mask_coverage(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let mask = textureLoad(source, vec2<i32>(position.xy), 0);
	let coverage = select(dot(mask.rgb, MASK_LUMINANCE_WEIGHTS), mask.a, parameters.values.x > 0.5);
	return vec4<f32>(clamp(coverage, 0., 1.), 0., 0., 0.);
}

// The mask's feathered coverage in `source` mapped through its density line, of slope `values.x` and intercept `values.y`
@fragment
fn mask_density_line(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let coverage = textureLoad(source, vec2<i32>(position.xy), 0).r;
	return vec4<f32>(clamp(parameters.values.x * coverage + parameters.values.y, 0., 1.));
}

// The image in `source` faded by the strength from 0 to 1 in `secondary`
@fragment
fn apply_mask(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	return textureLoad(source, pixel, 0) * textureLoad(secondary, pixel, 0).r;
}

// The alpha of the full-strength content in `source` with the trailing masks' strength (`secondary`) in red, standing in for that content where `layer_opacity` reads both
@fragment
fn content_with_layer_fade(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	return vec4<f32>(textureLoad(secondary, pixel, 0).r, 0., 0., textureLoad(source, pixel, 0).a);
}
