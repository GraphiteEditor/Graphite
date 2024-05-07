use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::misc::{GridSnapping, GridType};
use crate::messages::prelude::*;
use glam::DVec2;
use graphene_core::raster::color::Color;
use graphene_core::renderer::Quad;

fn grid_overlay_rectangular(document: &DocumentMessageHandler, overlay_context: &mut OverlayContext, spacing: DVec2) {
	let origin = document.snapping_state.grid.origin;
	let grid_color: Color = document.snapping_state.grid.grid_color;
	let Some(spacing) = GridSnapping::compute_rectangle_spacing(spacing, &document.navigation) else {
		return;
	};
	let document_to_viewport = document.metadata().document_to_viewport;
	let bounds = document_to_viewport.inverse() * Quad::from_box([DVec2::ZERO, overlay_context.size]);

	for primary in 0..2 {
		let secondary = 1 - primary;
		let min = bounds.0.iter().map(|&corner| corner[secondary]).min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or_default();
		let max = bounds.0.iter().map(|&corner| corner[secondary]).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or_default();
		let primary_start = bounds.0.iter().map(|&corner| corner[primary]).min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or_default();
		let primary_end = bounds.0.iter().map(|&corner| corner[primary]).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or_default();
		let spacing = spacing[secondary];
		for line_index in 0..=((max - min) / spacing).ceil() as i32 {
			let secondary_pos = (((min - origin[secondary]) / spacing).ceil() + line_index as f64) * spacing + origin[secondary];
			let start = if primary == 0 {
				DVec2::new(primary_start, secondary_pos)
			} else {
				DVec2::new(secondary_pos, primary_start)
			};
			let end = if primary == 0 {
				DVec2::new(primary_end, secondary_pos)
			} else {
				DVec2::new(secondary_pos, primary_end)
			};
			overlay_context.line(
				document_to_viewport.transform_point2(start),
				document_to_viewport.transform_point2(end),
				Some(&("#".to_owned() + &grid_color.rgb_hex())),
			);
		}
	}
}

//TODO: Potentially create an image and render the image onto the canvas a single time
fn grid_overlay_dot(document: &DocumentMessageHandler, overlay_context: &mut OverlayContext, spacing: DVec2) {
	let origin = document.snapping_state.grid.origin;
	let grid_color: Color = document.snapping_state.grid.grid_color;
	let Some(spacing) = GridSnapping::compute_rectangle_spacing(spacing, &document.navigation) else {
		return;
	};
	let document_to_viewport = document.metadata().document_to_viewport;
	let bounds = document_to_viewport.inverse() * Quad::from_box([DVec2::ZERO, overlay_context.size]);

	let min = bounds.0.iter().map(|&corner| corner[1]).min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or_default();
	let max = bounds.0.iter().map(|&corner| corner[1]).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or_default();
	let mut primary_start = bounds.0.iter().map(|&corner| corner[0]).min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or_default();
	let mut primary_end = bounds.0.iter().map(|&corner| corner[0]).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or_default();

	primary_start = (primary_start / spacing.x).ceil() * spacing.x;
	primary_end = (primary_end / spacing.x).ceil() * spacing.x;

	let spacing = spacing[0];

	let total_dots = ((primary_end - primary_start) / spacing).ceil();

	for line_index in 0..=((max - min) / spacing).ceil() as i32 {
		let secondary_pos = (((min - origin[1]) / spacing).ceil() + line_index as f64) * spacing + origin[1];
		let start = DVec2::new(primary_start, secondary_pos);
		let end = DVec2::new(primary_end, secondary_pos);

		let x_per_dot = (end.x - start.x) / total_dots;
		for dot_index in 0..total_dots as usize {
			let exact_x = x_per_dot * dot_index as f64;
			overlay_context.pixel(
				document_to_viewport.transform_point2(DVec2::new(start.x + exact_x, start.y)).round(),
				Some(&("#".to_owned() + &grid_color.rgb_hex())),
			)
		}
	}
}

