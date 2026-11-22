// Renders a tile of a generator's pattern, sampled at the centers of the tile's pixels, as opaque gamma-encoded color like the CPU's tiles.
// Each pattern's shader is this followed by its own, whose `Parameters` start with the tile's top left corner in the pattern's space and its pixel size there.

// One triangle that covers the whole target
@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
	let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
	return vec4<f32>(corner * 2. - 1., 0., 1.);
}

// Clamped to the range the CPU encodes, which takes NaN as 0
fn linear_to_srgb(unclamped: f32) -> f32 {
	let linear = select(0., min(unclamped, 1.), unclamped >= 0.);
	let low = linear * 12.92;
	let high = 1.055 * pow(linear, 1. / 2.4) - 0.055;
	return select(high, low, linear <= 0.0031308);
}
