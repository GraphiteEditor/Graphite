//! Runs the kernels of the `raster-kernels` crate on GPU rasters.

use crate::shader_runtime::raster_kernel_runtime::{RasterKernelArgs, RasterKernelInput, RasterKernelRuntime};
use crate::{RASTER_FORMAT, WgpuExecutor, WgpuPipelineCache, WgpuTextureFormat};
use core_types::ATTR_TRANSFORM;
use core_types::NodeError;
use core_types::list::Item;
use glam::{DAffine2, DMat2, DVec2, UVec2};
use graphic_types::Adjustment;
use raster_kernels::adjustments::{AdjustmentVisitor, visit_kernel};
use raster_kernels::{GpuParams, MapColor, RasterKernel, ToGpuParams, map_color_module, raster_kernel_module};
use raster_types::{GPU, Raster, Storage};
use std::any::{TypeId, type_name};

/// The format of intermediate rasters a [`RasterKernel::FILTERED`] kernel reads, half-float precision but filterable on WebGL2 unlike [`RASTER_FORMAT`].
pub const FILTERABLE_FORMAT: WgpuTextureFormat = WgpuTextureFormat::Rgba16Float;

fn run<K: ToGpuParams + 'static>(
	kernel: &K,
	wgsl: fn() -> String,
	filtered: bool,
	inputs: Vec<RasterKernelInput<'_>>,
	output_size: UVec2,
	output_format: WgpuTextureFormat,
	pipeline: &WgpuPipelineCache,
) -> Result<Raster<GPU>, NodeError> {
	let gpu_params = kernel.to_params();
	let mut params = Vec::new();
	gpu_params.write(&mut params);
	let args = RasterKernelArgs {
		kernel: TypeId::of::<K>(),
		label: type_name::<K>(),
		wgsl,
		params,
		inputs,
		lookups: gpu_params.lookups(),
		filtered,
		output_size,
		output_format,
	};
	pipeline
		.try_run::<RasterKernelRuntime>(&args)
		.ok_or_else(|| NodeError::new("A GPU is required to process a GPU raster"))
}

fn raster_format(pipeline: &WgpuPipelineCache) -> WgpuTextureFormat {
	pipeline.executor().map_or(RASTER_FORMAT, WgpuExecutor::raster_format)
}

/// Runs `kernel` on its inputs, each placed by the affine from output pixel coordinates to its texel coordinates, into a raster of `output_size`.
pub fn run_raster_kernel<K: RasterKernel>(kernel: &K, inputs: Vec<RasterKernelInput<'_>>, output_size: UVec2, pipeline: &WgpuPipelineCache) -> Result<Raster<GPU>, NodeError> {
	run_raster_kernel_with_format(kernel, inputs, output_size, raster_format(pipeline), pipeline)
}

/// Runs `kernel` like [`run_raster_kernel`] into a texture of `output_format` instead of the executor's raster format, such as a [`FILTERABLE_FORMAT`] intermediate a [`RasterKernel::FILTERED`] kernel can read.
pub fn run_raster_kernel_with_format<K: RasterKernel>(
	kernel: &K,
	inputs: Vec<RasterKernelInput<'_>>,
	output_size: UVec2,
	output_format: WgpuTextureFormat,
	pipeline: &WgpuPipelineCache,
) -> Result<Raster<GPU>, NodeError> {
	assert_eq!(inputs.len(), K::INPUTS, "{} takes {} inputs", type_name::<K>(), K::INPUTS);
	run(kernel, raster_kernel_module::<K>, K::FILTERED, inputs, output_size, output_format, pipeline)
}

/// Runs a [`MapColor`] on each pixel of `raster`, into a raster of its size.
pub fn map_raster<K: MapColor>(kernel: &K, raster: &Raster<GPU>, pipeline: &WgpuPipelineCache) -> Result<Raster<GPU>, NodeError> {
	if raster.is_empty() {
		return Ok(raster.clone());
	}

	let input = RasterKernelInput {
		raster,
		output_to_texel: DAffine2::IDENTITY,
	};
	run(kernel, map_color_module::<K>, false, vec![input], raster.size(), raster_format(pipeline), pipeline).map(|mapped| mapped.with_coverage(raster.coverage()))
}

