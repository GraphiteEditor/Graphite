use glam::{DAffine2, DVec2};
use graph_craft::generic::FnNode;
use graph_craft::proto::FutureWrapperNode;
use graphene_core::bounds::BoundingBox;
use graphene_core::instances::Instance;
use graphene_core::math::bbox::{AxisAlignedBbox, Bbox};
use graphene_core::raster::brush_cache::BrushCache;
use graphene_core::raster::image::Image;
use graphene_core::raster::{Alpha, BitmapMut, BlendMode, Color, Pixel, Sample};
use graphene_core::raster_types::{CPU, Raster, RasterDataTable};
use graphene_core::transform::Transform;
use graphene_core::value::ClonedNode;
use graphene_core::vector::brush_stroke::{BrushStroke, BrushStyle};
use graphene_core::{Ctx, GraphicElement, Node};
use graphene_raster_nodes::adjustments::blend_colors;
use graphene_raster_nodes::std_nodes::{empty_image, extend_image_to_bounds};

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
fn blit<BlendFn>(mut target: RasterDataTable<CPU>, texture: Raster<CPU>, positions: Vec<DVec2>, blend_mode: BlendFn) -> RasterDataTable<CPU>
where
	BlendFn: for<'any_input> Node<'any_input, (Color, Color), Output = Color>,
	GraphicElement: From<Raster<CPU>>,
{
	if positions.is_empty() {
		return target;
	}

	for target_instance in target.instance_mut_iter() {
		let target_width = target_instance.instance.width;
		let target_height = target_instance.instance.height;
		let target_size = DVec2::new(target_width as f64, target_height as f64);

		let texture_size = DVec2::new(texture.width as f64, texture.height as f64);

		let document_to_target = DAffine2::from_translation(-texture_size / 2.) * DAffine2::from_scale(target_size) * target_instance.transform.inverse();

		for position in &positions {
			let start = document_to_target.transform_point2(*position).round();
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
			assert!(target_index(max_x, max_y) < target_instance.instance.data.len());

			for y in blit_area_offset.y..blit_area_offset.y + blit_area_dimensions.y {
				for x in blit_area_offset.x..blit_area_offset.x + blit_area_dimensions.x {
					let src_pixel = texture.data[texture_index(x, y)];
					let dst_pixel = &mut target_instance.instance.data_mut().data[target_index(x + clamp_start.x, y + clamp_start.y)];
					*dst_pixel = blend_mode.eval((src_pixel, *dst_pixel));
				}
			}
		}
	}

	target
}

pub async fn create_brush_texture(brush_style: &BrushStyle) -> Raster<CPU> {
	let stamp = brush_stamp_generator(brush_style.diameter, brush_style.color, brush_style.hardness, brush_style.flow);
	let transform = DAffine2::from_scale_angle_translation(DVec2::splat(brush_style.diameter), 0., -DVec2::splat(brush_style.diameter / 2.));
	let blank_texture = empty_image((), transform, Color::TRANSPARENT).instance_iter().next().unwrap_or_default();
	let image = blend_stamp_closure(stamp, blank_texture, |a, b| blend_colors(a, b, BlendMode::Normal, 1.));

	image.instance
}

