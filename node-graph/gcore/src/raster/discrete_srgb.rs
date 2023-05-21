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
    0.94299024, 0.95163417, 0.96032387, 0.96906, 0.977842, 0.9866705, 0.9955452, 1.0
];

#[rustfmt::skip]
const FLOAT_SRGB_LERP: [u32; 27] = [
    0x66f, 0x66f063b, 0xcaa0515, 0x11c00773, 0x193305dc, 0x1f1004f3, 0x24030481, 0x28850773,
    0x2ff9065e, 0x365805a1, 0x3bfa0547, 0x414108f7, 0x4a3907d8, 0x52110709, 0x591b06aa, 0x5fc50b70,
    0x6b350a18, 0x754e091c, 0x7e6b08aa, 0x87160ef1, 0x96070d3e, 0xa3460bfc, 0xaf430b6c, 0xbaaf13bd,
    0xce6d1187, 0xdff40fe3, 0xefd70f28
];

#[inline]
pub fn float_to_srgb_u8(mut f: f32) -> u8 {
	// Clamp f to [0, 1], with a negated condition to handle NaNs as 0.
	if !(f >= 0.0) {
		f = 0.0;
	} else if f > 1.0 {
		f = 1.0;
	}

	// Shift away slightly from 0.0 to reduce exponent range.
	const C: f32 = 0.009842521f32;
	let u = (f + C).to_bits() - C.to_bits();
	if u > (1.0 + C).to_bits() - C.to_bits() {
		// We clamped f to [0, 1], and the integer representations
		// of the positive finite non-NaN floats are monotonic.
		// This makes the later LUT lookup panicless.
		unsafe { std::hint::unreachable_unchecked() }
	}

	// Compute a piecewise linear interpolation that is always
	// the correct answer, or one less than it.
	let u16mask = (1 << 16) - 1;
	let lut_idx = u >> 21;
	let lerp_idx = (u >> 5) & u16mask;
	let biasmult = FLOAT_SRGB_LERP[lut_idx as usize];
	let bias = (biasmult >> 16) << 16;
	let mult = biasmult & u16mask;
	// I don't believe this wraps, but since we test in release mode,
	// better make sure debug mode behaves the same.
	let lerp = bias.wrapping_add(mult * lerp_idx) >> 24;

	// Adjust linear interpolation to the correct value.
	if f > CRITICAL_POINTS[lerp as usize] {
		lerp as u8 + 1
	} else {
		lerp as u8
	}
}

