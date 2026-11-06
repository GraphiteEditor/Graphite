use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::misc::{GridLevels, GridSnapping, GridType, SnappingState};
use crate::messages::prelude::*;
use glam::{DAffine2, DVec2};
use graphene_std::color::SRGBA8;
use graphene_std::renderer::Quad;
use graphene_std::vector::style::FillChoice;

/// How opaque minor grid lines are, as a fraction of the grid color's opacity.
const MINOR_LINE_OPACITY: f64 = 0.5;

/// About how wide a dot of a dotted grid is drawn, in pixels, before its edges are put on whole physical pixels.
const DOT_SIZE: f64 = 1.5;

/// The most lines of one family drawn across the view, past which the family is left undrawn.
const MAX_LINES_IN_VIEW: f64 = 10_000.;

/// The width of the label column in the grid options.
const LABEL_WIDTH: u32 = 60;

/// How the document is seen in the viewport.
struct View {
	document_to_viewport: DAffine2,
	/// The viewport's corners in document space.
	corners: Quad,
	size: DVec2,
}

impl View {
	fn new(document: &DocumentMessageHandler, overlay_context: &OverlayContext) -> Self {
		let document_to_viewport = document
			.navigation_handler
			.calculate_offset_transform(overlay_context.viewport.center_in_viewport_space().into(), &document.document_ptz);
		let size: DVec2 = overlay_context.viewport.size().into();
		let corners = document_to_viewport.inverse() * Quad::from_box([DVec2::ZERO, size]);

		Self { document_to_viewport, corners, size }
	}

	/// The first and last whole steps of `spacing` along `axis` from `origin` within the view, or `None` when there are too many to draw.
	fn steps_in_view(&self, origin: DVec2, axis: DVec2, spacing: f64) -> Option<(f64, f64)> {
		let steps = self.corners.0.map(|corner| (corner - origin).dot(axis) / spacing);
		let first = steps.into_iter().fold(f64::INFINITY, f64::min).ceil();
		let last = steps.into_iter().fold(f64::NEG_INFINITY, f64::max).floor();

		let count = last - first;
		(count.is_finite() && count <= MAX_LINES_IN_VIEW).then_some((first, last))
	}
}

/// A color given as CSS hex, with its opacity scaled.
fn faded_color(color: &str, opacity: f64) -> String {
	let Some(mut faded) = SRGBA8::from_hex_str(color) else { return color.to_string() };
	faded.alpha = (faded.alpha as f64 * opacity).round() as u8;
	faded.to_css_hex()
}

/// Draws the lines running along `direction` that lie whole numbers of `spacing` across from `origin`, over the part of the document in view.
/// Every `skip_every`th line from the origin is left for the major lines to draw.
fn draw_line_family(overlay_context: &mut OverlayContext, view: &View, origin: DVec2, direction: DVec2, spacing: f64, skip_every: Option<u32>, color: &str) {
	let across = direction.perp();
	let Some((first, last)) = view.steps_in_view(origin, across, spacing) else { return };

	let along = view.corners.0.map(|corner| (corner - origin).dot(direction));
	let start = along.into_iter().fold(f64::INFINITY, f64::min);
	let end = along.into_iter().fold(f64::NEG_INFINITY, f64::max);

	// O(lines in view), which the coarsening of the spacing keeps to around the viewport's size over 5 pixels
	for step in (first as i64)..=(last as i64) {
		if skip_every.is_some_and(|every| step.rem_euclid(every as i64) == 0) {
			continue;
		}

		let through = origin + across * (step as f64 * spacing);
		let [start, end] = [start, end].map(|along| view.document_to_viewport.transform_point2(through + direction * along));
		overlay_context.line(start, end, Some(color), None);
	}
}

