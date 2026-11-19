// The passes that resample an item's content from elsewhere than nearby, like blurs along lines and arcs.
// They join the effect passes' module, sharing its parameters and inputs, and read their frame from the start of the tap array (see `frame_parameters`).

// The source's texel, or transparency past its edges
fn texel_or_transparent(texel: vec2<i32>) -> vec4<f32> {
	let size = vec2<i32>(textureDimensions(source));
	if any(texel < vec2<i32>(0)) || any(texel >= size) {
		return vec4<f32>(0.);
	}
	return textureLoad(source, texel, 0);
}

// The source at a point in texels, interpolated between the four texel centers around it, with transparency past the edges, matching the CPU's `sample_bilinear`
fn sample_bilinear(point: vec2<f32>) -> vec4<f32> {
	let position = point - 0.5;
	let base = floor(position);
	let fraction = position - base;
	let texel = vec2<i32>(base);

	let top = mix(texel_or_transparent(texel), texel_or_transparent(texel + vec2<i32>(1, 0)), fraction.x);
	let bottom = mix(texel_or_transparent(texel + vec2<i32>(0, 1)), texel_or_transparent(texel + vec2<i32>(1, 1)), fraction.x);
	return mix(top, bottom, fraction.y);
}

// The Keys cubic convolution kernel with a = -0.75, matching the CPU's `keys_cubic`
fn keys_cubic(value: f32) -> f32 {
	let a = -0.75;
	let x = abs(value);
	if x < 1. {
		return (a + 2.) * x * x * x - (a + 3.) * x * x + 1.;
	}
	if x < 2. {
		return a * x * x * x - 5. * a * x * x + 8. * a * x - 4. * a;
	}
	return 0.;
}

// Premultiplied channels clipped to what they can hold: alpha from 0 to 1, and color from 0 to the alpha
fn clipped(color: vec4<f32>) -> vec4<f32> {
	let alpha = clamp(color.a, 0., 1.);
	return vec4<f32>(clamp(color.rgb, vec3<f32>(0.), vec3<f32>(alpha)), alpha);
}

// A motion blur's passes measure texels along the major axis (x) and across it (y), which are the texture's y and x when `values.z` is 1.
// The line moves `values.x` across the minor axis per texel along the major axis, less the fraction `values.y` that measures it from the frame's edge, matching the CPU's `MotionBlurShear`.
fn motion_axes(position: vec2<f32>) -> vec2<i32> {
	let texel = vec2<i32>(position);
	if parameters.values.z > 0.5 {
		return vec2<i32>(texel.y, texel.x);
	}
	return texel;
}

fn motion_load(major: i32, minor: i32) -> vec4<f32> {
	if parameters.values.z > 0.5 {
		return texel_or_transparent(vec2<i32>(minor, major));
	}
	return texel_or_transparent(vec2<i32>(major, minor));
}

// How far the shear moves the column (or row) at `major`, as whole texels (x) and the fraction left over (y)
fn motion_shift(major: i32) -> vec2<f32> {
	let shift = parameters.values.x * (f32(major) + 0.5) - parameters.values.y;
	let whole = floor(shift);
	return vec2<f32>(whole, shift - whole);
}

// Moves each column across the minor axis by the fraction of its shift
@fragment
fn motion_shear(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let axes = motion_axes(position.xy);
	let fraction = motion_shift(axes.x).y;

	var sum = vec4<f32>(0.);
	for (var offset = -1; offset <= 2; offset++) {
		sum += motion_load(axes.x, axes.y + offset) * keys_cubic(f32(offset) - fraction);
	}
	return clipped(sum);
}

// Averages the sheared texels along the line, whose box spans `taps[0].x` to `taps[0].y` texels along the major axis, sampled `taps[0].z` apart
@fragment
fn motion_box(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let axes = motion_axes(position.xy);
	let span = parameters.taps[0].xy;
	let stride = parameters.taps[0].z;
	let whole = motion_shift(axes.x).x;

	let first = i32(floor((span.x + stride / 2.) / stride));
	let last = i32(ceil((span.y - stride / 2.) / stride));
	var sum = vec4<f32>(0.);
	var total = 0.;
	for (var index = first; index <= last; index++) {
		let offset = f32(index) * stride;
		let weight = min(span.y, offset + stride / 2.) - max(span.x, offset - stride / 2.);
		if weight > 0. {
			let major = axes.x + i32(offset);
			sum += motion_load(major, axes.y + i32(motion_shift(major).x - whole)) * weight;
			total += weight;
		}
	}
	return sum / max(total, 1e-6);
}

// Moves each column back by the fraction of its shift
@fragment
fn motion_unshear(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let axes = motion_axes(position.xy);
	let fraction = motion_shift(axes.x).y;

	var sum = vec4<f32>(0.);
	for (var offset = -2; offset <= 1; offset++) {
		sum += motion_load(axes.x, axes.y + offset) * keys_cubic(f32(offset) + fraction);
	}
	return clipped(sum);
}

// The frame of an effect relative to the content's extent, which `taps[0..3]` hold
struct Frame {
	// The map from texels into the frame's local space, measured in the effect space's units from its center
	texels_to_local: mat3x2<f32>,
	// The map from the frame's local space into texels
	local_to_texels: mat3x2<f32>,
	// Half the lengths of the frame's sides, in the effect space
	half_size: vec2<f32>,
}

fn frame_parameters() -> Frame {
	let taps = parameters.taps;
	return Frame(
		mat3x2<f32>(taps[0].xy, taps[0].zw, taps[1].xy),
		mat3x2<f32>(taps[1].zw, taps[2].xy, taps[2].zw),
		taps[3].xy,
	);
}

fn transform_point(transform: mat3x2<f32>, point: vec2<f32>) -> vec2<f32> {
	return transform * vec3<f32>(point, 1.);
}

// A radial blur around or through the local point `taps[3].zw` of the frame, spanning `values.x` (a spin's angle, or a zoom's share of the distance to the center),
// spinning with `values.y` 0 or zooming with 1, at `values.z` texels per unit of the effect space, matching the CPU's `radial_blurred`
@fragment
fn radial_blur(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
	let frame = frame_parameters();
	let local = transform_point(frame.texels_to_local, position.xy);
	let center = parameters.taps[3].zw;
	let extent = parameters.values.x;
	let spin = parameters.values.y < 0.5;
	let offset = local - center;
	let samples_along = length(offset) * extent * parameters.values.z * RADIAL_SAMPLES_PER_TEXEL;
	let count = u32(clamp(ceil(samples_along) + 1., 1., f32(MAX_RADIAL_SAMPLES)));

	var sum = vec4<f32>(0.);
	for (var index = 0u; index < count; index++) {
		// In the middle of an equal share of the arc or line, which runs from -0.5 to 0.5 and centers on the point
		let along = (f32(index) + 0.5) / f32(count) - 0.5;

		var point = center + offset * (1. - extent * along);
		if spin {
			let turn = vec2<f32>(cos(extent * along), sin(extent * along));
			point = center + vec2<f32>(turn.x * offset.x - turn.y * offset.y, turn.y * offset.x + turn.x * offset.y);
		}
		sum += sample_bilinear(transform_point(frame.local_to_texels, point));
	}
	return sum / f32(count);
}

// Matching `RADIAL_SAMPLES_PER_PIXEL` and `MAX_RADIAL_SAMPLES`
const RADIAL_SAMPLES_PER_TEXEL: f32 = 2.;
const MAX_RADIAL_SAMPLES: u32 = 512u;