fn grid_overlay_isometric(document: &DocumentMessageHandler, overlay_context: &mut OverlayContext, y_axis_spacing: f64, angle_a: f64, angle_b: f64) {
	let grid_color: Color = document.snapping_state.grid.grid_color;
	let cmp = |a: &f64, b: &f64| a.partial_cmp(b).unwrap();
	let origin = document.snapping_state.grid.origin;
	let document_to_viewport = document.metadata().document_to_viewport;
	let bounds = document_to_viewport.inverse() * Quad::from_box([DVec2::ZERO, overlay_context.size]);
	let tan_a = angle_a.to_radians().tan();
	let tan_b = angle_b.to_radians().tan();
	let spacing = DVec2::new(y_axis_spacing / (tan_a + tan_b), y_axis_spacing);
	let Some(spacing_multiplier) = GridSnapping::compute_isometric_multiplier(y_axis_spacing, tan_a + tan_b, &document.navigation) else {
		return;
	};
	let isometric_spacing = spacing * spacing_multiplier;

	let min_x = bounds.0.iter().map(|&corner| corner.x).min_by(cmp).unwrap_or_default();
	let max_x = bounds.0.iter().map(|&corner| corner.x).max_by(cmp).unwrap_or_default();
	let min_y = bounds.0.iter().map(|&corner| corner.y).min_by(cmp).unwrap_or_default();
	let max_y = bounds.0.iter().map(|&corner| corner.y).max_by(cmp).unwrap_or_default();
	let spacing = isometric_spacing.x;
	for line_index in 0..=((max_x - min_x) / spacing).ceil() as i32 {
		let x_pos = (((min_x - origin.x) / spacing).ceil() + line_index as f64) * spacing + origin.x;
		let start = DVec2::new(x_pos, min_y);
		let end = DVec2::new(x_pos, max_y);
		overlay_context.line(
			document_to_viewport.transform_point2(start),
			document_to_viewport.transform_point2(end),
			Some(&("#".to_owned() + &grid_color.rgb_hex())),
		);
	}

	for (tan, multiply) in [(tan_a, -1.), (tan_b, 1.)] {
		let project = |corner: &DVec2| corner.y + multiply * tan * (corner.x - origin.x);
		let inverse_project = |corner: &DVec2| corner.y - tan * multiply * (corner.x - origin.x);
		let min_y = bounds.0.into_iter().min_by(|a, b| inverse_project(a).partial_cmp(&inverse_project(b)).unwrap()).unwrap_or_default();
		let max_y = bounds.0.into_iter().max_by(|a, b| inverse_project(a).partial_cmp(&inverse_project(b)).unwrap()).unwrap_or_default();
		let spacing = isometric_spacing.y;
		let lines = ((inverse_project(&max_y) - inverse_project(&min_y)) / spacing).ceil() as i32;
		for line_index in 0..=lines {
			let y_pos = (((inverse_project(&min_y) - origin.y) / spacing).ceil() + line_index as f64) * spacing + origin.y;
			let start = DVec2::new(min_x, project(&DVec2::new(min_x, y_pos)));
			let end = DVec2::new(max_x, project(&DVec2::new(max_x, y_pos)));
			overlay_context.line(
				document_to_viewport.transform_point2(start),
				document_to_viewport.transform_point2(end),
				Some(&("#".to_owned() + &grid_color.rgb_hex())),
			);
		}
	}
}

pub fn grid_overlay(document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
	match document.snapping_state.grid.grid_type {
		GridType::Rectangle { spacing } => {
			if document.snapping_state.grid.dot_display {
				grid_overlay_dot(document, overlay_context, spacing)
			} else {
				grid_overlay_rectangular(document, overlay_context, spacing)
			}
		}
		GridType::Isometric { y_axis_spacing, angle_a, angle_b } => grid_overlay_isometric(document, overlay_context, y_axis_spacing, angle_a, angle_b),
	}
}