fn grid_overlay_rectangular(overlay_context: &mut OverlayContext, view: &View, origin: DVec2, levels: GridLevels, subdivisions: u32, color: &str) {
	// Minor lines go underneath, leaving out the ones the major lines are drawn over
	if let Some(minor) = levels.minor {
		let faded = faded_color(color, MINOR_LINE_OPACITY);
		for (direction, spacing) in levels.families(minor) {
			draw_line_family(overlay_context, view, origin, direction, spacing, Some(subdivisions), &faded);
		}
	}

	for (direction, spacing) in levels.families(levels.major) {
		draw_line_family(overlay_context, view, origin, direction, spacing, None, color);
	}
}

/// Draws a dot where each pair of the finest lines drawn cross, over the part of the document in view, fainter where they aren't both major lines.
fn grid_overlay_rectangular_dot(overlay_context: &mut OverlayContext, view: &View, origin: DVec2, levels: GridLevels, subdivisions: u32, color: &str) {
	let spacing = levels.finest();
	let (x_axis, y_axis) = (levels.x_axis, levels.y_axis());
	let Some((first_x, last_x)) = view.steps_in_view(origin, x_axis, spacing.x) else { return };
	let Some((first_y, last_y)) = view.steps_in_view(origin, y_axis, spacing.y) else { return };
	let major_every = if levels.minor.is_some() { subdivisions.max(1) as i64 } else { 1 };

	// O(dots in view), which the coarsening of the spacing keeps to around the viewport's area over 25 square pixels
	let (mut major, mut minor) = (Vec::new(), Vec::new());
	for x in (first_x as i64)..=(last_x as i64) {
		for y in (first_y as i64)..=(last_y as i64) {
			let dot = origin + x_axis * (x as f64 * spacing.x) + y_axis * (y as f64 * spacing.y);
			let dot = view.document_to_viewport.transform_point2(dot);

			// A turned grid's steps span past the viewport's corners
			if dot.cmpge(DVec2::ZERO).all() && dot.cmple(view.size).all() {
				let on_major_lines = x.rem_euclid(major_every) == 0 && y.rem_euclid(major_every) == 0;
				if on_major_lines { major.push(dot) } else { minor.push(dot) }
			}
		}
	}

	overlay_context.dots(&minor, DOT_SIZE, Some(&faded_color(color, MINOR_LINE_OPACITY)));
	overlay_context.dots(&major, DOT_SIZE, Some(color));
}

fn grid_overlay_isometric(document: &DocumentMessageHandler, overlay_context: &mut OverlayContext, view: &View, origin: DVec2, y_axis_spacing: f64, angle_a: f64, angle_b: f64) {
	let grid_color = document.snapping_state.grid.color.as_str();
	let tan_a = angle_a.to_radians().tan();
	let tan_b = angle_b.to_radians().tan();
	let spacing = DVec2::new(y_axis_spacing / (tan_a + tan_b), y_axis_spacing);
	let Some(spacing_multiplier) = GridSnapping::compute_isometric_multiplier(y_axis_spacing, tan_a + tan_b, &document.document_ptz) else {
		return;
	};
	let isometric_spacing = spacing * spacing_multiplier;

	draw_line_family(overlay_context, view, origin, DVec2::Y, isometric_spacing.x, None, grid_color);

	// The slanted lines are spaced along Y, which is further apart than they are straight across
	for slope in [-tan_a, tan_b] {
		let Some(direction) = DVec2::new(1., slope).try_normalize() else { continue };
		draw_line_family(overlay_context, view, origin, direction, isometric_spacing.y / slope.hypot(1.), None, grid_color);
	}
}

