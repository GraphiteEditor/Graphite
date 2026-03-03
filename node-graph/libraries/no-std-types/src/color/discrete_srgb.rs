#![allow(clippy::neg_cmp_op_on_partial_ord)]
//! Fast conversions between u8 sRGB and linear float.

// Inspired by https://gist.github.com/rygorous/2203834, but with a slightly
// modified method, custom derived constants and error correction for perfect
// accuracy in accordance with the D3D11 spec:
// https://microsoft.github.io/DirectX-Specs/d3d/archive/D3D11_3_FunctionalSpec.htm#FLOATtoSRGB.

/// CRITICAL_POINTS[i] is the last float value such that it maps to i after
/// conversion to integer sRGB. So if x > CRITICAL_POINTS[i] you know you need
/// to increment i.
#[rustfmt::skip]
const CRITICAL_POINTS: [f32; 256] = [
	0.00015176347, 0.00045529046, 0.0007588174, 0.0010623443, 0.0013658714, 0.0016693983, 0.0019729252, 0.0022764523,
	0.0025799791, 0.0028835062, 0.0031883009, 0.003509259, 0.003848315, 0.004205748, 0.0045818323, 0.0049768374,
	0.005391024, 0.00582465, 0.0062779686, 0.0067512267, 0.0072446675, 0.0077585294, 0.008293047, 0.008848451,
	0.0094249705, 0.010022825, 0.010642236, 0.01128342, 0.011946591, 0.012631957, 0.013339729, 0.014070111,
	0.0148233045, 0.015599505, 0.01639891, 0.017221717, 0.018068114, 0.018938294, 0.019832445, 0.020750746,
	0.021693384, 0.022660539, 0.02365239, 0.024669115, 0.025710886, 0.026777886, 0.027870273, 0.028988222,
	0.030131903, 0.03130148, 0.032497127, 0.033718992, 0.034967244, 0.03624204, 0.03754355, 0.03887192,
	0.040227327, 0.041609894, 0.04301979, 0.044457167, 0.04592218, 0.04741497, 0.04893569, 0.050484486,
	0.05206151, 0.053666897, 0.055300802, 0.056963358, 0.058654714, 0.060375024, 0.062124394, 0.06390298,
	0.065710925, 0.06754836, 0.06941542, 0.07131224, 0.07323896, 0.07519571, 0.07718261, 0.07919981,
	0.08124744, 0.08332562, 0.08543448, 0.08757417, 0.08974478, 0.091946445, 0.09417931, 0.09644348,
	0.098739095, 0.10106628, 0.10342514, 0.105815805, 0.1082384, 0.110693045, 0.11317986, 0.11569896,
	0.118250474, 0.12083454, 0.12345121, 0.12610064, 0.12878296, 0.13149826, 0.13424668, 0.1370283,
	0.13984327, 0.14269169, 0.14557366, 0.1484893, 0.15143873, 0.15442204, 0.15743938, 0.16049084,
	0.1635765, 0.16669647, 0.16985092, 0.1730399, 0.17626354, 0.17952198, 0.18281525, 0.1861435,
	0.18950681, 0.19290532, 0.19633913, 0.19980833, 0.20331302, 0.20685332, 0.21042931, 0.21404111,
	0.21768881, 0.22137253, 0.22509235, 0.22884844, 0.23264077, 0.23646952, 0.24033478, 0.24423665,
	0.24817522, 0.25215057, 0.25616285, 0.26021212, 0.26429847, 0.26842204, 0.27258286, 0.27678108,
	0.2810168, 0.28529006, 0.289601, 0.2939497, 0.29833627, 0.30276078, 0.30722332, 0.311724,
	0.31626293, 0.32084015, 0.32545578, 0.33010995, 0.3348027, 0.3395341, 0.34430432, 0.34911346,
	0.3539615, 0.35884857, 0.3637748, 0.36874023, 0.373745, 0.37878913, 0.38387278, 0.388996,
	0.39415887, 0.39936152, 0.404604, 0.4098864, 0.41520882, 0.42057133, 0.425974, 0.431417,
	0.43690032, 0.4424241, 0.44798836, 0.45359328, 0.45923886, 0.46492523, 0.47065246, 0.47642064,
	0.48222986, 0.48808017, 0.4939718, 0.49990457, 0.5058787, 0.5118943, 0.5179514, 0.5240501,
	0.5301905, 0.5363727, 0.5425967, 0.54886264, 0.5551706, 0.56152064, 0.5679129, 0.5743473,
	0.5808241, 0.5873433, 0.593905, 0.60050917, 0.60715604, 0.61384565, 0.62057805, 0.6273533,
	0.63417155, 0.6410328, 0.6479372, 0.65488476, 0.66187555, 0.6689097, 0.6759874, 0.68310845,
	0.6902731, 0.6974814, 0.7047334, 0.71202916, 0.7193688, 0.7267524, 0.73418003, 0.7416518,
	0.7491677, 0.7567278, 0.76433223, 0.7719811, 0.7796744, 0.7874122, 0.7951947, 0.80302185,
	0.8108938, 0.81881046, 0.82677215, 0.8347787, 0.8428304, 0.8509272, 0.85906917, 0.8672564,
	0.875489, 0.8837671, 0.89209044, 0.9004596, 0.9088741, 0.91733456, 0.9258405, 0.9343926,
	0.94299024, 0.95163417, 0.96032387, 0.96906, 0.977842, 0.9866705, 0.9955452, 1.,
];