pub fn blend_with_mode(background: Instance<Raster<CPU>>, foreground: Instance<Raster<CPU>>, blend_mode: BlendMode, opacity: f64) -> Instance<Raster<CPU>> {
	let opacity = opacity / 100.;
	match std::hint::black_box(blend_mode) {
		// Normal group
		BlendMode::Normal => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::Normal, opacity)),
		// Darken group
		BlendMode::Darken => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::Darken, opacity)),
		BlendMode::Multiply => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::Multiply, opacity)),
		BlendMode::ColorBurn => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::ColorBurn, opacity)),
		BlendMode::LinearBurn => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::LinearBurn, opacity)),
		BlendMode::DarkerColor => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::DarkerColor, opacity)),
		// Lighten group
		BlendMode::Lighten => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::Lighten, opacity)),
		BlendMode::Screen => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::Screen, opacity)),
		BlendMode::ColorDodge => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::ColorDodge, opacity)),
		BlendMode::LinearDodge => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::LinearDodge, opacity)),
		BlendMode::LighterColor => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::LighterColor, opacity)),
		// Contrast group
		BlendMode::Overlay => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::Overlay, opacity)),
		BlendMode::SoftLight => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::SoftLight, opacity)),
		BlendMode::HardLight => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::HardLight, opacity)),
		BlendMode::VividLight => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::VividLight, opacity)),
		BlendMode::LinearLight => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::LinearLight, opacity)),
		BlendMode::PinLight => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::PinLight, opacity)),
		BlendMode::HardMix => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::HardMix, opacity)),
		// Inversion group
		BlendMode::Difference => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::Difference, opacity)),
		BlendMode::Exclusion => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::Exclusion, opacity)),
		BlendMode::Subtract => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::Subtract, opacity)),
		BlendMode::Divide => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::Divide, opacity)),
		// Component group
		BlendMode::Hue => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::Hue, opacity)),
		BlendMode::Saturation => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::Saturation, opacity)),
		BlendMode::Color => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::Color, opacity)),
		BlendMode::Luminosity => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::Luminosity, opacity)),
		// Other utility blend modes (hidden from the normal list)
		BlendMode::Erase => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::Erase, opacity)),
		BlendMode::Restore => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::Restore, opacity)),
		BlendMode::MultiplyAlpha => blend_image_closure(foreground, background, |a, b| blend_colors(a, b, BlendMode::MultiplyAlpha, opacity)),
	}
}

#[node_macro::node(category("Raster"))]
async fn brush(_: impl Ctx, mut image_frame_table: RasterDataTable<CPU>, strokes: Vec<BrushStroke>, cache: BrushCache) -> RasterDataTable<CPU> {
	if image_frame_table.is_empty() {
		image_frame_table.push(Instance::default());
	}
	// TODO: Find a way to handle more than one instance
	let image_frame_instance = image_frame_table.instance_ref_iter().next().expect("Expected the one instance we just pushed").to_instance_cloned();

	let [start, end] = image_frame_instance.clone().to_table().bounding_box(DAffine2::IDENTITY, false).unwrap_or([DVec2::ZERO, DVec2::ZERO]);
	let image_bbox = AxisAlignedBbox { start, end };
	let stroke_bbox = strokes.iter().map(|s| s.bounding_box()).reduce(|a, b| a.union(&b)).unwrap_or(AxisAlignedBbox::ZERO);
	let bbox = if image_bbox.size().length() < 0.1 { stroke_bbox } else { stroke_bbox.union(&image_bbox) };
	let background_bounds = bbox.to_transform();

	let mut draw_strokes: Vec<_> = strokes.iter().filter(|&s| !matches!(s.style.blend_mode, BlendMode::Erase | BlendMode::Restore)).cloned().collect();
	let erase_restore_strokes: Vec<_> = strokes.iter().filter(|&s| matches!(s.style.blend_mode, BlendMode::Erase | BlendMode::Restore)).cloned().collect();

	let mut brush_plan = cache.compute_brush_plan(image_frame_instance, &draw_strokes);

	// TODO: Find a way to handle more than one instance
	let Some(mut actual_image) = extend_image_to_bounds((), brush_plan.background.to_table(), background_bounds).instance_iter().next() else {
		return RasterDataTable::default();
	};

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
				extend_image_to_bounds((), target.to_table(), stroke_to_layer)
			} else {
				empty_image((), stroke_to_layer, Color::TRANSPARENT)
				// EmptyImageNode::new(CopiedNode::new(stroke_to_layer), CopiedNode::new(Color::TRANSPARENT)).eval(())
			};

			let instances = blit_node.eval(blit_target).await;
			assert_eq!(instances.len(), 1);
			instances.instance_iter().next().unwrap_or_default()
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
		let mut erase_restore_mask = Instance {
			instance: Raster::new_cpu(opaque_image),
			transform: background_bounds,
			..Default::default()
		};

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
					erase_restore_mask = blit_node.eval(erase_restore_mask.to_table()).await.instance_iter().next().unwrap_or_default();
				}
				// Yes, this is essentially the same as the above, but we duplicate to inline the blend mode.
				BlendMode::Restore => {
					let blend_params = FnNode::new(|(a, b)| blend_colors(a, b, BlendMode::Restore, 1.));
					let blit_node = BlitNode::new(
						FutureWrapperNode::new(ClonedNode::new(brush_texture)),
						FutureWrapperNode::new(ClonedNode::new(positions)),
						FutureWrapperNode::new(ClonedNode::new(blend_params)),
					);
					erase_restore_mask = blit_node.eval(erase_restore_mask.to_table()).await.instance_iter().next().unwrap_or_default();
				}
				_ => unreachable!(),
			}
		}

		let blend_params = FnNode::new(|(a, b)| blend_colors(a, b, BlendMode::MultiplyAlpha, 1.));
		actual_image = blend_image_closure(erase_restore_mask, actual_image, |a, b| blend_params.eval((a, b)));
	}

	let first_row = image_frame_table.instance_mut_iter().next().unwrap();
	*first_row.instance = actual_image.instance;
	*first_row.transform = actual_image.transform;
	*first_row.alpha_blending = actual_image.alpha_blending;
	*first_row.source_node_id = actual_image.source_node_id;

	image_frame_table
}