/// Carries out `adjustment` on each pixel of `raster` with its kernel.
pub fn adjust_raster(adjustment: &Adjustment, raster: Raster<GPU>, pipeline: &WgpuPipelineCache) -> Result<Raster<GPU>, NodeError> {
	struct AdjustRaster<'a> {
		raster: Raster<GPU>,
		pipeline: &'a WgpuPipelineCache,
	}

	impl AdjustmentVisitor for AdjustRaster<'_> {
		type Output = Result<Raster<GPU>, NodeError>;

		fn visit<K: MapColor>(self, kernel: &K) -> Self::Output {
			map_raster(kernel, &self.raster, self.pipeline)
		}

		fn unchanged(self) -> Self::Output {
			Ok(self.raster)
		}
	}

	visit_kernel(adjustment, AdjustRaster { raster, pipeline })
}

/// The most pixels along each side of a [`union_frame`], which coarsens its grid to fit, within the texture size most WebGL2 devices support.
pub const MAX_FRAME_SIDE: u32 = 8192;

/// The output raster of a kernel over placed inputs.
pub struct KernelFrame {
	pub size: UVec2,
	/// The output item's transform, mapping the unit square onto the frame in document space.
	pub transform: DAffine2,
	/// For each input, the affine from output pixel coordinates to its texel coordinates.
	pub output_to_texel: Vec<DAffine2>,
}

impl KernelFrame {
	/// The frame of `size` pixels placed by `transform`, over `inputs` given as each raster's size and its invertible transform.
	pub fn new(size: UVec2, transform: DAffine2, inputs: &[(UVec2, DAffine2)]) -> Self {
		let output_to_document = transform * DAffine2::from_scale(1. / size.as_dvec2());
		let output_to_texel = inputs
			.iter()
			.map(|(input_size, input_transform)| DAffine2::from_scale(input_size.as_dvec2()) * input_transform.inverse() * output_to_document)
			.collect();
		Self { size, transform, output_to_texel }
	}

	/// The kernel inputs placing `rasters`, in the order of the frame's inputs.
	pub fn kernel_inputs<'a>(&self, rasters: &[&'a Raster<GPU>]) -> Vec<RasterKernelInput<'a>> {
		rasters
			.iter()
			.zip(&self.output_to_texel)
			.map(|(raster, output_to_texel)| RasterKernelInput {
				raster,
				output_to_texel: *output_to_texel,
			})
			.collect()
	}
}

/// A raster item's size in pixels and the placement of its pixels, its `ATTR_TRANSFORM` composed with its coverage, as [`union_frame`] and [`KernelFrame::new`] take each input.
pub fn raster_placement(raster: &Item<Raster<GPU>>) -> (UVec2, DAffine2) {
	let transform: DAffine2 = raster.attribute_cloned_or_default(ATTR_TRANSFORM);
	(raster.element().size(), transform * raster.element().coverage())
}

