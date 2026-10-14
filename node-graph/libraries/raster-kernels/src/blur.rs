//! The passes of a Gaussian or box blur, which halve an image until its kernel fits one pass, blur it along each axis, and double it back.

use crate::{RasterKernel, ToGpuParams, Wgsl, gpu_param_enum, gpu_params};

/// The farthest texel from the center a blur pass weighs, beyond which the image is first halved until the kernel fits.
pub const MAX_BLUR_REACH: i32 = 64;

/// The radius of a blur over texels `scale` image pixels wide, and the farthest texel from the center it weighs.
///
/// A box covers its whole texels and a fraction of the texel at each end, which keeps the window as wide as the whole pixel window of the unhalved image once halved,
/// and a Gaussian leaves out the blur that halving and doubling the image add.
pub fn blur_extent(radius: f64, box_blur: bool, scale: u32) -> (f32, i32) {
	let scale = scale as f64;
	if box_blur {
		let radius = ((radius as i32) as f64 + 0.5) / scale - 0.5;
		(radius as f32, radius.ceil() as i32)
	} else {
		let radius = (radius * radius - (scale * scale - 1.) / 3.).max(0.).sqrt() / scale;
		(radius as f32, (3. * radius).ceil() as i32)
	}
}

/// The colors a blur pass reads from its input or writes to its output, around the premultiplied colors it sums.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum BlurSpace {
	/// Values summed as they are, such as premultiplied colors or coverage.
	Premultiplied,
	/// Straight-alpha linear-light colors, premultiplied for summing.
	Linear,
	/// Straight-alpha linear-light colors, gamma-encoded and premultiplied for summing.
	Gamma,
}

impl From<BlurSpace> for u32 {
	fn from(space: BlurSpace) -> Self {
		space as u32
	}
}

gpu_param_enum!(u32: BlurSpace);

gpu_params! {
	/// Averages each 2x2 block of texels `scale` image pixels wide, weighed by how much of the `full_size` image each covers.
	#[derive(Clone)]
	pub struct BlurHalve {
		pub space: BlurSpace,
		pub scale: i32,
		pub full_size: [i32; 2],
	}
}

impl RasterKernel for BlurHalve {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(concat!(include_str!("wgsl/blur.wgsl"), include_str!("wgsl/blur_halve.wgsl")))
	}
}

gpu_params! {
	/// One axis of a blur over texels `scale` image pixels wide, each weighed by how much of the `full_size` image it covers.
	#[derive(Clone)]
	pub struct BlurPass {
		pub radius: f32,
		pub reach: i32,
		pub box_blur: bool,
		pub space: BlurSpace,
		pub scale: i32,
		pub full_size: [i32; 2],
	}
}

pub struct BlurHorizontal(pub BlurPass);

impl ToGpuParams for BlurHorizontal {
	type Params = BlurPass;
	fn to_params(&self) -> BlurPass {
		self.0.clone()
	}
}

impl RasterKernel for BlurHorizontal {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(concat!(include_str!("wgsl/blur.wgsl"), include_str!("wgsl/blur_horizontal.wgsl")))
	}
}

/// Reads its input in pairs of texels sampled bilinearly between them, so the input must be in a filterable format.
pub struct BlurVertical(pub BlurPass);

impl ToGpuParams for BlurVertical {
	type Params = BlurPass;
	fn to_params(&self) -> BlurPass {
		self.0.clone()
	}
}

impl RasterKernel for BlurVertical {
	const INPUTS: usize = 1;
	const FILTERED: bool = true;
	fn wgsl() -> Wgsl {
		Wgsl::function(concat!(include_str!("wgsl/blur.wgsl"), include_str!("wgsl/blur_vertical.wgsl")))
	}
}

gpu_params! {
	#[derive(Clone)]
	pub struct BlurUpsample {
		pub scale: f32,
		pub space: BlurSpace,
	}
}

impl RasterKernel for BlurUpsample {
	const INPUTS: usize = 1;
	const FILTERED: bool = true;
	fn wgsl() -> Wgsl {
		Wgsl::function(concat!(include_str!("wgsl/blur.wgsl"), include_str!("wgsl/blur_upsample.wgsl")))
	}
}