pub fn overlay_options(grid: &GridSnapping) -> Vec<LayoutGroup> {
	let mut widgets = Vec::new();
	fn update_val<I>(grid: &GridSnapping, update: impl Fn(&mut GridSnapping, &I)) -> impl Fn(&I) -> Message {
		let grid = grid.clone();
		move |input: &I| {
			let mut grid = grid.clone();
			update(&mut grid, input);
			DocumentMessage::GridOptions(grid).into()
		}
	}
	let update_origin = |grid, update: fn(&mut GridSnapping) -> Option<&mut f64>| {
		update_val::<NumberInput>(grid, move |grid, val| {
			if let Some(val) = val.value {
				if let Some(update) = update(grid) {
					*update = val;
				}
			}
		})
	};
	let update_color = |grid, update: fn(&mut GridSnapping) -> Option<&mut Color>| {
		update_val::<ColorButton>(grid, move |grid, val| {
			if let Some(val) = val.value {
				if let Some(update) = update(grid) {
					*update = val;
				}
			}
		})
	};
	let update_display = |grid, update: fn(&mut GridSnapping) -> Option<&mut bool>| {
		update_val::<CheckboxInput>(grid, move |grid, val| {
			if let Some(update) = update(grid) {
				*update = val.checked;
			}
		})
	};

	widgets.push(LayoutGroup::Row {
		widgets: vec![TextLabel::new("Grid").bold(true).widget_holder()],
	});
	widgets.push(LayoutGroup::Row {
		widgets: vec![
			TextLabel::new("Origin").table_align(true).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			NumberInput::new(Some(grid.origin.x))
				.label("X")
				.unit(" px")
				.min_width(98)
				.on_update(update_origin(grid, |grid| Some(&mut grid.origin.x)))
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			NumberInput::new(Some(grid.origin.y))
				.label("Y")
				.unit(" px")
				.min_width(98)
				.on_update(update_origin(grid, |grid| Some(&mut grid.origin.y)))
				.widget_holder(),
		],
	});
	widgets.push(LayoutGroup::Row {
		widgets: vec![
			TextLabel::new("Type").table_align(true).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			RadioInput::new(vec![
				RadioEntryData::new("rectangular")
					.label("Rectangular")
					.on_update(update_val(grid, |grid, _| grid.grid_type = GridType::RECTANGLE)),
				RadioEntryData::new("isometric")
					.label("Isometric")
					.on_update(update_val(grid, |grid, _| grid.grid_type = GridType::ISOMETRIC)),
			])
			.min_width(200)
			.selected_index(Some(match grid.grid_type {
				GridType::Rectangle { .. } => 0,
				GridType::Isometric { .. } => 1,
			}))
			.widget_holder(),
		],
	});

	match grid.grid_type {
		GridType::Rectangle { spacing } => widgets.push(LayoutGroup::Row {
			widgets: vec![
				TextLabel::new("Spacing").table_align(true).widget_holder(),
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				NumberInput::new(Some(spacing.x))
					.label("X")
					.unit(" px")
					.min(0.)
					.min_width(98)
					.on_update(update_origin(grid, |grid| grid.grid_type.rect_spacing().map(|spacing| &mut spacing.x)))
					.widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				NumberInput::new(Some(spacing.y))
					.label("Y")
					.unit(" px")
					.min(0.)
					.min_width(98)
					.on_update(update_origin(grid, |grid| grid.grid_type.rect_spacing().map(|spacing| &mut spacing.y)))
					.widget_holder(),
			],
		}),
		GridType::Isometric { y_axis_spacing, angle_a, angle_b } => {
			widgets.push(LayoutGroup::Row {
				widgets: vec![
					TextLabel::new("Y Spacing").table_align(true).widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					NumberInput::new(Some(y_axis_spacing))
						.unit(" px")
						.min(0.)
						.min_width(200)
						.on_update(update_origin(grid, |grid| grid.grid_type.isometric_y_spacing()))
						.widget_holder(),
				],
			});
			widgets.push(LayoutGroup::Row {
				widgets: vec![
					TextLabel::new("Angles").table_align(true).widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					NumberInput::new(Some(angle_a))
						.unit("°")
						.min_width(98)
						.on_update(update_origin(grid, |grid| grid.grid_type.angle_a()))
						.widget_holder(),
					Separator::new(SeparatorType::Related).widget_holder(),
					NumberInput::new(Some(angle_b))
						.unit("°")
						.min_width(98)
						.on_update(update_origin(grid, |grid| grid.grid_type.angle_b()))
						.widget_holder(),
				],
			});
		}
	}
	match grid.grid_type {
		GridType::Rectangle { .. } => widgets.push(LayoutGroup::Row {
			widgets: vec![
				TextLabel::new("Dot display").table_align(true).widget_holder(),
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				CheckboxInput::new(grid.dot_display).on_update(update_display(grid, |grid| Some(&mut grid.dot_display))).widget_holder(),
			],
		}),
		GridType::Isometric {
			y_axis_spacing: _,
			angle_a: _,
			angle_b: _,
		} => {}
	}
	widgets.push(LayoutGroup::Row {
		widgets: vec![
			TextLabel::new("Color").table_align(true).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			ColorButton::new(Some(grid.grid_color)).on_update(update_color(grid, |grid| Some(&mut grid.grid_color))).widget_holder(),
		],
	});

	widgets
}
