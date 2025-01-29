use crate::raster::{blend_image_closure, BlendImageTupleNode, EmptyImageNode, ExtendImageToBoundsNode};

use graphene_core::raster::adjustments::blend_colors;
use graphene_core::raster::bbox::{AxisAlignedBbox, Bbox};
use graphene_core::raster::brush_cache::BrushCache;
use graphene_core::raster::image::{ImageFrame, ImageFrameTable};
use graphene_core::raster::BlendMode;
use graphene_core::raster::{Alpha, BlendColorPairNode, Color, Image, Pixel, Sample};
use graphene_core::transform::{Footprint, Transform, TransformMut};
use graphene_core::value::{ClonedNode, CopiedNode, ValueNode};
use graphene_core::vector::brush_stroke::{BrushStroke, BrushStyle};
use graphene_core::vector::VectorDataTable;
use graphene_core::Node;

use glam::{DAffine2, DVec2};

#[node_macro::node(category("Debug"))]
fn vector_points(_: (), vector_data: VectorDataTable) -> Vec<DVec2> {
	let vector_data = vector_data.one_item();

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
fn blit<P: Alpha + Pixel + std::fmt::Debug, BlendFn>(mut target: ImageFrame<P>, texture: Image<P>, positions: Vec<DVec2>, blend_mode: BlendFn) -> ImageFrame<P>
where
	BlendFn: for<'any_input> Node<'any_input, (P, P), Output = P>,
{
	if positions.is_empty() {
		return target;
	}

	let target_size = DVec2::new(target.image.width as f64, target.image.height as f64);
	let texture_size = DVec2::new(texture.width as f64, texture.height as f64);
	let document_to_target = DAffine2::from_translation(-texture_size / 2.) * DAffine2::from_scale(target_size) * target.transform.inverse();

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
		let target_index = |x: u32, y: u32| -> usize { (y as usize * target.image.width as usize) + (x as usize) };

		let max_y = (blit_area_offset.y + blit_area_dimensions.y).saturating_sub(1);
		let max_x = (blit_area_offset.x + blit_area_dimensions.x).saturating_sub(1);
		assert!(texture_index(max_x, max_y) < texture.data.len());
		assert!(target_index(max_x, max_y) < target.image.data.len());

		for y in blit_area_offset.y..blit_area_offset.y + blit_area_dimensions.y {
			for x in blit_area_offset.x..blit_area_offset.x + blit_area_dimensions.x {
				let src_pixel = texture.data[texture_index(x, y)];
				let dst_pixel = &mut target.image.data[target_index(x + clamp_start.x, y + clamp_start.y)];
				*dst_pixel = blend_mode.eval((src_pixel, *dst_pixel));
			}
		}
	}

	target
}

