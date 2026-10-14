// The coverage of the content's silhouette, or of the area outside it if `inverse`, where the input's placement casts it
fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let coverage = input_0_at(coord).a;
	return vec4<f32>(select(coverage, 1. - coverage, params().inverse));
}