#[rustfmt::skip]
const FLOAT_SRGB_LERP: [u32; 27] = [
	0x66f, 0x66f063b, 0xcaa0515, 0x11c00773, 0x193305dc, 0x1f1004f3, 0x24030481, 0x28850773,
	0x2ff9065e, 0x365805a1, 0x3bfa0547, 0x414108f7, 0x4a3907d8, 0x52110709, 0x591b06aa, 0x5fc50b70,
	0x6b350a18, 0x754e091c, 0x7e6b08aa, 0x87160ef1, 0x96070d3e, 0xa3460bfc, 0xaf430b6c, 0xbaaf13bd,
	0xce6d1187, 0xdff40fe3, 0xefd70f28,
];

#[inline]
pub fn float_to_srgb_u8(mut f: f32) -> u8 {
	// Clamp f to [0, 1], with a negated condition to handle NaNs as 0.
	if !(f >= 0.) {
		f = 0.;
	} else if f > 1. {
		f = 1.;
	}

	// Shift away slightly from 0.0 to reduce exponent range.
	const C: f32 = 0.009842521f32;
	let u = (f + C).to_bits() - C.to_bits();
	if u > (1. + C).to_bits() - C.to_bits() {
		// We clamped f to [0, 1], and the integer representations
		// of the positive finite non-NaN floats are monotonic.
		// This makes the later LUT lookup panicless.
		unsafe { core::hint::unreachable_unchecked() }
	}

	// Compute a piecewise linear interpolation that is always
	// the correct answer, or one less than it.
	let u16mask = (1 << 16) - 1;
	let lut_idx = u >> 21;
	let lerp_idx = (u >> 5) & u16mask;
	let bias_mult = FLOAT_SRGB_LERP[lut_idx as usize];
	let bias = (bias_mult >> 16) << 16;
	let mult = bias_mult & u16mask;
	// I don't believe this wraps, but since we test in release mode,
	// better make sure debug mode behaves the same.
	let lerp = bias.wrapping_add(mult * lerp_idx) >> 24;

	// Adjust linear interpolation to the correct value.
	if f > CRITICAL_POINTS[lerp as usize] { lerp as u8 + 1 } else { lerp as u8 }
}

