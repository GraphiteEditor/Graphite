use crate::brush_cache::BrushCache;
use crate::brush_stroke::{BrushStroke, BrushStyle};
use glam::{DAffine2, DVec2};
use graphene_core::blending::BlendMode;
use graphene_core::bounds::{BoundingBox, RenderBoundingBox};
use graphene_core::color::{Alpha, Color, Pixel, Sample};
use graphene_core::generic::FnNode;
use graphene_core::math::bbox::{AxisAlignedBbox, Bbox};
use graphene_core::raster::BitmapMut;
use graphene_core::raster::image::Image;
use graphene_core::raster_types::{CPU, Raster};
use graphene_core::registry::FutureWrapperNode;
use graphene_core::table::{Table, TableRow};
use graphene_core::transform::Transform;
use graphene_core::value::ClonedNode;
use graphene_core::{Ctx, Node};
use graphene_raster_nodes::blending_nodes::blend_colors;
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

		use graphene_core::color::Channel;
		Some(self.color.multiplied_alpha(P::AlphaChannel::from_linear(result)))
	}
}

#[node_macro::node(skip_impl)]
fn brush_stamp_generator(#[unit(" px")] diameter: f64, color: Color, hardness: f64, flow: f64) -> BrushStampGenerator<Color> {
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
fn blit<BlendFn>(mut target: Table<Raster<CPU>>, texture: Raster<CPU>, positions: Vec<DVec2>, blend_mode: BlendFn) -> Table<Raster<CPU>>
where
	BlendFn: for<'any_input> Node<'any_input, (Color, Color), Output = Color>,
{
	if positions.is_empty() {
		return target;
	}

	for table_row in target.iter_mut() {
		let target_width = table_row.element.width;
		let target_height = table_row.element.height;
		let target_size = DVec2::new(target_width as f64, target_height as f64);

		let texture_size = DVec2::new(texture.width as f64, texture.height as f64);

		let document_to_target = DAffine2::from_translation(-texture_size / 2.) * DAffine2::from_scale(target_size) * table_row.transform.inverse();

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
			assert!(target_index(max_x, max_y) < table_row.element.data.len());

			for y in blit_area_offset.y..blit_area_offset.y + blit_area_dimensions.y {
				for x in blit_area_offset.x..blit_area_offset.x + blit_area_dimensions.x {
					let src_pixel = texture.data[texture_index(x, y)];
					let dst_pixel = &mut table_row.element.data_mut().data[target_index(x + clamp_start.x, y + clamp_start.y)];
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
	let blank_texture = empty_image((), transform, Table::new_from_element(Color::TRANSPARENT)).into_iter().next().unwrap_or_default();
	let image = blend_stamp_closure(stamp, blank_texture, |a, b| blend_colors(a, b, BlendMode::Normal, 1.));

	image.element
}

pub fn blend_with_mode(background: TableRow<Raster<CPU>>, foreground: TableRow<Raster<CPU>>, blend_mode: BlendMode, opacity: f64) -> TableRow<Raster<CPU>> {
	let opacity = opacity as f32 / 100.;
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
async fn brush(_: impl Ctx, mut image_frame_table: Table<Raster<CPU>>, strokes: Vec<BrushStroke>, cache: BrushCache) -> Table<Raster<CPU>> {
	if image_frame_table.is_empty() {
		image_frame_table.push(TableRow::default());
	}
	// TODO: Find a way to handle more than one row
	let table_row = image_frame_table.iter().next().expect("Expected the one row we just pushed").into_cloned();

	let bounds = Table::new_from_row(table_row.clone()).bounding_box(DAffine2::IDENTITY, false);
	let [start, end] = if let RenderBoundingBox::Rectangle(rect) = bounds { rect } else { [DVec2::ZERO, DVec2::ZERO] };
	let image_bbox = AxisAlignedBbox { start, end };
	let stroke_bbox = strokes.iter().map(|s| s.bounding_box()).reduce(|a, b| a.union(&b)).unwrap_or(AxisAlignedBbox::ZERO);
	let bbox = if image_bbox.size().length() < 0.1 { stroke_bbox } else { stroke_bbox.union(&image_bbox) };
	let background_bounds = bbox.to_transform();

	let mut draw_strokes: Vec<_> = strokes.iter().filter(|&s| !matches!(s.style.blend_mode, BlendMode::Erase | BlendMode::Restore)).cloned().collect();

	let mut brush_plan = cache.compute_brush_plan(table_row, &draw_strokes);

	// TODO: Find a way to handle more than one row
	let Some(mut actual_image) = extend_image_to_bounds((), Table::new_from_row(brush_plan.background), background_bounds).into_iter().next() else {
		return Table::new();
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

			let normal_blend = FnNode::new(|(a, b)| blend_colors(a, b, BlendMode::Normal, 1.));
			let blit_node = BlitNode::new(
				FutureWrapperNode::new(ClonedNode::new(brush_texture)),
				FutureWrapperNode::new(ClonedNode::new(positions)),
				FutureWrapperNode::new(ClonedNode::new(normal_blend)),
			);
			let blit_target = if idx == 0 {
				let target = core::mem::take(&mut brush_plan.first_stroke_texture);
				extend_image_to_bounds((), Table::new_from_row(target), stroke_to_layer)
			} else {
				empty_image((), stroke_to_layer, Table::new_from_element(Color::TRANSPARENT))
				// EmptyImageNode::new(CopiedNode::new(stroke_to_layer), CopiedNode::new(Color::TRANSPARENT)).eval(())
			};

			let table = blit_node.eval(blit_target).await;
			assert_eq!(table.len(), 1);
			table.into_iter().next().unwrap_or_default()
		};

		// Cache image before doing final blend, and store final stroke texture.
		if idx == final_stroke_idx {
			cache.cache_results(core::mem::take(&mut draw_strokes), actual_image.clone(), stroke_texture.clone());
		}

		// TODO: Is this the correct way to do opacity in blending?
		actual_image = blend_with_mode(actual_image, stroke_texture, stroke.style.blend_mode, (stroke.style.color.a() * 100.) as f64);
	}

	let has_erase_or_restore_strokes = strokes.iter().any(|s| matches!(s.style.blend_mode, BlendMode::Erase | BlendMode::Restore));
	if has_erase_or_restore_strokes {
		let opaque_image = Image::new(bbox.size().x as u32, bbox.size().y as u32, Color::WHITE);
		let mut erase_restore_mask = TableRow {
			element: Raster::new_cpu(opaque_image),
			transform: background_bounds,
			..Default::default()
		};

		for stroke in strokes {
			let mut brush_texture = cache.get_cached_brush(&stroke.style);
			if brush_texture.is_none() {
				let tex = create_brush_texture(&stroke.style).await;
				cache.store_brush(stroke.style.clone(), tex.clone());
				brush_texture = Some(tex);
			}
			let brush_texture = brush_texture.unwrap();
			let positions: Vec<_> = stroke.compute_blit_points().into_iter().collect();

			// For mask composition: Erase subtracts alpha, Restore adds alpha, and Draw acts like Restore to allow repainting erased areas.
			let mask_blend_mode = match stroke.style.blend_mode {
				BlendMode::Erase => BlendMode::Erase,
				BlendMode::Restore => BlendMode::Restore,
				_ => BlendMode::Restore,
			};

			let blend_params = FnNode::new(move |(a, b)| blend_colors(a, b, mask_blend_mode, 1.));
			let blit_node = BlitNode::new(
				FutureWrapperNode::new(ClonedNode::new(brush_texture)),
				FutureWrapperNode::new(ClonedNode::new(positions)),
				FutureWrapperNode::new(ClonedNode::new(blend_params)),
			);
			erase_restore_mask = blit_node.eval(Table::new_from_row(erase_restore_mask)).await.into_iter().next().unwrap_or_default();
		}

		let blend_params = FnNode::new(|(a, b)| blend_colors(a, b, BlendMode::MultiplyAlpha, 1.));
		actual_image = blend_image_closure(erase_restore_mask, actual_image, |a, b| blend_params.eval((a, b)));
	}

	let first_row = image_frame_table.iter_mut().next().unwrap();
	*first_row.element = actual_image.element;
	*first_row.transform = actual_image.transform;
	*first_row.alpha_blending = actual_image.alpha_blending;
	*first_row.source_node_id = actual_image.source_node_id;

	image_frame_table
}

pub fn blend_image_closure(foreground: TableRow<Raster<CPU>>, mut background: TableRow<Raster<CPU>>, map_fn: impl Fn(Color, Color) -> Color) -> TableRow<Raster<CPU>> {
	let foreground_size = DVec2::new(foreground.element.width as f64, foreground.element.height as f64);
	let background_size = DVec2::new(background.element.width as f64, background.element.height as f64);

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

			let source_pixel = foreground.element.sample(foreground_point);
			let Some(destination_pixel) = background.element.data_mut().get_pixel_mut(x, y) else { continue };

			*destination_pixel = map_fn(source_pixel, *destination_pixel);
		}
	}

	background
}

pub fn blend_stamp_closure(foreground: BrushStampGenerator<Color>, mut background: TableRow<Raster<CPU>>, map_fn: impl Fn(Color, Color) -> Color) -> TableRow<Raster<CPU>> {
	let background_size = DVec2::new(background.element.width as f64, background.element.height as f64);

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
			let Some(destination_pixel) = background.element.data_mut().get_pixel_mut(x, y) else { continue };

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
			Table::new_from_element(Raster::new_cpu(Image::<Color>::default())),
			vec![BrushStroke {
				trace: vec![crate::brush_stroke::BrushInputSample { position: DVec2::ZERO }],
				style: BrushStyle {
					color: Color::BLACK,
					diameter: 20.,
					hardness: 20.,
					flow: 20.,
					spacing: 20.,
					blend_mode: BlendMode::Normal,
				},
			}],
			BrushCache::default(),
		)
		.await;
		assert_eq!(image.iter().next().unwrap().element.width, 20);
	}
}