/// Draws a dot where the isometric grid's lines cross, over the part of the document in view.
fn grid_overlay_isometric_dot(document: &DocumentMessageHandler, overlay_context: &mut OverlayContext, view: &View, origin: DVec2, y_axis_spacing: f64, angle_a: f64, angle_b: f64) {
	let grid_color = document.snapping_state.grid.color.as_str();
	let tan_a = angle_a.to_radians().tan();
	let tan_b = angle_b.to_radians().tan();
	let spacing = DVec2::new(y_axis_spacing / (tan_a + tan_b), y_axis_spacing);
	let Some(spacing_multiplier) = GridSnapping::compute_isometric_multiplier(y_axis_spacing, tan_a + tan_b, &document.document_ptz) else {
		return;
	};
	let spacing = spacing * spacing_multiplier;

	// The dots stand in columns on the vertical lines, each column starting lower than the last along the slope of the first slanted lines
	// O(dots in view), which the coarsening of the spacing keeps to around the viewport's area over 100 square pixels
	let Some((first_x, last_x)) = view.steps_in_view(origin, DVec2::X, spacing.x) else { return };
	let mut dots = Vec::new();
	for x in (first_x as i64)..=(last_x as i64) {
		let column = origin + DVec2::new(1., -tan_a) * (x as f64 * spacing.x);
		let Some((first_y, last_y)) = view.steps_in_view(column, DVec2::Y, spacing.y) else { continue };

		for y in (first_y as i64)..=(last_y as i64) {
			let dot = view.document_to_viewport.transform_point2(column + DVec2::Y * (y as f64 * spacing.y));
			if dot.cmpge(DVec2::ZERO).all() && dot.cmple(view.size).all() {
				dots.push(dot);
			}
		}
	}

	overlay_context.dots(&dots, DOT_SIZE, Some(grid_color));
}

pub fn grid_overlay(document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
	let grid = &document.snapping_state.grid;
	let view = View::new(document, overlay_context);
	let origin = grid.effective_origin(document);
	if !origin.is_finite() {
		return;
	}

	match grid.grid_type {
		GridType::Rectangular { .. } => {
			let Some(levels) = grid.drawn_levels(&document.document_ptz) else { return };

			if grid.dot_display {
				grid_overlay_rectangular_dot(overlay_context, &view, origin, levels, grid.subdivisions, &grid.color)
			} else {
				grid_overlay_rectangular(overlay_context, &view, origin, levels, grid.subdivisions, &grid.color)
			}
		}
		GridType::Isometric { y_axis_spacing, angle_a, angle_b } => {
			if grid.dot_display {
				grid_overlay_isometric_dot(document, overlay_context, &view, origin, y_axis_spacing, angle_a, angle_b)
			} else {
				grid_overlay_isometric(document, overlay_context, &view, origin, y_axis_spacing, angle_a, angle_b)
			}
		}
	}
}

