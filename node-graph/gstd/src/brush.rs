use crate::raster::{BlendImageTupleNode, blend_image_closure, extend_image_to_bounds};
use glam::{DAffine2, DVec2};
use graph_craft::generic::FnNode;
use graph_craft::proto::FutureWrapperNode;
use graphene_core::raster::adjustments::blend_colors;
use graphene_core::raster::bbox::{AxisAlignedBbox, Bbox};
use graphene_core::raster::brush_cache::BrushCache;
use graphene_core::raster::image::{Image, ImageFrameTable};
use graphene_core::raster::{Alpha, Bitmap, BlendMode, Color, Pixel, Sample};
use graphene_core::transform::{Transform, TransformMut};
use graphene_core::value::{ClonedNode, ValueNode};
use graphene_core::vector::VectorDataTable;
use graphene_core::vector::brush_stroke::{BrushStroke, BrushStyle};
use graphene_core::{Ctx, GraphicElement, Node};

#[node_macro::node(category("Debug"))]
fn vector_points(_: impl Ctx, vector_data: VectorDataTable) -> Vec<DVec2> {
	let vector_data = vector_data.one_instance_ref().instance;

	vector_data.point_domain.positions().to_vec()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushStampGenerator<P: Pixel + Alpha> {
	color: P,
	feather_exponent: f32,
	transform: DAffine2,
}

impl<P: Pixel + Alpha> Transform for BrushStampGenerator<P> {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
}

impl<P: Pixel + Alpha> TransformMut for BrushStampGenerator<P> {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		&mut self.transform
	}
}

impl<P: Pixel + Alpha> Sample for BrushStampGenerator<P> {
	type Pixel = P;

	#[inline]
	fn sample(&self, position: DVec2, area: DVec2) -> Option<P> {
		let position = self.transform.inverse().transform_point2(position);
		let area = self.transform.inverse().transform_vector2(area);
		let aa_blur_radius = area.length() as f32 * 2.;
		let center = DVec2::splat(0.5);

		let distance = (position + area / 2. - center).length() as f32 * 2.;

		let edge_opacity = 1. - (1. - aa_blur_radius).powf(self.feather_exponent);
		let result = if distance < 1. - aa_blur_radius {
			1. - distance.powf(self.feather_exponent)
		} else if distance < 1. {
			// TODO: Replace this with a proper analytical AA implementation
			edge_opacity * ((1. - distance) / aa_blur_radius)
		} else {
			return None;
		};

		use graphene_core::raster::Channel;
		Some(self.color.multiplied_alpha(P::AlphaChannel::from_linear(result)))
	}
}

#[node_macro::node(skip_impl)]
fn brush_stamp_generator(diameter: f64, color: Color, hardness: f64, flow: f64) -> BrushStampGenerator<Color> {
	// Diameter
	let radius = diameter / 2.;

	// Hardness
	let hardness = hardness / 100.;
	let feather_exponent = 1. / (1. - hardness) as f32;

	// Flow
	let flow = flow / 100.;

	// Color
	let color = color.apply_opacity(flow as f32);

	let transform = DAffine2::from_scale_angle_translation(DVec2::splat(diameter), 0., -DVec2::splat(radius));
	BrushStampGenerator { color, feather_exponent, transform }
}

