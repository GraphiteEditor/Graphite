// Places the shadows and glows (input 1) beneath the image (input 0), knocked out by the content at full strength (input 2) so a faded fill doesn't reveal them
fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let image = input_0_load(coord);
	let beneath = input_1_load(coord);
	let full_content = input_2_load(coord);
	return image + beneath * (1. - full_content.a);
}
