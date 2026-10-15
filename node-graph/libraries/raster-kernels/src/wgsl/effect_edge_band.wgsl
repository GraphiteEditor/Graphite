// Covers the texels within `width` texels of the edge, which lies about half a texel short of the nearest seed, fading out over one texel
fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let nearest = input_0_load(coord);
	if nearest.z < 0.5 {
		return vec4<f32>(0.);
	}
	return vec4<f32>(clamp(params().width + 1. - length(nearest.xy), 0., 1.));
}