#[node_macro::node(skip_impl)]
fn blit<P, BlendFn>(mut target: ImageFrameTable<P>, texture: Image<P>, positions: Vec<DVec2>, blend_mode: BlendFn) -> ImageFrameTable<P>
where
	P: Pixel + Alpha + std::fmt::Debug,
	BlendFn: for<'any_input> Node<'any_input, (P, P), Output = P>,
	GraphicElement: From<Image<P>>,
{
	if positions.is_empty() {
		return target;
	}

	let target_width = target.one_instance_ref().instance.width;
	let target_height = target.one_instance_ref().instance.height;
	let target_size = DVec2::new(target_width as f64, target_height as f64);

	let texture_size = DVec2::new(texture.width as f64, texture.height as f64);

	let document_to_target = DAffine2::from_translation(-texture_size / 2.) * DAffine2::from_scale(target_size) * target.transform().inverse();

	for position in positions {
		let start = document_to_target.transform_point2(position).round();
		let stop = start + texture_size;

		// Half-open integer ranges [start, stop).
		let clamp_start = start.clamp(DVec2::ZERO, target_size).as_uvec2();
		let clamp_stop = stop.clamp(DVec2::ZERO, target_size).as_uvec2();

		let blit_area_offset = (clamp_start.as_dvec2() - start).as_uvec2().min(texture_size.as_uvec2());
		let blit_area_dimensions = (clamp_stop - clamp_start).min(texture_size.as_uvec2() - blit_area_offset);

		// Tight blitting loop. Eagerly assert bounds to hopefully eliminate bounds check inside loop.
		let texture_index = |x: u32, y: u32| -> usize { (y as usize * texture.width as usize) + (x as usize) };
		let target_index = |x: u32, y: u32| -> usize { (y as usize * target_width as usize) + (x as usize) };

		let max_y = (blit_area_offset.y + blit_area_dimensions.y).saturating_sub(1);
		let max_x = (blit_area_offset.x + blit_area_dimensions.x).saturating_sub(1);
		assert!(texture_index(max_x, max_y) < texture.data.len());
		assert!(target_index(max_x, max_y) < target.one_instance_ref().instance.data.len());

		for y in blit_area_offset.y..blit_area_offset.y + blit_area_dimensions.y {
			for x in blit_area_offset.x..blit_area_offset.x + blit_area_dimensions.x {
				let src_pixel = texture.data[texture_index(x, y)];
				let dst_pixel = &mut target.one_instance_mut().instance.data[target_index(x + clamp_start.x, y + clamp_start.y)];
				*dst_pixel = blend_mode.eval((src_pixel, *dst_pixel));
			}
		}
	}

	target
}

pub async fn create_brush_texture(brush_style: &BrushStyle) -> Image<Color> {
	let stamp = brush_stamp_generator(brush_style.diameter, brush_style.color, brush_style.hardness, brush_style.flow);
	let transform = DAffine2::from_scale_angle_translation(DVec2::splat(brush_style.diameter), 0., -DVec2::splat(brush_style.diameter / 2.));
	use crate::raster::empty_image;
	let blank_texture = empty_image((), transform, Color::TRANSPARENT);
	let image = crate::raster::blend_image_closure(stamp, blank_texture, |a, b| blend_colors(a, b, BlendMode::Normal, 1.));

	image.one_instance_ref().instance.clone()
}

macro_rules! inline_blend_funcs {
	($bg:ident, $fg:ident, $blend_mode:ident, $opacity:ident, [$($mode:path,)*]) => {
		match std::hint::black_box($blend_mode) {
			$(
				$mode => {
					blend_image_closure($fg, $bg, |a, b| blend_colors(a, b, $mode, $opacity))
				}
			)*
		}
	};
}

pub fn blend_with_mode(background: ImageFrameTable<Color>, foreground: ImageFrameTable<Color>, blend_mode: BlendMode, opacity: f64) -> ImageFrameTable<Color> {
	let opacity = opacity / 100.;
	inline_blend_funcs!(
		background,
		foreground,
		blend_mode,
		opacity,
		[
			// Normal group
			BlendMode::Normal,
			// Darken group
			BlendMode::Darken,
			BlendMode::Multiply,
			BlendMode::ColorBurn,
			BlendMode::LinearBurn,
			BlendMode::DarkerColor,
			// Lighten group
			BlendMode::Lighten,
			BlendMode::Screen,
			BlendMode::ColorDodge,
			BlendMode::LinearDodge,
			BlendMode::LighterColor,
			// Contrast group
			BlendMode::Overlay,
			BlendMode::SoftLight,
			BlendMode::HardLight,
			BlendMode::VividLight,
			BlendMode::LinearLight,
			BlendMode::PinLight,
			BlendMode::HardMix,
			// Inversion group
			BlendMode::Difference,
			BlendMode::Exclusion,
			BlendMode::Subtract,
			BlendMode::Divide,
			// Component group
			BlendMode::Hue,
			BlendMode::Saturation,
			BlendMode::Color,
			BlendMode::Luminosity,
			// Other utility blend modes (hidden from the normal list)
			BlendMode::Erase,
			BlendMode::Restore,
			BlendMode::MultiplyAlpha,
		]
	)
}

