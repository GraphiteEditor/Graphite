// Stacks a tinted silhouette (input 1's coverage) over the shadows and glows beneath the content (input 0)
fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let beneath = input_0_load(coord);
	let tinted = params().color * input_1_load(coord).a;
	return tinted + beneath * (1. - tinted.a);
}
