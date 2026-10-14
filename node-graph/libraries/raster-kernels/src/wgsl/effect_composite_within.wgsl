// Places a tinted silhouette (input 1's coverage) over the image (input 0), within the content at full strength (input 2) so it stays visible over a faded fill
fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let image = input_0_load(coord);
	let tinted = params().color * input_1_load(coord).a;
	let full_content = input_2_load(coord);
	return tinted * full_content.a + image * (1. - tinted.a);
}
