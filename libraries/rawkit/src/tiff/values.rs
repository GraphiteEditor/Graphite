use num_enum::{IntoPrimitive, TryFromPrimitive};

pub trait ToFloat {
	fn to_float(&self) -> f64;
}

impl ToFloat for u32 {
	fn to_float(&self) -> f64 {
		*self as f64
	}
}

impl ToFloat for i32 {
	fn to_float(&self) -> f64 {
		*self as f64
	}
}

pub struct Rational<T: ToFloat> {
	pub numerator: T,
	pub denominator: T,
}

impl<T: ToFloat> ToFloat for Rational<T> {
	fn to_float(&self) -> f64 {
		self.numerator.to_float() / self.denominator.to_float()
	}
}

pub struct CurveLookupTable {
	table: Vec<u16>,
}

impl CurveLookupTable {
	pub fn from_sony_tone_table(values: [u16; 4]) -> CurveLookupTable {
		let mut sony_curve = [0, 0, 0, 0, 0, 4095];
		for i in 0..4 {
			sony_curve[i + 1] = values[i] >> 2 & 0xfff;
		}

		let mut table = vec![0_u16; (sony_curve[5] + 1).into()];
		for i in 0..5 {
			for j in (sony_curve[i] + 1)..=sony_curve[i + 1] {
				table[j as usize] = table[(j - 1) as usize] + (1 << i);
			}
		}

		CurveLookupTable { table }
	}

	pub fn get(&self, x: usize) -> u16 {
		self.table[x]
	}
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, IntoPrimitive, TryFromPrimitive)]
#[repr(u16)]
pub enum OrientationValue {
	Horizontal = 1,
	MirrorHorizontal = 2,
	Rotate180 = 3,
	MirrorVertical = 4,
	MirrorHorizontalRotate270 = 5,
	Rotate90 = 6,
	MirrorHorizontalRotate90 = 7,
	Rotate270 = 8,
}

impl OrientationValue {
	pub fn is_identity(&self) -> bool {
		*self == Self::Horizontal
	}

	pub fn will_swap_coordinates(&self) -> bool {
		match *self {
			Self::Horizontal | Self::MirrorHorizontal | Self::Rotate180 | Self::MirrorVertical => false,
			Self::MirrorHorizontalRotate270 | Self::Rotate90 | Self::MirrorHorizontalRotate90 | Self::Rotate270 => true,
		}
	}
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, IntoPrimitive, TryFromPrimitive)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum CompressionValue {
	Uncompressed = 1,
	CCITT_1D = 2,
	T4 = 3,
	T6 = 4,
	LZW = 5,
	JPEG_Old = 6,
	JPEG = 7,
	AdobeDeflate = 8,
	JBIG_BW = 9,
	JBIG_Color = 10,
	KODAK_626 = 262,
	Next = 32766,
	Sony_ARW_Compressed = 32767,
	Packed_Raw = 32769,
	Samsung_SRW_Compressed = 32770,
	CCIRLEW = 32771,
	Samsung_SRW_Compressed_2 = 32772,
	PackedBits = 32773,
	Thunderscan = 32809,
	Kodak_KDC_Compressed = 32867,
	IT8CTPAD = 32895,
	IT8LW = 32896,
	IT8MP = 32897,
	IT8BL = 32898,
	PixarFilm = 32908,
	PixarLog = 32909,
	Deflate = 32946,
	DCS = 32947,
	AperioJPEG2K_YCbCr = 33003,
	AperioJPEG2K_RGB = 33005,
	JBIG = 34661,
	SGILog = 34676,
	SGILog24 = 34677,
	JPEG2K = 34712,
	NikonNEFCompressed = 34713,
	JBIG2_TIFF_FX = 34715,
	ESRI_Lerc = 34887,
	LossyJPEG = 34892,
	LZMA2 = 34925,
	PNG = 34933,
	JPEG_XR = 34934,
	Zstd = 50000,
	WebP = 50001,
	JPEG_XL = 52546,
	Kodak_DCR_Compressed = 65000,
	Pentax_PEF_Compressed = 65535,
}
