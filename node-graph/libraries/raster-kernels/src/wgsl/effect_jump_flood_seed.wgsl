// Jump flooding keeps each texel's offset to the nearest seed found so far (xy) and whether one is found (z).
// The seeds are the texels inside the content's silhouette, or outside it if `inward`, and each is its own nearest.
fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let inside = input_0_load(coord).a >= 0.5;
	let seed = inside != params().inward;
	return vec4<f32>(0., 0., select(0., 1., seed), 0.);
}