pub fn blend_image_closure(foreground: Instance<Raster<CPU>>, mut background: Instance<Raster<CPU>>, map_fn: impl Fn(Color, Color) -> Color) -> Instance<Raster<CPU>> {
	let foreground_size = DVec2::new(foreground.instance.width as f64, foreground.instance.height as f64);
	let background_size = DVec2::new(background.instance.width as f64, background.instance.height as f64);

	// Transforms a point from the background image to the foreground image
	let background_to_foreground = DAffine2::from_scale(foreground_size) * foreground.transform.inverse() * background.transform * DAffine2::from_scale(1. / background_size);

	// Footprint of the foreground image (0, 0)..(1, 1) in the background image space
	let background_aabb = Bbox::unit().affine_transform(background.transform.inverse() * foreground.transform).to_axis_aligned_bbox();

	// Clamp the foreground image to the background image
	let start = (background_aabb.start * background_size).max(DVec2::ZERO).as_uvec2();
	let end = (background_aabb.end * background_size).min(background_size).as_uvec2();

	for y in start.y..end.y {
		for x in start.x..end.x {
			let background_point = DVec2::new(x as f64, y as f64);
			let foreground_point = background_to_foreground.transform_point2(background_point);

			let source_pixel = foreground.instance.sample(foreground_point);
			let Some(destination_pixel) = background.instance.data_mut().get_pixel_mut(x, y) else { continue };

			*destination_pixel = map_fn(source_pixel, *destination_pixel);
		}
	}

	background
}

pub fn blend_stamp_closure(foreground: BrushStampGenerator<Color>, mut background: Instance<Raster<CPU>>, map_fn: impl Fn(Color, Color) -> Color) -> Instance<Raster<CPU>> {
	let background_size = DVec2::new(background.instance.width as f64, background.instance.height as f64);

	// Transforms a point from the background image to the foreground image
	let background_to_foreground = background.transform * DAffine2::from_scale(1. / background_size);

	// Footprint of the foreground image (0, 0)..(1, 1) in the background image space
	let background_aabb = Bbox::unit().affine_transform(background.transform.inverse() * foreground.transform).to_axis_aligned_bbox();

	// Clamp the foreground image to the background image
	let start = (background_aabb.start * background_size).max(DVec2::ZERO).as_uvec2();
	let end = (background_aabb.end * background_size).min(background_size).as_uvec2();

	let area = background_to_foreground.transform_point2(DVec2::new(1., 1.)) - background_to_foreground.transform_point2(DVec2::ZERO);
	for y in start.y..end.y {
		for x in start.x..end.x {
			let background_point = DVec2::new(x as f64, y as f64);
			let foreground_point = background_to_foreground.transform_point2(background_point);

			let Some(source_pixel) = foreground.sample(foreground_point, area) else { continue };
			let Some(destination_pixel) = background.instance.data_mut().get_pixel_mut(x, y) else { continue };

			*destination_pixel = map_fn(source_pixel, *destination_pixel);
		}
	}

	background
}

#[cfg(test)]
mod test {
	use super::*;
	use glam::DAffine2;
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
			RasterDataTable::<CPU>::new(Raster::new_cpu(Image::<Color>::default())),
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
		assert_eq!(image.instance_ref_iter().next().unwrap().instance.width, 20);
	}
}