#[node_macro::node(category(""))]
async fn brush(_: impl Ctx, image_frame_table: ImageFrameTable<Color>, bounds: ImageFrameTable<Color>, strokes: Vec<BrushStroke>, cache: BrushCache) -> ImageFrameTable<Color> {
	let stroke_bbox = strokes.iter().map(|s| s.bounding_box()).reduce(|a, b| a.union(&b)).unwrap_or(AxisAlignedBbox::ZERO);
	let image_bbox = Bbox::from_transform(image_frame_table.transform()).to_axis_aligned_bbox();
	let bbox = if image_bbox.size().length() < 0.1 { stroke_bbox } else { stroke_bbox.union(&image_bbox) };

	let mut draw_strokes: Vec<_> = strokes.iter().filter(|&s| !matches!(s.style.blend_mode, BlendMode::Erase | BlendMode::Restore)).cloned().collect();
	let erase_restore_strokes: Vec<_> = strokes.iter().filter(|&s| matches!(s.style.blend_mode, BlendMode::Erase | BlendMode::Restore)).cloned().collect();

	let mut brush_plan = cache.compute_brush_plan(image_frame_table, &draw_strokes);

	let mut background_bounds = bbox.to_transform();

	// If the bounds are empty (no size on images or det(transform) = 0), keep the target bounds
	let bounds_empty = bounds.instance_ref_iter().all(|bounds| bounds.instance.width() == 0 || bounds.instance.height() == 0);
	if bounds.transform().matrix2.determinant() != 0. && !bounds_empty {
		background_bounds = bounds.transform();
	}

	let mut actual_image = extend_image_to_bounds((), brush_plan.background, background_bounds);
	let final_stroke_idx = brush_plan.strokes.len().saturating_sub(1);
	for (idx, stroke) in brush_plan.strokes.into_iter().enumerate() {
		// Create brush texture.
		// TODO: apply rotation from layer to stamp for non-rotationally-symmetric brushes.
		let mut brush_texture = cache.get_cached_brush(&stroke.style);
		if brush_texture.is_none() {
			let tex = create_brush_texture(&stroke.style).await;
			cache.store_brush(stroke.style.clone(), tex.clone());
			brush_texture = Some(tex);
		}
		let brush_texture = brush_texture.unwrap();

		// Compute transformation from stroke texture space into layer space, and create the stroke texture.
		let skip = if idx == 0 { brush_plan.first_stroke_point_skip } else { 0 };
		let positions: Vec<_> = stroke.compute_blit_points().into_iter().skip(skip).collect();
		let stroke_texture = if idx == 0 && positions.is_empty() {
			core::mem::take(&mut brush_plan.first_stroke_texture)
		} else {
			let mut bbox = stroke.bounding_box();
			bbox.start = bbox.start.floor();
			bbox.end = bbox.end.floor();
			let stroke_size = bbox.size() + DVec2::splat(stroke.style.diameter);
			// For numerical stability we want to place the first blit point at a stable, integer offset in layer space.
			let snap_offset = positions[0].floor() - positions[0];
			let stroke_origin_in_layer = bbox.start - snap_offset - DVec2::splat(stroke.style.diameter / 2.);
			let stroke_to_layer = DAffine2::from_translation(stroke_origin_in_layer) * DAffine2::from_scale(stroke_size);

			// let normal_blend = BlendColorPairNode::new(ValueNode::new(CopiedNode::new(BlendMode::Normal)), ValueNode::new(CopiedNode::new(100.)));
			let normal_blend = FnNode::new(|(a, b)| blend_colors(a, b, BlendMode::Normal, 1.));
			let blit_node = BlitNode::new(
				FutureWrapperNode::new(ClonedNode::new(brush_texture)),
				FutureWrapperNode::new(ClonedNode::new(positions)),
				FutureWrapperNode::new(ClonedNode::new(normal_blend)),
			);
			let blit_target = if idx == 0 {
				let target = core::mem::take(&mut brush_plan.first_stroke_texture);
				extend_image_to_bounds((), target, stroke_to_layer)
			} else {
				use crate::raster::empty_image;
				empty_image((), stroke_to_layer, Color::TRANSPARENT)
				// EmptyImageNode::new(CopiedNode::new(stroke_to_layer), CopiedNode::new(Color::TRANSPARENT)).eval(())
			};

			blit_node.eval(blit_target).await
		};

		// Cache image before doing final blend, and store final stroke texture.
		if idx == final_stroke_idx {
			cache.cache_results(core::mem::take(&mut draw_strokes), actual_image.clone(), stroke_texture.clone());
		}

		// TODO: Is this the correct way to do opacity in blending?
		actual_image = blend_with_mode(actual_image, stroke_texture, stroke.style.blend_mode, (stroke.style.color.a() * 100.) as f64);
	}

	let has_erase_strokes = strokes.iter().any(|s| s.style.blend_mode == BlendMode::Erase);
	if has_erase_strokes {
		let opaque_image = Image::new(bbox.size().x as u32, bbox.size().y as u32, Color::WHITE);
		let mut erase_restore_mask = ImageFrameTable::new(opaque_image);
		*erase_restore_mask.transform_mut() = background_bounds;
		*erase_restore_mask.one_instance_mut().alpha_blending = Default::default();

		for stroke in erase_restore_strokes {
			let mut brush_texture = cache.get_cached_brush(&stroke.style);
			if brush_texture.is_none() {
				let tex = create_brush_texture(&stroke.style).await;
				cache.store_brush(stroke.style.clone(), tex.clone());
				brush_texture = Some(tex);
			}
			let brush_texture = brush_texture.unwrap();
			let positions: Vec<_> = stroke.compute_blit_points().into_iter().collect();

			match stroke.style.blend_mode {
				BlendMode::Erase => {
					let blend_params = FnNode::new(|(a, b)| blend_colors(a, b, BlendMode::Erase, 1.));
					let blit_node = BlitNode::new(
						FutureWrapperNode::new(ClonedNode::new(brush_texture)),
						FutureWrapperNode::new(ClonedNode::new(positions)),
						FutureWrapperNode::new(ClonedNode::new(blend_params)),
					);
					erase_restore_mask = blit_node.eval(erase_restore_mask).await;
				}
				// Yes, this is essentially the same as the above, but we duplicate to inline the blend mode.
				BlendMode::Restore => {
					let blend_params = FnNode::new(|(a, b)| blend_colors(a, b, BlendMode::Restore, 1.));
					let blit_node = BlitNode::new(
						FutureWrapperNode::new(ClonedNode::new(brush_texture)),
						FutureWrapperNode::new(ClonedNode::new(positions)),
						FutureWrapperNode::new(ClonedNode::new(blend_params)),
					);
					erase_restore_mask = blit_node.eval(erase_restore_mask).await;
				}
				_ => unreachable!(),
			}
		}

		let blend_params = FnNode::new(|(a, b)| blend_colors(a, b, BlendMode::MultiplyAlpha, 1.));
		let blend_executor = BlendImageTupleNode::new(FutureWrapperNode::new(ValueNode::new(blend_params)));
		actual_image = blend_executor.eval((actual_image, erase_restore_mask)).await;
	}

	actual_image
}