/// The frame covering the union of `inputs`, each a raster's size and transform, on the finest of their pixel grids.
///
/// The grid's axes follow the first input's, with the smallest pixel step along each that any input has, anchored at the first input's origin.
/// Each side is capped at [`MAX_FRAME_SIDE`] pixels. Empty inputs are ignored. `None` without a non-empty input or with a non-invertible one.
pub fn union_frame(inputs: &[(UVec2, DAffine2)]) -> Option<KernelFrame> {
	let placed = inputs.iter().copied().filter(|(size, _)| size.x > 0 && size.y > 0).collect::<Vec<_>>();
	let first = placed.first()?.1;
	if placed.iter().any(|(_, transform)| transform.matrix2.determinant() == 0.) {
		return None;
	}

	let axes = DMat2::from_cols(first.matrix2.x_axis.normalize(), first.matrix2.y_axis.normalize());
	let to_axes = axes.inverse();

	let corners = placed
		.iter()
		.flat_map(|(_, transform)| [DVec2::ZERO, DVec2::X, DVec2::Y, DVec2::ONE].map(|corner| to_axes * transform.transform_point2(corner)));
	let (min, max) = corners.fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(min, max), corner| (min.min(corner), max.max(corner)));

	let step = placed
		.iter()
		.map(|(size, transform)| {
			let to_texels = DMat2::from_diagonal(size.as_dvec2()) * transform.matrix2.inverse() * axes;
			1. / DVec2::new(to_texels.x_axis.length(), to_texels.y_axis.length())
		})
		.fold(DVec2::INFINITY, DVec2::min);

	let anchor = to_axes * first.translation;
	let start = anchor + ((min - anchor) / step + 1e-6).floor() * step;
	let end = anchor + ((max - anchor) / step - 1e-6).ceil() * step;
	let cells = ((end - start) / step).round().max(DVec2::ONE);

	let max_side = DVec2::splat(MAX_FRAME_SIDE as f64);
	let oversized = cells.cmpgt(max_side);
	let start = DVec2::select(oversized, min, start);
	let step = DVec2::select(oversized, (max - min) / max_side, step);
	let cells = cells.min(max_side);

	let transform = DAffine2::from_mat2_translation(axes * DMat2::from_diagonal(cells * step), axes * start);
	Some(KernelFrame::new(cells.as_uvec2(), transform, inputs))
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::Color;
	use core_types::blending::BlendMode;
	use core_types::ops::{Convert, ConvertAsync};
	use core_types::transfer_curve::TransferCurve;
	use core_types::transform::Footprint;
	use graphene_resource::Resource;
	use graphic_types::adjustment::*;
	use graphic_types::vector_types::{Gradient, GradientStop};
	use raster_kernels::adjustments::adjust_color;
	use raster_types::{CPU, Image};

	fn levels_record(shadows: f64, midtones: f64, highlights: f64) -> LevelsRecord {
		LevelsRecord {
			shadows,
			midtones,
			highlights,
			output_minimums: 5.,
			output_maximums: 95.,
		}
	}

	fn hue_range(range_start: f64) -> HueRangeSettings {
		HueRangeSettings {
			sliders: HueSaturationSliders {
				hue: 20.,
				saturation: -30.,
				lightness: 10.,
			},
			falloff_start: range_start - 30.,
			range_start,
			range_end: range_start + 30.,
			falloff_end: range_start + 60.,
		}
	}

	/// One of each adjustment, with settings away from their defaults.
	fn adjustments() -> Vec<Adjustment> {
		let curve = TransferCurve::new(vec![DVec2::new(0., 0.1), DVec2::new(0.4, 0.6), DVec2::new(1., 0.9)]);
		let inks = SelectiveColorInks {
			cyan: 20.,
			magenta: -30.,
			yellow: 10.,
			black: 5.,
		};
		let shifts = ColorBalanceShifts {
			cyan_red: 30.,
			magenta_green: -20.,
			yellow_blue: 10.,
		};
		let gradient = Gradient::new([
			GradientStop {
				position: 0.,
				midpoint: 0.5,
				color: Color::BLUE,
			},
			GradientStop {
				position: 1.,
				midpoint: 0.5,
				color: Color::RED,
			},
		]);

		vec![
			Adjustment::Desaturate {
				method: DesaturateMethod::LightnessOkLab,
			},
			Adjustment::GammaCorrection { gamma: 1.8, inverse: false },
			Adjustment::MakeOpaque,
			Adjustment::BrightnessContrast {
				brightness: 40.,
				contrast: 30.,
				use_classic: false,
				classic_pivot: 127.,
			},
			Adjustment::BrightnessContrast {
				brightness: -30.,
				contrast: 20.,
				use_classic: true,
				classic_pivot: 127.,
			},
			Adjustment::Levels(Box::new(LevelsSettings {
				composite: levels_record(10., 1.3, 90.),
				red: levels_record(5., 0.8, 100.),
				green: levels_record(0., 1., 100.),
				blue: levels_record(0., 1.5, 80.),
				alpha: levels_record(0., 1., 100.),
			})),
			Adjustment::Curves(Box::new(CurvesSettings {
				composite: curve.clone(),
				red: curve.clone(),
				green: TransferCurve::new(Vec::new()),
				blue: curve.clone(),
				alpha: TransferCurve::new(Vec::new()),
			})),
			Adjustment::BlackAndWhite(BlackAndWhiteSettings {
				use_tint: true,
				tint: Color::from_rgbaf32_unchecked(0.9, 0.8, 0.6, 1.),
				reds: 40.,
				yellows: 60.,
				greens: 40.,
				cyans: 60.,
				blues: 20.,
				magentas: 80.,
			}),
			Adjustment::HueSaturation(Box::new(HueSaturationSettings {
				master: HueSaturationSliders {
					hue: 30.,
					saturation: 20.,
					lightness: -10.,
				},
				colorize: false,
				colorize_sliders: HueSaturationSliders {
					hue: 0.,
					saturation: 25.,
					lightness: 0.,
				},
				reds: hue_range(345.),
				yellows: hue_range(45.),
				greens: hue_range(105.),
				cyans: hue_range(165.),
				blues: hue_range(225.),
				magentas: hue_range(285.),
			})),
			Adjustment::Invert,
			Adjustment::GradientMap {
				gradient: Item::new_from_element(gradient),
				reverse: true,
			},
			Adjustment::Vibrance { vibrance: 50., saturation: 20. },
			Adjustment::ChannelMixer(Box::new(ChannelMixerSettings {
				monochrome: false,
				monochrome_mix: ChannelMixerRow {
					red: 40.,
					green: 40.,
					blue: 20.,
					constant: 0.,
				},
				red: ChannelMixerRow {
					red: 80.,
					green: 20.,
					blue: 0.,
					constant: 5.,
				},
				green: ChannelMixerRow {
					red: 0.,
					green: 90.,
					blue: 10.,
					constant: 0.,
				},
				blue: ChannelMixerRow {
					red: 10.,
					green: 0.,
					blue: 100.,
					constant: -5.,
				},
			})),
			Adjustment::SelectiveColor(Box::new(SelectiveColorSettings {
				mode: RelativeAbsolute::Relative,
				reds: inks,
				yellows: inks,
				greens: inks,
				cyans: inks,
				blues: inks,
				magentas: inks,
				whites: inks,
				neutrals: inks,
				blacks: inks,
			})),
			Adjustment::Exposure {
				exposure: 0.5,
				offset: 0.02,
				gamma_correction: 1.2,
			},
			Adjustment::ColorBalance(ColorBalanceSettings {
				shadows: shifts,
				midtones: shifts,
				highlights: shifts,
				preserve_luminosity: true,
			}),
			Adjustment::PhotoFilter {
				color: Color::from_rgbaf32_unchecked(0.9, 0.5, 0.1, 1.),
				density: 40.,
				preserve_luminosity: true,
			},
			Adjustment::ColorLookup {
				lut_file: Resource::new(b"LUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n0.9 0.8 0.7\n".to_vec()),
			},
			Adjustment::ColorOverlay {
				color: Color::from_rgbaf32_unchecked(0.2, 0.6, 0.4, 1.),
				blend_mode: BlendMode::Multiply,
				opacity: 60.,
			},
		]
	}

	/// Colors spread over the cube, kept away from the steps a threshold or posterize would land on.
	fn colors() -> Vec<Color> {
		let channel = |index: usize| 0.07 + 0.29 * index as f32;
		(0..64)
			.map(|index| Color::from_rgbaf32_unchecked(channel(index % 4), channel(index / 4 % 4), channel(index / 16), 0.6 + 0.1 * (index % 4) as f32))
			.collect()
	}

	#[test]
	fn each_adjustment_kernel_agrees_on_the_gpu_with_its_cpu_mapping() {
		futures::executor::block_on(async {
			let Some(executor) = WgpuExecutor::new().await else { return };
			let pipeline = WgpuPipelineCache::default();
			executor.pipeline_init::<RasterKernelRuntime>(&pipeline);

			let colors = colors();
			let image: Raster<GPU> = Raster::new_cpu(Image {
				width: 8,
				height: 8,
				data: colors.clone(),
			})
			.convert(Footprint::DEFAULT, executor.clone());

			for adjustment in adjustments() {
				let adjusted = adjust_raster(&adjustment, image.clone(), &pipeline).unwrap();
				let adjusted: Raster<CPU> = adjusted.convert(Footprint::DEFAULT, executor.clone()).await;

				for (color, actual) in colors.iter().zip(&adjusted.data().data) {
					let expected = adjust_color(&adjustment, *color);
					let difference = (expected.to_vec4() - actual.to_vec4()).abs().max_element();
					assert!(difference < 0.01, "{adjustment}: {color:?} became {actual:?} on the GPU but {expected:?} on the CPU");
				}
			}
		});
	}
}