#[rustfmt::skip]
const FROM_SRGB_U8: [f32; 256] = [
	f32::from_bits(0x00000000), f32::from_bits(0x399f22b4), f32::from_bits(0x3a1f22b4), f32::from_bits(0x3a6eb40f),
	f32::from_bits(0x3a9f22b4), f32::from_bits(0x3ac6eb61), f32::from_bits(0x3aeeb40f), f32::from_bits(0x3b0b3e5e),
	f32::from_bits(0x3b1f22b4), f32::from_bits(0x3b33070b), f32::from_bits(0x3b46eb61), f32::from_bits(0x3b5b518d),
	f32::from_bits(0x3b70f18d), f32::from_bits(0x3b83e1c6), f32::from_bits(0x3b8fe616), f32::from_bits(0x3b9c87fd),
	f32::from_bits(0x3ba9c9b7), f32::from_bits(0x3bb7ad6f), f32::from_bits(0x3bc63549), f32::from_bits(0x3bd56361),
	f32::from_bits(0x3be539c1), f32::from_bits(0x3bf5ba70), f32::from_bits(0x3c0373b5), f32::from_bits(0x3c0c6152),
	f32::from_bits(0x3c15a703), f32::from_bits(0x3c1f45be), f32::from_bits(0x3c293e6b), f32::from_bits(0x3c3391f7),
	f32::from_bits(0x3c3e4149), f32::from_bits(0x3c494d43), f32::from_bits(0x3c54b6c7), f32::from_bits(0x3c607eb1),
	f32::from_bits(0x3c6ca5df), f32::from_bits(0x3c792d22), f32::from_bits(0x3c830aa8), f32::from_bits(0x3c89af9f),
	f32::from_bits(0x3c9085db), f32::from_bits(0x3c978dc5), f32::from_bits(0x3c9ec7c2), f32::from_bits(0x3ca63433),
	f32::from_bits(0x3cadd37d), f32::from_bits(0x3cb5a601), f32::from_bits(0x3cbdac20), f32::from_bits(0x3cc5e639),
	f32::from_bits(0x3cce54ab), f32::from_bits(0x3cd6f7d5), f32::from_bits(0x3cdfd010), f32::from_bits(0x3ce8ddb9),
	f32::from_bits(0x3cf22131), f32::from_bits(0x3cfb9ac6), f32::from_bits(0x3d02a56c), f32::from_bits(0x3d0798df),
	f32::from_bits(0x3d0ca7e7), f32::from_bits(0x3d11d2b2), f32::from_bits(0x3d171965), f32::from_bits(0x3d1c7c31),
	f32::from_bits(0x3d21fb3f), f32::from_bits(0x3d2796b5), f32::from_bits(0x3d2d4ebe), f32::from_bits(0x3d332384),
	f32::from_bits(0x3d39152e), f32::from_bits(0x3d3f23e6), f32::from_bits(0x3d454fd4), f32::from_bits(0x3d4b991f),
	f32::from_bits(0x3d51ffef), f32::from_bits(0x3d58846a), f32::from_bits(0x3d5f26b7), f32::from_bits(0x3d65e6fe),
	f32::from_bits(0x3d6cc564), f32::from_bits(0x3d73c20f), f32::from_bits(0x3d7add29), f32::from_bits(0x3d810b68),
	f32::from_bits(0x3d84b795), f32::from_bits(0x3d887330), f32::from_bits(0x3d8c3e4a), f32::from_bits(0x3d9018f6),
	f32::from_bits(0x3d940345), f32::from_bits(0x3d97fd4a), f32::from_bits(0x3d9c0716), f32::from_bits(0x3da020bb),
	f32::from_bits(0x3da44a4b), f32::from_bits(0x3da883d7), f32::from_bits(0x3daccd70), f32::from_bits(0x3db12728), f32::from_bits(0x3db59112),
	f32::from_bits(0x3dba0b3b), f32::from_bits(0x3dbe95b5), f32::from_bits(0x3dc33092), f32::from_bits(0x3dc7dbe2),
	f32::from_bits(0x3dcc97b6), f32::from_bits(0x3dd1641f), f32::from_bits(0x3dd6412c), f32::from_bits(0x3ddb2eef),
	f32::from_bits(0x3de02d77), f32::from_bits(0x3de53cd5), f32::from_bits(0x3dea5d19), f32::from_bits(0x3def8e55),
	f32::from_bits(0x3df4d093), f32::from_bits(0x3dfa23ea), f32::from_bits(0x3dff8864), f32::from_bits(0x3e027f09),
	f32::from_bits(0x3e054282), f32::from_bits(0x3e080ea5), f32::from_bits(0x3e0ae379), f32::from_bits(0x3e0dc107),
	f32::from_bits(0x3e10a755), f32::from_bits(0x3e13966c), f32::from_bits(0x3e168e53), f32::from_bits(0x3e198f11),
	f32::from_bits(0x3e1c98ae), f32::from_bits(0x3e1fab32), f32::from_bits(0x3e22c6a3), f32::from_bits(0x3e25eb0b),
	f32::from_bits(0x3e29186d), f32::from_bits(0x3e2c4ed4), f32::from_bits(0x3e2f8e45), f32::from_bits(0x3e32d6c8),
	f32::from_bits(0x3e362865), f32::from_bits(0x3e398322), f32::from_bits(0x3e3ce706), f32::from_bits(0x3e405419),
	f32::from_bits(0x3e43ca62), f32::from_bits(0x3e4749e8), f32::from_bits(0x3e4ad2b1), f32::from_bits(0x3e4e64c6),
	f32::from_bits(0x3e52002b), f32::from_bits(0x3e55a4e9), f32::from_bits(0x3e595307), f32::from_bits(0x3e5d0a8b),
	f32::from_bits(0x3e60cb7c), f32::from_bits(0x3e6495e0), f32::from_bits(0x3e6869bf), f32::from_bits(0x3e6c4720),
	f32::from_bits(0x3e702e0c), f32::from_bits(0x3e741e84), f32::from_bits(0x3e781890), f32::from_bits(0x3e7c1c38),
	f32::from_bits(0x3e8014c2), f32::from_bits(0x3e82203c), f32::from_bits(0x3e84308d), f32::from_bits(0x3e8645ba),
	f32::from_bits(0x3e885fc5), f32::from_bits(0x3e8a7eb2), f32::from_bits(0x3e8ca283), f32::from_bits(0x3e8ecb3d),
	f32::from_bits(0x3e90f8e1), f32::from_bits(0x3e932b74), f32::from_bits(0x3e9562f8), f32::from_bits(0x3e979f71),
	f32::from_bits(0x3e99e0e2), f32::from_bits(0x3e9c274e), f32::from_bits(0x3e9e72b7), f32::from_bits(0x3ea0c322),
	f32::from_bits(0x3ea31892), f32::from_bits(0x3ea57308), f32::from_bits(0x3ea7d289), f32::from_bits(0x3eaa3718),
	f32::from_bits(0x3eaca0b7), f32::from_bits(0x3eaf0f69), f32::from_bits(0x3eb18333), f32::from_bits(0x3eb3fc18),
	f32::from_bits(0x3eb67a18), f32::from_bits(0x3eb8fd37), f32::from_bits(0x3ebb8579), f32::from_bits(0x3ebe12e1),
	f32::from_bits(0x3ec0a571), f32::from_bits(0x3ec33d2d), f32::from_bits(0x3ec5da17), f32::from_bits(0x3ec87c33),
	f32::from_bits(0x3ecb2383), f32::from_bits(0x3ecdd00b), f32::from_bits(0x3ed081cd), f32::from_bits(0x3ed338cc),
	f32::from_bits(0x3ed5f50b), f32::from_bits(0x3ed8b68d), f32::from_bits(0x3edb7d54), f32::from_bits(0x3ede4965),
	f32::from_bits(0x3ee11ac1), f32::from_bits(0x3ee3f16b), f32::from_bits(0x3ee6cd67), f32::from_bits(0x3ee9aeb7),
	f32::from_bits(0x3eec955d), f32::from_bits(0x3eef815d), f32::from_bits(0x3ef272ba), f32::from_bits(0x3ef56976),
	f32::from_bits(0x3ef86594), f32::from_bits(0x3efb6717), f32::from_bits(0x3efe6e02), f32::from_bits(0x3f00bd2d),
	f32::from_bits(0x3f02460e), f32::from_bits(0x3f03d1a7), f32::from_bits(0x3f055ff9), f32::from_bits(0x3f06f108),
	f32::from_bits(0x3f0884d1), f32::from_bits(0x3f0a1b57), f32::from_bits(0x3f0bb49d), f32::from_bits(0x3f0d50a2),
	f32::from_bits(0x3f0eef69), f32::from_bits(0x3f1090f2), f32::from_bits(0x3f123540), f32::from_bits(0x3f13dc53),
	f32::from_bits(0x3f15862d), f32::from_bits(0x3f1732cf), f32::from_bits(0x3f18e23b), f32::from_bits(0x3f1a9471),
	f32::from_bits(0x3f1c4973), f32::from_bits(0x3f1e0143), f32::from_bits(0x3f1fbbe1), f32::from_bits(0x3f217950),
	f32::from_bits(0x3f23398f), f32::from_bits(0x3f24fca2), f32::from_bits(0x3f26c288), f32::from_bits(0x3f288b43),
	f32::from_bits(0x3f2a56d5), f32::from_bits(0x3f2c253f), f32::from_bits(0x3f2df681), f32::from_bits(0x3f2fca9e),
	f32::from_bits(0x3f31a199), f32::from_bits(0x3f337b6e), f32::from_bits(0x3f355822), f32::from_bits(0x3f3737b5),
	f32::from_bits(0x3f391a28), f32::from_bits(0x3f3aff7e), f32::from_bits(0x3f3ce7b7), f32::from_bits(0x3f3ed2d4), f32::from_bits(0x3f40c0d6),
	f32::from_bits(0x3f42b1c0), f32::from_bits(0x3f44a592), f32::from_bits(0x3f469c4d), f32::from_bits(0x3f4895f3),
	f32::from_bits(0x3f4a9284), f32::from_bits(0x3f4c9203), f32::from_bits(0x3f4e9470), f32::from_bits(0x3f5099cd),
	f32::from_bits(0x3f52a21a), f32::from_bits(0x3f54ad59), f32::from_bits(0x3f56bb8c), f32::from_bits(0x3f58ccb3),
	f32::from_bits(0x3f5ae0cf), f32::from_bits(0x3f5cf7e2), f32::from_bits(0x3f5f11ee), f32::from_bits(0x3f612ef2), f32::from_bits(0x3f634eef),
	f32::from_bits(0x3f6571ec), f32::from_bits(0x3f6797e3), f32::from_bits(0x3f69c0db), f32::from_bits(0x3f6beccd),
	f32::from_bits(0x3f6e1bc4), f32::from_bits(0x3f704db8), f32::from_bits(0x3f7282b4), f32::from_bits(0x3f74baae),
	f32::from_bits(0x3f76f5b3), f32::from_bits(0x3f7933b9), f32::from_bits(0x3f7b74cb), f32::from_bits(0x3f7db8e0),
	f32::from_bits(0x3f800000),
];