pub fn overlay_options(snapping_state: &SnappingState) -> Vec<LayoutGroup> {
	let grid = &snapping_state.grid;
	let mut widgets = Vec::new();
	fn update_val<I, F: Fn(&mut GridSnapping, &I)>(grid: &GridSnapping, update: F) -> impl Fn(&I) -> Message + use<I, F> {
		let grid = grid.clone();
		move |input: &I| {
			let mut grid = grid.clone();
			update(&mut grid, input);
			DocumentMessage::GridOptions { options: grid }.into()
		}
	}
	let update_number = |grid, update: fn(&mut GridSnapping) -> Option<&mut f64>| {
		update_val::<NumberInput, _>(grid, move |grid, val| {
			if let Some(val) = val.value
				&& let Some(update) = update(grid)
			{
				*update = val;
			}
		})
	};
	let update_checkbox = |grid, update: fn(&mut GridSnapping) -> Option<&mut bool>| {
		update_val::<CheckboxInput, _>(grid, move |grid, checkbox| {
			if let Some(update) = update(grid) {
				*update = checkbox.checked;
			}
		})
	};
	let label = |text: &str| TextLabel::new(text).sizing(WidgetSizing::fixed(LABEL_WIDTH)).table_align(true).widget_instance();

	let snap_checkbox_id = CheckboxId::new();
	let snap_description = "Snaps to the points where the drawn grid lines cross.";
	widgets.push(LayoutGroup::row(vec![
		TextLabel::new("Grid").bold(true).sizing(WidgetSizing::fixed(LABEL_WIDTH)).table_align(true).widget_instance(),
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		CheckboxInput::new(snapping_state.grid_snapping)
			.icon("Snapping")
			.on_update(|input: &CheckboxInput| {
				DocumentMessage::SetSnapping {
					closure: Some(|snapping_state| &mut snapping_state.grid_snapping),
					snapping_state: input.checked,
				}
				.into()
			})
			.tooltip_label("Snap to Grid")
			.tooltip_description(snap_description)
			.for_label(snap_checkbox_id)
			.widget_instance(),
		TextLabel::new("Snap to Grid")
			.tooltip_label("Snap to Grid")
			.tooltip_description(snap_description)
			.for_checkbox(snap_checkbox_id)
			.widget_instance(),
	]));

	widgets.push(LayoutGroup::row(vec![
		label("Type"),
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		RadioInput::new(vec![
			RadioEntryData::new("rectangular").label("Rectangular").on_update(update_val(grid, |grid, _| {
				if let GridType::Isometric { y_axis_spacing, angle_a, angle_b } = grid.grid_type {
					grid.isometric_y_spacing = y_axis_spacing;
					grid.isometric_angle_a = angle_a;
					grid.isometric_angle_b = angle_b;
				}
				grid.grid_type = GridType::Rectangular { spacing: grid.rectangular_spacing };
			})),
			RadioEntryData::new("isometric").label("Isometric").on_update(update_val(grid, |grid, _| {
				if let GridType::Rectangular { spacing } = grid.grid_type {
					grid.rectangular_spacing = spacing;
				}
				grid.grid_type = GridType::Isometric {
					y_axis_spacing: grid.isometric_y_spacing,
					angle_a: grid.isometric_angle_a,
					angle_b: grid.isometric_angle_b,
				};
			})),
		])
		.sizing(WidgetSizing::default().min_width(200))
		.selected_index(Some(match grid.grid_type {
			GridType::Rectangular { .. } => 0,
			GridType::Isometric { .. } => 1,
		}))
		.widget_instance(),
	]));

	let mut color_widgets = vec![label("Display"), Separator::new(SeparatorStyle::Unrelated).widget_instance()];
	color_widgets.extend([
		CheckboxInput::new(grid.dot_display)
			.icon("GridDotted")
			.tooltip_label("Display as Dotted Grid")
			.on_update(update_checkbox(grid, |grid| Some(&mut grid.dot_display)))
			.widget_instance(),
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
	]);
	color_widgets.push(
		ColorInput::new(FillChoice::<SRGBA8>::Solid(SRGBA8::from_hex_str(&grid.color).unwrap_or(SRGBA8::BLACK)))
			.tooltip_label("Grid Display Color")
			.allow_none(false)
			.on_update(update_val::<ColorInput, _>(grid, |grid, color| {
				if let Some(srgba) = color.value.as_solid() {
					grid.color = srgba.to_css_hex();
				}
			}))
			.widget_instance(),
	);
	widgets.push(LayoutGroup::row(color_widgets));

	let follow_checkbox_id = CheckboxId::new();
	let follow_description = "Measures the origin from the top left corner of the artboard containing the selection, or else the first artboard.";
	widgets.push(LayoutGroup::row(vec![
		label("Origin"),
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		CheckboxInput::new(grid.origin_follows_artboard)
			.tooltip_label("Origin Follows Artboard")
			.tooltip_description(follow_description)
			.on_update(update_checkbox(grid, |grid| Some(&mut grid.origin_follows_artboard)))
			.for_label(follow_checkbox_id)
			.widget_instance(),
		TextLabel::new("Origin Follows Artboard")
			.tooltip_label("Origin Follows Artboard")
			.tooltip_description(follow_description)
			.for_checkbox(follow_checkbox_id)
			.widget_instance(),
	]));

	widgets.push(LayoutGroup::row(vec![
		label(""),
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		NumberInput::new(Some(grid.origin.x))
			.label("X")
			.unit(" px")
			.sizing(WidgetSizing::default().min_width(98))
			.on_update(update_number(grid, |grid| Some(&mut grid.origin.x)))
			.widget_instance(),
		Separator::new(SeparatorStyle::Related).widget_instance(),
		NumberInput::new(Some(grid.origin.y))
			.label("Y")
			.unit(" px")
			.sizing(WidgetSizing::default().min_width(98))
			.on_update(update_number(grid, |grid| Some(&mut grid.origin.y)))
			.widget_instance(),
	]));

	match grid.grid_type {
		GridType::Rectangular { spacing } => {
			widgets.push(LayoutGroup::row(vec![
				label("Spacing"),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				NumberInput::new(Some(spacing.x))
					.label("X")
					.unit(" px")
					.min(0.01)
					.sizing(WidgetSizing::default().min_width(98))
					.on_update(update_number(grid, |grid| grid.grid_type.rectangular_spacing().map(|spacing| &mut spacing.x)))
					.widget_instance(),
				Separator::new(SeparatorStyle::Related).widget_instance(),
				NumberInput::new(Some(spacing.y))
					.label("Y")
					.unit(" px")
					.min(0.01)
					.sizing(WidgetSizing::default().min_width(98))
					.on_update(update_number(grid, |grid| grid.grid_type.rectangular_spacing().map(|spacing| &mut spacing.y)))
					.widget_instance(),
			]));
			let subdivisions_description = "Fainter minor lines divide each space between major lines into this many parts.";
			widgets.push(LayoutGroup::row(vec![
				TextLabel::new("Subdiv.")
					.sizing(WidgetSizing::fixed(LABEL_WIDTH))
					.table_align(true)
					.tooltip_label("Subdivisions")
					.tooltip_description(subdivisions_description)
					.widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				NumberInput::new(Some(grid.subdivisions as f64))
					.int()
					.min(1.)
					.max(10.)
					.sizing(WidgetSizing::default().min_width(200))
					.tooltip_label("Subdivisions")
					.tooltip_description(subdivisions_description)
					.on_update(update_val::<NumberInput, _>(grid, |grid, input| {
						if let Some(value) = input.value {
							grid.subdivisions = (value.round() as u32).max(1);
						}
					}))
					.widget_instance(),
			]));
			widgets.push(LayoutGroup::row(vec![
				label("Angle"),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				NumberInput::new(Some(grid.angle))
					.unit("°")
					.mode(NumberInputMode::Range)
					.range_min(Some(-180.))
					.range_max(Some(180.))
					.sizing(WidgetSizing::default().min_width(200))
					.on_update(update_number(grid, |grid| Some(&mut grid.angle)))
					.widget_instance(),
			]));
		}
		GridType::Isometric { y_axis_spacing, angle_a, angle_b } => {
			widgets.push(LayoutGroup::row(vec![
				label("Y Spacing"),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				NumberInput::new(Some(y_axis_spacing))
					.unit(" px")
					.min(0.01)
					.sizing(WidgetSizing::default().min_width(200))
					.on_update(update_number(grid, |grid| grid.grid_type.isometric_y_spacing()))
					.widget_instance(),
			]));
			widgets.push(LayoutGroup::row(vec![
				label("Angles"),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				NumberInput::new(Some(angle_a))
					.unit("°")
					.min(1.)
					.max(89.)
					.sizing(WidgetSizing::default().min_width(98))
					.on_update(update_number(grid, |grid| grid.grid_type.angle_a()))
					.widget_instance(),
				Separator::new(SeparatorStyle::Related).widget_instance(),
				NumberInput::new(Some(angle_b))
					.unit("°")
					.min(1.)
					.max(89.)
					.sizing(WidgetSizing::default().min_width(98))
					.on_update(update_number(grid, |grid| grid.grid_type.angle_b()))
					.widget_instance(),
			]));
		}
	}

	widgets
}
