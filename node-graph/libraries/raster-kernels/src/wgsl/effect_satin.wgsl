// Satin's shading from two casts of the silhouette: how much one covers without the other, or how much they agree if `invert`
fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let difference = abs(input_0_load(coord).a - input_1_load(coord).a);
	return vec4<f32>(select(difference, 1. - difference, params().invert));
}
