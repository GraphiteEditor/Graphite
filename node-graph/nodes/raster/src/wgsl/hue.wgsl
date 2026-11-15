fn hexagon_hue_degrees(rgb: vec3<f32>) -> f32 {
	let maximum = max(max(rgb.r, rgb.g), rgb.b);
	let chroma = maximum - min(min(rgb.r, rgb.g), rgb.b);
	if chroma <= 0. {
		return 0.;
	}

	var sector: f32;
	if maximum == rgb.r {
		sector = wrap_positive((rgb.g - rgb.b) / chroma, 6.);
	} else if maximum == rgb.g {
		sector = (rgb.b - rgb.r) / chroma + 2.;
	} else {
		sector = (rgb.r - rgb.g) / chroma + 4.;
	}
	return sector * 60.;
}

fn wrap_positive(value: f32, modulus: f32) -> f32 {
	return value - floor(value / modulus) * modulus;
}
