fn luma_rec_601_fixed_point(rgb: vec3<f32>) -> f32 {
	return (4915. * rgb.r + 9667. * rgb.g + 1802. * rgb.b) / 16384.;
}