pub fn create_brush_texture(brush_style: &BrushStyle) -> Image<Color> {
	let stamp = BrushStampGeneratorNode::new(CopiedNode::new(brush_style.color), CopiedNode::new(brush_style.hardness), CopiedNode::new(brush_style.flow));
	let stamp = stamp.eval(brush_style.diameter);
	let transform = DAffine2::from_scale_angle_translation(DVec2::splat(brush_style.diameter), 0., -DVec2::splat(brush_style.diameter / 2.));
	let blank_texture = EmptyImageNode::new(CopiedNode::new(transform), CopiedNode::new(Color::TRANSPARENT)).eval(());
	let normal_blend = BlendColorPairNode::new(CopiedNode::new(BlendMode::Normal), CopiedNode::new(100.));
	let blend_executor = BlendImageTupleNode::new(ValueNode::new(normal_blend));
	blend_executor.eval((blank_texture, stamp)).image
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

pub fn blend_with_mode(background: ImageFrame<Color>, foreground: ImageFrame<Color>, blend_mode: BlendMode, opacity: f64) -> ImageFrame<Color> {
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
fn brush(_: Footprint, image: ImageFrameTable<Color>, bounds: ImageFrameTable<Color>, strokes: Vec<BrushStroke>, cache: BrushCache) -> ImageFrameTable<Color> {
	let image = image.one_item().clone();

	let stroke_bbox = strokes.iter().map(|s| s.bounding_box()).reduce(|a, b| a.union(&b)).unwrap_or(AxisAlignedBbox::ZERO);
	let image_bbox = Bbox::from_transform(image.transform).to_axis_aligned_bbox();
	let bbox = if image_bbox.size().length() < 0.1 { stroke_bbox } else { stroke_bbox.union(&image_bbox) };

	let mut draw_strokes: Vec<_> = strokes.iter().filter(|&s| !matches!(s.style.blend_mode, BlendMode::Erase | BlendMode::Restore)).cloned().collect();
	let erase_restore_strokes: Vec<_> = strokes.iter().filter(|&s| matches!(s.style.blend_mode, BlendMode::Erase | BlendMode::Restore)).cloned().collect();

	let mut brush_plan = cache.compute_brush_plan(image, &draw_strokes);

	let mut background_bounds = bbox.to_transform();

	if bounds.transform() != DAffine2::ZERO {
		background_bounds = bounds.transform();
	}

	let mut actual_image = ExtendImageToBoundsNode::new(ClonedNode::new(background_bounds)).eval(brush_plan.background);
	let final_stroke_idx = brush_plan.strokes.len().saturating_sub(1);
	for (idx, stroke) in brush_plan.strokes.into_iter().enumerate() {
		// Create brush texture.
		// TODO: apply rotation from layer to stamp for non-rotationally-symmetric brushes.
		let brush_texture = cache.get_cached_brush(&stroke.style).unwrap_or_else(|| {
			let tex = create_brush_texture(&stroke.style);
			cache.store_brush(stroke.style.clone(), tex.clone());
			tex
		});

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

			let normal_blend = BlendColorPairNode::new(CopiedNode::new(BlendMode::Normal), CopiedNode::new(100.));
			let blit_node = BlitNode::new(ClonedNode::new(brush_texture), ClonedNode::new(positions), ClonedNode::new(normal_blend));
			let blit_target = if idx == 0 {
				let target = core::mem::take(&mut brush_plan.first_stroke_texture);
				ExtendImageToBoundsNode::new(CopiedNode::new(stroke_to_layer)).eval(target)
			} else {
				EmptyImageNode::new(CopiedNode::new(stroke_to_layer), CopiedNode::new(Color::TRANSPARENT)).eval(())
			};

			blit_node.eval(blit_target)
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
		let opaque_image = ImageFrame {
			image: Image::new(bbox.size().x as u32, bbox.size().y as u32, Color::WHITE),
			transform: background_bounds,
			..Default::default()
		};
		let mut erase_restore_mask = opaque_image;

		for stroke in erase_restore_strokes {
			let brush_texture = cache.get_cached_brush(&stroke.style).unwrap_or_else(|| {
				let tex = create_brush_texture(&stroke.style);
				cache.store_brush(stroke.style.clone(), tex.clone());
				tex
			});
			let positions: Vec<_> = stroke.compute_blit_points().into_iter().collect();

			match stroke.style.blend_mode {
				BlendMode::Erase => {
					let blend_params = BlendColorPairNode::new(CopiedNode::new(BlendMode::Erase), CopiedNode::new(100.));
					let blit_node = BlitNode::new(ClonedNode::new(brush_texture), ClonedNode::new(positions), ClonedNode::new(blend_params));
					erase_restore_mask = blit_node.eval(erase_restore_mask);
				}

				// Yes, this is essentially the same as the above, but we duplicate to inline the blend mode.
				BlendMode::Restore => {
					let blend_params = BlendColorPairNode::new(CopiedNode::new(BlendMode::Restore), CopiedNode::new(100.));
					let blit_node = BlitNode::new(ClonedNode::new(brush_texture), ClonedNode::new(positions), ClonedNode::new(blend_params));
					erase_restore_mask = blit_node.eval(erase_restore_mask);
				}

				_ => unreachable!(),
			}
		}

		let blend_params = BlendColorPairNode::new(CopiedNode::new(BlendMode::MultiplyAlpha), CopiedNode::new(100.));
		let blend_executor = BlendImageTupleNode::new(ValueNode::new(blend_params));
		actual_image = blend_executor.eval((actual_image, erase_restore_mask));
	}
	ImageFrameTable::new(actual_image)
}

#[cfg(test)]
mod test {
	use super::*;

	use graphene_core::transform::Transform;
	use graphene_core::value::ClonedNode;

	use glam::DAffine2;

	#[test]
	fn test_brush_texture() {
		let brush_texture_node = BrushStampGeneratorNode::new(ClonedNode::new(Color::BLACK), ClonedNode::new(100.), ClonedNode::new(100.));
		let size = 20.;
		let image = brush_texture_node.eval(size);
		assert_eq!(image.transform(), DAffine2::from_scale_angle_translation(DVec2::splat(size.ceil()), 0., -DVec2::splat(size / 2.)));
		// center pixel should be BLACK
		assert_eq!(image.sample(DVec2::splat(0.), DVec2::ONE), Some(Color::BLACK));
	}
}