#[cfg(test)]
mod test {
	use super::*;
	use glam::DAffine2;
	use graphene_core::raster::Bitmap;
	use graphene_core::transform::Transform;

	#[test]
	fn test_brush_texture() {
		let size = 20.;
		let image = brush_stamp_generator(size, Color::BLACK, 100., 100.);
		assert_eq!(image.transform(), DAffine2::from_scale_angle_translation(DVec2::splat(size.ceil()), 0., -DVec2::splat(size / 2.)));
		// center pixel should be BLACK
		assert_eq!(image.sample(DVec2::splat(0.), DVec2::ONE), Some(Color::BLACK));
	}

	#[tokio::test]
	async fn test_brush_output_size() {
		let image = brush(
			(),
			ImageFrameTable::<Color>::default(),
			ImageFrameTable::<Color>::default(),
			vec![BrushStroke {
				trace: vec![crate::vector::brush_stroke::BrushInputSample { position: DVec2::ZERO }],
				style: BrushStyle {
					color: Color::BLACK,
					diameter: 20.,
					hardness: 20.,
					flow: 20.,
					spacing: 20.,
					blend_mode: BlendMode::Normal,
				},
			}],
			BrushCache::new_proto(),
		)
		.await;
		assert_eq!(image.width(), 20);
	}
}
