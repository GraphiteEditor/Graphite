// The passes that mask, feather, and blur content by a radius mask, appended to `vello_effects.wgsl` and sharing its bindings and parameters

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

// The alpha at or below which a feather counts content as absent, matching `FEATHER_FOOTPRINT_ALPHA`
const FEATHER_FOOTPRINT_ALPHA: f32 = 0.5 / 255.;

// The coverage of the area outside the footprint of the content in `source`, with its edge falling within the texels bordering it by their alpha relative to the largest beside them
@fragment
fn feather_outside(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let size = vec2<i32>(textureDimensions(source));
	let alpha = textureLoad(source, pixel, 0).a;
	if alpha <= FEATHER_FOOTPRINT_ALPHA {
		return vec4<f32>(1.);
	}

	var borders_absence = false;
	var largest = alpha;
	for (var neighbor_y = -1; neighbor_y <= 1; neighbor_y++) {
		for (var neighbor_x = -1; neighbor_x <= 1; neighbor_x++) {
			let neighbor = pixel + vec2<i32>(neighbor_x, neighbor_y);
			if any(neighbor < vec2<i32>(0)) || any(neighbor >= size) {
				continue;
			}
			let neighbor_alpha = textureLoad(source, neighbor, 0).a;
			borders_absence = borders_absence || neighbor_alpha <= FEATHER_FOOTPRINT_ALPHA;
			largest = max(largest, neighbor_alpha);
		}
	}
	return vec4<f32>(select(0., 1. - min(alpha / largest, 1.), borders_absence));
}

// How much the footprint eroded by `values.x` texels covers each texel, given its distance to the area outside the footprint plus half a texel in `source`,
// relative to its own coverage of the footprint (one less the outside coverage in `secondary`), matching the CPU's `erode`
@fragment
fn feather_erode(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let coverage = 1. - textureLoad(secondary, pixel, 0).r;
	let eroded = clamp(textureLoad(source, pixel, 0).r - parameters.values.x, 0., 1.);
	return vec4<f32>(select(eroded, min(eroded / coverage, 1.), coverage > 0.));
}

// The luminance of a masked radius blur's map in `source`, premultiplied, rounded to its 8-bit code, matching `masked_radius_blur_code`
@fragment
fn masked_radius_blur_luminance(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let map = textureLoad(source, vec2<i32>(position.xy), 0);
	return vec4<f32>(round(clamp(dot(map.rgb, MASK_LUMINANCE_WEIGHTS), 0., 1.) * 255.) / 255.);
}

// The level below the map luminance in `source` and the share of the level above, looked up in the `secondary` row by the luminance's 8-bit code, matching `masked_radius_blur_interpolation`
@fragment
fn masked_radius_blur_interpolation(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let code = i32(round(textureLoad(source, vec2<i32>(position.xy), 0).r * 255.));
	return textureLoad(secondary, vec2<i32>(code, 0), 0);
}

// Adds the masked radius blur's level `values.x` in `source`, weighed by its share of each texel's interpolation in `secondary`, to the sum so far in `content` (with `values.y` 1)
@fragment
fn accumulate_masked_radius_blur_level(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let pixel = vec2<i32>(position.xy);
	let interpolation = textureLoad(secondary, pixel, 0);
	let level = parameters.values.x;

	var weight = 0.;
	if abs(interpolation.r - level) < 0.5 {
		weight = 1. - interpolation.g;
	} else if abs(interpolation.r + 1. - level) < 0.5 {
		weight = interpolation.g;
	}

	var sum = vec4<f32>(0.);
	if parameters.values.y > 0.5 {
		sum = textureLoad(content, pixel, 0);
	}
	return sum + textureLoad(source, pixel, 0) * weight;
}