#[rustfmt::skip]
const FROM_SRGB_U8: [f32; 256] = [
    0.0, 0.000303527, 0.000607054, 0.00091058103, 0.001214108, 0.001517635, 0.0018211621, 0.002124689,
    0.002428216, 0.002731743, 0.00303527, 0.0033465356, 0.003676507, 0.004024717, 0.004391442,
    0.0047769533, 0.005181517, 0.0056053917, 0.0060488326, 0.006512091, 0.00699541, 0.0074990317,
    0.008023192, 0.008568125, 0.009134057, 0.009721218, 0.010329823, 0.010960094, 0.011612245,
    0.012286487, 0.012983031, 0.013702081, 0.014443844, 0.015208514, 0.015996292, 0.016807375,
    0.017641952, 0.018500218, 0.019382361, 0.020288562, 0.02121901, 0.022173883, 0.023153365,
    0.02415763, 0.025186857, 0.026241222, 0.027320892, 0.028426038, 0.029556843, 0.03071345, 0.03189604,
    0.033104774, 0.03433981, 0.035601325, 0.036889452, 0.038204376, 0.039546248, 0.04091521, 0.042311423,
    0.043735042, 0.045186214, 0.046665095, 0.048171833, 0.049706575, 0.051269468, 0.052860655, 0.05448028,
    0.056128494, 0.057805434, 0.05951124, 0.06124607, 0.06301003, 0.06480328, 0.06662595, 0.06847818,
    0.07036011, 0.07227186, 0.07421358, 0.07618539, 0.07818743, 0.08021983, 0.082282715, 0.084376216,
    0.086500466, 0.088655606, 0.09084173, 0.09305898, 0.095307484, 0.09758736, 0.09989874, 0.10224175,
    0.10461649, 0.10702311, 0.10946172, 0.111932434, 0.11443538, 0.116970696, 0.11953845, 0.12213881,
    0.12477186, 0.12743773, 0.13013652, 0.13286836, 0.13563336, 0.13843165, 0.14126332, 0.1441285,
    0.1470273, 0.14995982, 0.15292618, 0.1559265, 0.15896086, 0.16202943, 0.16513224, 0.16826946,
    0.17144115, 0.17464745, 0.17788847, 0.1811643, 0.18447503, 0.1878208, 0.19120172, 0.19461787,
    0.19806935, 0.2015563, 0.20507877, 0.2086369, 0.21223079, 0.21586053, 0.21952623, 0.22322798,
    0.22696589, 0.23074007, 0.23455065, 0.23839766, 0.2422812, 0.2462014, 0.25015837, 0.25415218,
    0.2581829, 0.26225072, 0.26635566, 0.27049786, 0.27467737, 0.27889434, 0.2831488, 0.2874409,
    0.2917707, 0.29613832, 0.30054384, 0.30498737, 0.30946895, 0.31398875, 0.31854683, 0.32314324,
    0.32777813, 0.33245158, 0.33716366, 0.34191445, 0.3467041, 0.3515327, 0.35640025, 0.36130688,
    0.3662527, 0.37123778, 0.37626222, 0.3813261, 0.38642952, 0.39157256, 0.3967553, 0.40197787,
    0.4072403, 0.4125427, 0.41788515, 0.42326775, 0.42869055, 0.4341537, 0.43965724, 0.44520125,
    0.45078585, 0.45641106, 0.46207705, 0.46778384, 0.47353154, 0.47932023, 0.48514998, 0.4910209,
    0.49693304, 0.5028866, 0.50888145, 0.5149178, 0.5209957, 0.52711535, 0.5332766, 0.5394797,
    0.5457247, 0.5520116, 0.5583406, 0.5647117, 0.57112503, 0.57758063, 0.5840786, 0.590619, 0.597202,
    0.60382754, 0.61049575, 0.61720675, 0.62396055, 0.63075733, 0.637597, 0.6444799, 0.6514058,
    0.65837497, 0.66538745, 0.67244333, 0.6795426, 0.68668544, 0.69387203, 0.70110214, 0.70837605,
    0.7156938, 0.72305536, 0.730461, 0.7379107, 0.7454045, 0.75294244, 0.76052475, 0.7681514, 0.77582246,
    0.78353804, 0.79129815, 0.79910296, 0.8069525, 0.8148468, 0.822786, 0.8307701, 0.83879924, 0.84687346,
    0.8549928, 0.8631574, 0.87136734, 0.8796226, 0.8879232, 0.89626956, 0.90466136, 0.913099, 0.92158204,
    0.93011117, 0.9386859, 0.9473069, 0.9559735, 0.9646866, 0.9734455, 0.98225087, 0.9911022, 1.0
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
		if !(f > 0.0f32) {
			0.0f32
		} else if f <= 0.0031308f32 {
			12.92f32 * f
		} else if f < 1.0f32 {
			1.055f32 * f.powf(1.0f32 / 2.4f32) - 0.055f32
		} else {
			1.0f32
		}
	}

	fn float_to_srgb_u8_ref(f: f32) -> u8 {
		(float_to_srgb_ref(f) * 255.0f32 + 0.5f32) as u8
	}

	// https://microsoft.github.io/DirectX-Specs/d3d/archive/D3D11_3_FunctionalSpec.htm#SRGBtoFLOAT
	fn srgb_to_float_ref(f: f32) -> f32 {
		if f <= 0.04045f32 {
			f / 12.92f32
		} else {
			((f + 0.055f32) / 1.055f32).powf(2.4f32)
		}
	}

	fn srgb_u8_to_float_ref(c: u8) -> f32 {
		srgb_to_float_ref(c as f32 * (1.0f32 / 255.0f32))
	}

	#[test]
	fn test_float_to_srgb_u8() {
		for u in 0..=u8::MAX {
			assert!(srgb_u8_to_float(u) == srgb_u8_to_float_ref(u));
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
