// A bevel's height from 0 to 1, shaped by the style over a slope `size` texels wide, from the nearest texels inside (input 0) and outside (input 1)
fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let parameters = params();
	let to_inside = input_0_load(coord);
	let to_outside = input_1_load(coord);
	let size = parameters.size;

	// Texels inside the silhouette are those at no distance from it, and the edge lies about half a texel short of the nearest texel across it
	let far = 4. * size + 4.;
	var signed_distance: f32;
	if to_inside.z > 0.5 && dot(to_inside.xy, to_inside.xy) == 0. {
		signed_distance = select(far, length(to_outside.xy) - 0.5, to_outside.z > 0.5);
	} else {
		signed_distance = select(-far, 0.5 - length(to_inside.xy), to_inside.z > 0.5);
	}

	var progress: f32;
	switch parameters.style {
		case 0u: {
			progress = signed_distance / size;
		}
		case 1u: {
			progress = (signed_distance + size) / size;
		}
		case 2u: {
			progress = (signed_distance + size / 2.) / size;
		}
		default: {
			progress = abs(signed_distance) / (size / 2.);
		}
	}
	let height = clamp(progress, 0., 1.);
	return vec4<f32>(height * height * (3. - 2. * height));
}