#[inline]
pub fn srgb_u8_to_float(c: u8) -> f32 {
	FROM_SRGB_U8[c as usize]
}

#[cfg(test)]
mod tests {
	use super::*;

	// https://microsoft.github.io/DirectX-Specs/d3d/archive/D3D11_3_FunctionalSpec.htm#FLOATtoSRGB
	fn float_to_srgb_ref(f: f32) -> f32 {
		if !(f > 0_f32) {
			0_f32
		} else if f <= 0.0031308f32 {
			12.92_f32 * f
		} else if f < 1_f32 {
			1.055f32 * f.powf(1_f32 / 2.4_f32) - 0.055f32
		} else {
			1_f32
		}
	}

	fn float_to_srgb_u8_ref(f: f32) -> u8 {
		(float_to_srgb_ref(f) * 255_f32 + 0.5_f32) as u8
	}

	// https://microsoft.github.io/DirectX-Specs/d3d/archive/D3D11_3_FunctionalSpec.htm#SRGBtoFLOAT
	fn srgb_to_float_ref(f: f32) -> f32 {
		if f <= 0.04045f32 { f / 12.92f32 } else { ((f + 0.055f32) / 1.055f32).powf(2.4_f32) }
	}

	fn srgb_u8_to_float_ref(c: u8) -> f32 {
		srgb_to_float_ref(c as f32 * (1_f32 / 255_f32))
	}

	#[test]
	fn test_float_to_srgb_u8() {
		for u in 0..=u8::MAX {
			let a = srgb_u8_to_float(u);
			let b = srgb_u8_to_float_ref(u);
			if a != b {
				panic!("Mismatch at u={}: {} != {}", u, a, b);
			}
		}
	}

	#[ignore = "expensive, test in release mode"]
	#[test]
	fn test_srgb_u8_to_float() {
		// Simply... check all float values.
		for u in 0..=u32::MAX {
			let f = f32::from_bits(u);
			assert!(float_to_srgb_u8(f) == float_to_srgb_u8_ref(f));
		}
	}
}
