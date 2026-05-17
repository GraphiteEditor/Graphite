use super::tool_prelude::*;
use crate::consts::{BOUNDS_SELECT_THRESHOLD, SNAP_POINT_TOLERANCE};
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{
	DrawingToolState, apply_fill_color_pick, apply_fill_enabled, apply_stroke_color_pick, apply_stroke_enabled, apply_working_colors, has_selection, reset_colors_on_deactivation,
	swap_fill_and_stroke, sync_color_options, sync_drawing_state,
};
use crate::messages::tool::common_functionality::gizmos::gizmo_manager::GizmoManager;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::resize::Resize;
use crate::messages::tool::common_functionality::shapes::arc_shape::Arc;
use crate::messages::tool::common_functionality::shapes::arrow_shape::Arrow;
use crate::messages::tool::common_functionality::shapes::circle_shape::Circle;
use crate::messages::tool::common_functionality::shapes::grid_shape::Grid;
use crate::messages::tool::common_functionality::shapes::line_shape::LineToolData;
use crate::messages::tool::common_functionality::shapes::polygon_shape::Polygon;
use crate::messages::tool::common_functionality::shapes::shape_utility::{ShapeToolModifierKey, ShapeType, anchor_overlays, clicked_on_shape_endpoints, transform_cage_overlays};
use crate::messages::tool::common_functionality::shapes::spiral_shape::Spiral;
use crate::messages::tool::common_functionality::shapes::star_shape::Star;
use crate::messages::tool::common_functionality::shapes::{Ellipse, Line, Rectangle};
use crate::messages::tool::common_functionality::snapping::{self, SnapCandidatePoint, SnapData, SnapTypeConfiguration};
use crate::messages::tool::common_functionality::stroke_options::{StrokeOptionsUpdate, apply_stroke_option, create_stroke_options_popover_widget};
use crate::messages::tool::common_functionality::transformation_cage::{BoundingBoxManager, EdgeBool};
use crate::messages::tool::common_functionality::utility_functions::{closest_point, resize_bounds, rotate_bounds, skew_bounds, transforming_transform_cage};
use crate::messages::tool::utility_types::DocumentToolData;
use graph_craft::document::NodeId;
use graph_craft::document::value::TaggedValue;
use graphene_std::renderer::Quad;
use graphene_std::vector::misc::{ArcType, GridType, SpiralType};
use graphene_std::vector::style::FillChoice;
use graphene_std::{Color, NodeInputDecleration};
use std::vec;

#[derive(Default, ExtractField)]
pub struct ShapeTool {
	fsm_state: ShapeToolFsmState,
	tool_data: ShapeToolData,
	options: ShapeToolOptions,
}

pub struct ShapeToolOptions {
	drawing: DrawingToolState,
	/// Per-shape-mode default for whether the fill checkbox is ticked when no layer is selected. Initialized from
	/// [`ShapeType::defaults_to_fill`] and updated when the user toggles the fill checkbox while nothing is selected,
	/// so the preference for each mode persists across mode switches and selection changes.
	shape_fill_defaults: std::collections::HashMap<ShapeType, bool>,
	/// Per-shape-mode default for whether the stroke checkbox is ticked when no layer is selected.
	/// Initialized to `true` for every mode and updated when the user toggles the stroke checkbox while nothing is selected.
	shape_stroke_defaults: std::collections::HashMap<ShapeType, bool>,
	/// Per-shape-mode value of the fill/stroke swap flag (mirrors `drawing.colors_swapped` for the current shape mode).
	/// Updated whenever the user toggles swap; read back when changing shape modes so each alias remembers its own routing.
	shape_colors_swapped: std::collections::HashMap<ShapeType, bool>,
	vertices: u32,
	shape_type: ShapeType,
	arc_type: ArcType,
	grid_type: GridType,
	spiral_type: SpiralType,
	turns: f64,
	arrow_shaft_width: f64,
	arrow_head_width: f64,
	arrow_head_length: f64,
}

impl Default for ShapeToolOptions {
	fn default() -> Self {
		let shape_fill_defaults = ShapeType::ALL.iter().map(|&shape| (shape, shape.defaults_to_fill())).collect();
		let shape_stroke_defaults = ShapeType::ALL.iter().map(|&shape| (shape, true)).collect();
		let shape_colors_swapped = ShapeType::ALL.iter().map(|&shape| (shape, false)).collect();

		Self {
			drawing: DrawingToolState::new(true),
			shape_fill_defaults,
			shape_stroke_defaults,
			shape_colors_swapped,
			vertices: 5,
			shape_type: ShapeType::Polygon,
			arc_type: ArcType::Open,
			spiral_type: SpiralType::Archimedean,
			turns: 5.,
			grid_type: GridType::Rectangular,
			arrow_shaft_width: 14.,
			arrow_head_width: 32.,
			arrow_head_length: 28.,
		}
	}
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ShapeOptionsUpdate {
	FillColor(FillChoice),
	FillEnabled(bool),
	StrokeOption(StrokeOptionsUpdate),
	StrokeColor(Option<Color>),
	StrokeEnabled(bool),
	SwapFillAndStroke,
	WorkingColorsChanged,
	Vertices(u32),
	ShapeType(ShapeType),
	ArcType(ArcType),
	SpiralType(SpiralType),
	Turns(f64),
	GridType(GridType),
	ArrowShaftWidth(f64),
	ArrowHeadWidth(f64),
	ArrowHeadLength(f64),
}

#[impl_message(Message, ToolMessage, Shape)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ShapeToolMessage {
	// Standard messages
	Overlays { context: OverlayContext },
	Abort,
	SelectionChanged,
	WorkingColorChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	HideShapeTypeWidget { hide: bool },
	PointerMove { modifier: ShapeToolModifierKey },
	PointerOutsideViewport { modifier: ShapeToolModifierKey },
	UpdateOptions { options: ShapeOptionsUpdate },
	SetShape { shape: ShapeType },
	SyncShapeWithOptions,

	IncreaseSides,
	DecreaseSides,

	NudgeSelectedLayers { delta_x: f64, delta_y: f64, resize: Key, resize_opposite_corner: Key },
}

fn create_sides_widget(vertices: u32) -> WidgetInstance {
	NumberInput::new(Some(vertices as f64))
		.label("Sides")
		.int()
		.min(3.)
		.max(1000.)
		.mode(NumberInputMode::Increment)
		.on_update(|number_input: &NumberInput| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::Vertices(number_input.value.unwrap() as u32),
			}
			.into()
		})
		.on_commit(|_| DocumentMessage::StartTransaction.into())
		.widget_instance()
}

fn create_turns_widget(turns: f64) -> WidgetInstance {
	NumberInput::new(Some(turns))
		.label("Turns")
		.min(0.5)
		.mode(NumberInputMode::Increment)
		.on_update(|number_input: &NumberInput| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::Turns(number_input.value.unwrap()),
			}
			.into()
		})
		.on_commit(|_| DocumentMessage::StartTransaction.into())
		.widget_instance()
}

fn create_shape_option_widget(shape_type: ShapeType) -> WidgetInstance {
	let entries = vec![vec![
		MenuListEntry::new("Polygon").label("Polygon").on_commit(move |_| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::ShapeType(ShapeType::Polygon),
			}
			.into()
		}),
		MenuListEntry::new("Star").label("Star").on_commit(move |_| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::ShapeType(ShapeType::Star),
			}
			.into()
		}),
		MenuListEntry::new("Circle").label("Circle").on_commit(move |_| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::ShapeType(ShapeType::Circle),
			}
			.into()
		}),
		MenuListEntry::new("Arc").label("Arc").on_commit(move |_| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::ShapeType(ShapeType::Arc),
			}
			.into()
		}),
		MenuListEntry::new("Spiral").label("Spiral").on_commit(move |_| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::ShapeType(ShapeType::Spiral),
			}
			.into()
		}),
		MenuListEntry::new("Grid").label("Grid").on_commit(move |_| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::ShapeType(ShapeType::Grid),
			}
			.into()
		}),
		MenuListEntry::new("Arrow").label("Arrow").on_commit(move |_| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::ShapeType(ShapeType::Arrow),
			}
			.into()
		}),
	]];
	DropdownInput::new(entries).selected_index(Some(shape_type as u32)).widget_instance()
}

fn create_arc_type_widget(arc_type: ArcType) -> WidgetInstance {
	let entries = vec![
		RadioEntryData::new("Open").label("Open").on_update(move |_| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::ArcType(ArcType::Open),
			}
			.into()
		}),
		RadioEntryData::new("Closed").label("Closed").on_update(move |_| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::ArcType(ArcType::Closed),
			}
			.into()
		}),
		RadioEntryData::new("Pie").label("Pie").on_update(move |_| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::ArcType(ArcType::PieSlice),
			}
			.into()
		}),
	];
	RadioInput::new(entries).selected_index(Some(arc_type as u32)).widget_instance()
}

fn create_arrow_shaft_width_widget(shaft_width: f64) -> WidgetInstance {
	NumberInput::new(Some(shaft_width))
		.unit(" px")
		.label("Shaft")
		.min(0.1)
		.max(1000.)
		.on_update(|number_input: &NumberInput| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::ArrowShaftWidth(number_input.value.unwrap()),
			}
			.into()
		})
		.on_commit(|_| DocumentMessage::StartTransaction.into())
		.widget_instance()
}

fn create_arrow_head_width_widget(head_width: f64) -> WidgetInstance {
	NumberInput::new(Some(head_width))
		.unit(" px")
		.label("Head W")
		.min(0.1)
		.max(1000.)
		.on_update(|number_input: &NumberInput| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::ArrowHeadWidth(number_input.value.unwrap()),
			}
			.into()
		})
		.on_commit(|_| DocumentMessage::StartTransaction.into())
		.widget_instance()
}

fn create_arrow_head_length_widget(head_length: f64) -> WidgetInstance {
	NumberInput::new(Some(head_length))
		.unit(" px")
		.label("Head L")
		.min(0.1)
		.max(1000.)
		.on_update(|number_input: &NumberInput| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::ArrowHeadLength(number_input.value.unwrap()),
			}
			.into()
		})
		.on_commit(|_| DocumentMessage::StartTransaction.into())
		.widget_instance()
}

fn create_spiral_type_widget(spiral_type: SpiralType) -> WidgetInstance {
	let entries = vec![vec![
		MenuListEntry::new("Archimedean").label("Archimedean").on_commit(move |_| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::SpiralType(SpiralType::Archimedean),
			}
			.into()
		}),
		MenuListEntry::new("Logarithmic").label("Logarithmic").on_commit(move |_| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::SpiralType(SpiralType::Logarithmic),
			}
			.into()
		}),
	]];
	DropdownInput::new(entries).selected_index(Some(spiral_type as u32)).widget_instance()
}

fn create_grid_type_widget(grid_type: GridType) -> WidgetInstance {
	let entries = vec![
		RadioEntryData::new("Rectangular").label("Rectangular").on_update(move |_| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::GridType(GridType::Rectangular),
			}
			.into()
		}),
		RadioEntryData::new("Isometric").label("Isometric").on_update(move |_| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::GridType(GridType::Isometric),
			}
			.into()
		}),
	];
	RadioInput::new(entries).selected_index(Some(grid_type as u32)).widget_instance()
}

/// Mirrors the per-shape parameters (and `shape_type` itself) from the first selected non-artboard layer into the
/// control bar's option state. Detects the layer's shape by trying each generator's proto node, then reads only the
/// inputs relevant to that shape. Returns whether anything in `options` (or `tool_data.current_shape`) changed.
/// The caller decides whether to dispatch a layout refresh.
fn sync_shape_options_from_selection(options: &mut ShapeToolOptions, tool_data: &mut ShapeToolData, document: &DocumentMessageHandler) -> bool {
	use graphene_std::vector::generator_nodes::*;

	let Some(layer) = document.network_interface.selected_nodes().selected_layers_except_artboards(&document.network_interface).next() else {
		return false;
	};
	let layer_view = graph_modification_utils::NodeGraphLayer::new(layer, &document.network_interface);
	let proto = DefinitionIdentifier::ProtoNode;

	// Map each generator's proto node to the corresponding `ShapeType`.
	// First match wins. Only includes modes from the Shape tool's mode dropdown.
	let Some(shape_type) = [
		(regular_polygon::IDENTIFIER, ShapeType::Polygon),
		(star::IDENTIFIER, ShapeType::Star),
		(circle::IDENTIFIER, ShapeType::Circle),
		(arc::IDENTIFIER, ShapeType::Arc),
		(spiral::IDENTIFIER, ShapeType::Spiral),
		(grid::IDENTIFIER, ShapeType::Grid),
		(arrow::IDENTIFIER, ShapeType::Arrow),
	]
	.into_iter()
	.find_map(|(id, shape)| layer_view.upstream_node_id_from_name(&proto(id)).map(|_| shape)) else {
		return false;
	};

	let mut changed = false;

	if options.shape_type != shape_type {
		options.shape_type = shape_type;
		tool_data.current_shape = shape_type;
		changed = true;
	}

	// Only the shapes whose control bar exposes per-shape parameters need a sync below.
	// The rest (Ellipse, Rectangle, Line) just keep `shape_type` in step and rely on the shared Stroke/Fill controls.
	match shape_type {
		ShapeType::Polygon | ShapeType::Star => {
			let id = if shape_type == ShapeType::Polygon { regular_polygon::IDENTIFIER } else { star::IDENTIFIER };
			// Both `regular_polygon` and `star` are generic over `T: AsU64`, but the control bar widget always writes `u32`,
			// and existing call sites (e.g. `polygon_shape.rs`) read it back as `TaggedValue::U32`.
			let index = if shape_type == ShapeType::Polygon {
				regular_polygon::SidesInput::<u32>::INDEX
			} else {
				star::SidesInput::<u32>::INDEX
			};
			if let Some(&TaggedValue::U32(sides)) = layer_view.find_input(&proto(id), index)
				&& options.vertices != sides
			{
				options.vertices = sides;
				changed = true;
			}
		}
		ShapeType::Arc => {
			if let Some(&TaggedValue::ArcType(arc_type)) = layer_view.find_input(&proto(arc::IDENTIFIER), arc::ArcTypeInput::INDEX)
				&& options.arc_type != arc_type
			{
				options.arc_type = arc_type;
				changed = true;
			}
		}
		ShapeType::Spiral => {
			if let Some(&TaggedValue::SpiralType(spiral_type)) = layer_view.find_input(&proto(spiral::IDENTIFIER), spiral::SpiralTypeInput::INDEX)
				&& options.spiral_type != spiral_type
			{
				options.spiral_type = spiral_type;
				changed = true;
			}
			if let Some(&TaggedValue::F64(turns)) = layer_view.find_input(&proto(spiral::IDENTIFIER), spiral::TurnsInput::INDEX)
				&& options.turns != turns
			{
				options.turns = turns;
				changed = true;
			}
		}
		ShapeType::Grid => {
			if let Some(&TaggedValue::GridType(grid_type)) = layer_view.find_input(&proto(grid::IDENTIFIER), grid::GridTypeInput::INDEX)
				&& options.grid_type != grid_type
			{
				options.grid_type = grid_type;
				changed = true;
			}
		}
		ShapeType::Arrow => {
			if let Some(&TaggedValue::F64(shaft)) = layer_view.find_input(&proto(arrow::IDENTIFIER), arrow::ShaftWidthInput::INDEX)
				&& options.arrow_shaft_width != shaft
			{
				options.arrow_shaft_width = shaft;
				changed = true;
			}
			if let Some(&TaggedValue::F64(head_w)) = layer_view.find_input(&proto(arrow::IDENTIFIER), arrow::HeadWidthInput::INDEX)
				&& options.arrow_head_width != head_w
			{
				options.arrow_head_width = head_w;
				changed = true;
			}
			if let Some(&TaggedValue::F64(head_l)) = layer_view.find_input(&proto(arrow::IDENTIFIER), arrow::HeadLengthInput::INDEX)
				&& options.arrow_head_length != head_l
			{
				options.arrow_head_length = head_l;
				changed = true;
			}
		}
		ShapeType::Ellipse | ShapeType::Rectangle | ShapeType::Line | ShapeType::Circle => {}
	}

	changed
}

/// Shared logic for handling a shape-mode change from either the `SetShape` alias (FSM-driven) or the `ShapeType` dropdown.
/// Loads the new mode's persistent swap flag, resets the displayed fill/stroke colors back to the (now routed) working colors,
/// and re-syncs from the selection using the new mode's natural fill/stroke defaults. The caller is responsible for updating
/// `options.shape_type` and `tool_data.current_shape` beforehand if needed.
fn handle_shape_mode_change(options: &mut ShapeToolOptions, new_shape: ShapeType, prev_shape: ShapeType, global: &DocumentToolData, document: &DocumentMessageHandler) {
	if new_shape != prev_shape {
		options.drawing.colors_swapped = *options.shape_colors_swapped.get(&new_shape).unwrap_or(&false);
		reset_colors_on_deactivation(&mut options.drawing, global);
	}
	let natural_fill_enabled = *options.shape_fill_defaults.get(&new_shape).unwrap_or(&new_shape.defaults_to_fill());
	let natural_stroke_enabled = *options.shape_stroke_defaults.get(&new_shape).unwrap_or(&true);
	// Treat the shape change as a real selection change so the new mode's natural defaults apply when nothing matches on the selection.
	sync_color_options(&mut options.drawing, natural_fill_enabled, natural_stroke_enabled, global, document, true);
}

impl LayoutHolder for ShapeTool {
	fn layout(&self) -> Layout {
		let mut widgets = vec![];

		// Fill / Stroke / Weight (Shared across all shape modes. Line shows no Fill)
		if self.options.shape_type != ShapeType::Line {
			widgets.append(&mut self.options.drawing.fill.create_widgets(
				"Fill:",
				|checkbox: &CheckboxInput| {
					ShapeToolMessage::UpdateOptions {
						options: ShapeOptionsUpdate::FillEnabled(checkbox.checked),
					}
					.into()
				},
				|color: &ColorInput| {
					ShapeToolMessage::UpdateOptions {
						options: ShapeOptionsUpdate::FillColor(FillChoice::from(&color.value)),
					}
					.into()
				},
			));

			widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
			widgets.push(
				IconButton::new("SwapHorizontal", 16)
					.tooltip_label("Swap Fill/Stroke Colors")
					.on_update(|_| {
						ShapeToolMessage::UpdateOptions {
							options: ShapeOptionsUpdate::SwapFillAndStroke,
						}
						.into()
					})
					.widget_instance(),
			);
			widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
		}

		widgets.append(&mut self.options.drawing.stroke.create_widgets(
			"Stroke:",
			|checkbox: &CheckboxInput| {
				ShapeToolMessage::UpdateOptions {
					options: ShapeOptionsUpdate::StrokeEnabled(checkbox.checked),
				}
				.into()
			},
			|color: &ColorInput| {
				ShapeToolMessage::UpdateOptions {
					options: ShapeOptionsUpdate::StrokeColor(color.value.as_solid().map(Color::from)),
				}
				.into()
			},
		));
		let weight_disabled = self.options.drawing.stroke.enabled == Some(false);
		widgets.push(create_stroke_options_popover_widget(&self.options.drawing, weight_disabled, |update| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::StrokeOption(update),
			}
			.into()
		}));

		// Shape-mode dropdown and per-shape parameters
		if !self.tool_data.hide_shape_option_widget {
			widgets.push(Separator::new(SeparatorStyle::Section).widget_instance());
			widgets.push(create_shape_option_widget(self.options.shape_type));

			if self.options.shape_type == ShapeType::Polygon || self.options.shape_type == ShapeType::Star {
				widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
				widgets.push(create_sides_widget(self.options.vertices));
			}

			if self.options.shape_type == ShapeType::Arc {
				widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
				widgets.push(create_arc_type_widget(self.options.arc_type));
			}

			if self.options.shape_type == ShapeType::Spiral {
				widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
				widgets.push(create_spiral_type_widget(self.options.spiral_type));
				widgets.push(Separator::new(SeparatorStyle::Related).widget_instance());
				widgets.push(create_turns_widget(self.options.turns));
			}

			if self.options.shape_type == ShapeType::Grid {
				widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
				widgets.push(create_grid_type_widget(self.options.grid_type));
			}

			if self.options.shape_type == ShapeType::Arrow {
				widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
				widgets.push(create_arrow_shaft_width_widget(self.options.arrow_shaft_width));
				widgets.push(Separator::new(SeparatorStyle::Related).widget_instance());
				widgets.push(create_arrow_head_width_widget(self.options.arrow_head_width));
				widgets.push(Separator::new(SeparatorStyle::Related).widget_instance());
				widgets.push(create_arrow_head_length_widget(self.options.arrow_head_length));
			}
		}

		Layout(vec![LayoutGroup::row(widgets)])
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for ShapeTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		use graphene_std::vector::generator_nodes::*;

		// On tool deactivation (Abort fires from the dispatcher's tool transition), reset the displayed fill/stroke colors so
		// the next activation starts fresh from the current working colors. The global swap state persists across tool switches.
		// Guarded on `Ready(_)` so Esc-mid-drawing (which also fires Abort) doesn't wipe the user's customized fill/stroke options.
		if matches!(&message, ToolMessage::Shape(ShapeToolMessage::Abort)) && matches!(self.fsm_state, ShapeToolFsmState::Ready(_)) {
			reset_colors_on_deactivation(&mut self.options.drawing, context.global_tool_data);
		}

		if matches!(&message, ToolMessage::Shape(ShapeToolMessage::SelectionChanged)) {
			if !matches!(self.fsm_state, ShapeToolFsmState::Ready(_)) {
				return;
			}

			// The natural fill/stroke defaults depend on the shape type (Spiral/Grid/Line have no fill by default).
			let current_shape = self.tool_data.current_shape;
			let natural_fill_enabled = *self.options.shape_fill_defaults.get(&current_shape).unwrap_or(&current_shape.defaults_to_fill());
			let natural_stroke_enabled = *self.options.shape_stroke_defaults.get(&current_shape).unwrap_or(&true);
			let mut needs_refresh = sync_drawing_state(&mut self.options.drawing, natural_fill_enabled, natural_stroke_enabled, context.global_tool_data, context.document);

			// Detect which shape the first selected layer is by checking for each generator's proto node, then mirror
			// the control bar's `shape_type` into that and pull the shape's parameters into the matching control bar fields.
			needs_refresh |= sync_shape_options_from_selection(&mut self.options, &mut self.tool_data, context.document);

			if needs_refresh {
				self.send_layout(responses, LayoutTarget::ToolOptions);
			}
			return;
		}

		// SetShape changes the active shape mode, which can change the natural fill default (e.g. Line/Spiral/Grid have no fill).
		// Trigger a re-sync afterward so the controls reflect either the current selection or the new natural default.
		// Note: the `UpdateOptions { ShapeType(_) }` variant matches the `let else` below and is handled by the `ShapeType` arm,
		// so it can't reach the `else` block where this flag is read — only `SetShape` (the FSM-routed alias) can.
		let is_set_shape = matches!(&message, ToolMessage::Shape(ShapeToolMessage::SetShape { .. }));
		let shape_before = self.tool_data.current_shape;

		let ToolMessage::Shape(ShapeToolMessage::UpdateOptions { options }) = message else {
			self.fsm_state.process_event(message, &mut self.tool_data, context, &self.options, responses, true);
			if is_set_shape {
				handle_shape_mode_change(&mut self.options, self.tool_data.current_shape, shape_before, context.global_tool_data, context.document);
				self.send_layout(responses, LayoutTarget::ToolOptions);
			}
			return;
		};
		match options {
			ShapeOptionsUpdate::FillColor(fill_choice) => {
				apply_fill_color_pick(&mut self.options.drawing, fill_choice, context.document, responses);
			}
			ShapeOptionsUpdate::FillEnabled(enabled) => {
				// When toggled with no selection, persist the new state as the current shape mode's default
				if !has_selection(context.document) {
					self.options.shape_fill_defaults.insert(self.tool_data.current_shape, enabled);
				}
				apply_fill_enabled(&mut self.options.drawing, enabled, context.global_tool_data, context.document, responses);
			}
			ShapeOptionsUpdate::StrokeOption(update) => {
				apply_stroke_option(&mut self.options.drawing, update, context.document, responses);
			}
			ShapeOptionsUpdate::StrokeColor(color) => {
				apply_stroke_color_pick(&mut self.options.drawing, color, context.document, responses);
			}
			ShapeOptionsUpdate::StrokeEnabled(enabled) => {
				// When toggled with no selection, persist the new state as the current shape mode's default
				if !has_selection(context.document) {
					self.options.shape_stroke_defaults.insert(self.tool_data.current_shape, enabled);
				}
				apply_stroke_enabled(&mut self.options.drawing, enabled, context.global_tool_data, context.document, responses);
			}
			ShapeOptionsUpdate::SwapFillAndStroke => {
				swap_fill_and_stroke(&mut self.options.drawing, context.document, responses);
				// Persist the new swap state as the current shape mode's default
				self.options.shape_colors_swapped.insert(self.tool_data.current_shape, self.options.drawing.colors_swapped);
			}
			ShapeOptionsUpdate::WorkingColorsChanged => {
				apply_working_colors(&mut self.options.drawing, context.global_tool_data, context.document);
			}
			ShapeOptionsUpdate::ShapeType(shape) => {
				self.options.shape_type = shape;
				self.tool_data.current_shape = shape;
				handle_shape_mode_change(&mut self.options, shape, shape_before, context.global_tool_data, context.document);
			}
			ShapeOptionsUpdate::Vertices(vertices) => {
				self.options.vertices = vertices;
				// Push to whichever sides-bearing shape (Polygon or Star) the control bar's `shape_type` currently targets.
				// `set_proto_node_input_for_selected_layers` skips selected layers without that proto node, making it a no-op.
				let (id, index) = match self.options.shape_type {
					ShapeType::Polygon => (regular_polygon::IDENTIFIER, regular_polygon::SidesInput::<u32>::INDEX),
					ShapeType::Star => (star::IDENTIFIER, star::SidesInput::<u32>::INDEX),
					_ => return,
				};
				graph_modification_utils::set_proto_node_input_for_selected_layers(context.document, id, index, TaggedValue::U32(vertices), responses);
			}
			ShapeOptionsUpdate::ArcType(arc_type) => {
				self.options.arc_type = arc_type;
				graph_modification_utils::set_proto_node_input_for_selected_layers(context.document, arc::IDENTIFIER, arc::ArcTypeInput::INDEX, TaggedValue::ArcType(arc_type), responses);
			}
			ShapeOptionsUpdate::SpiralType(spiral_type) => {
				self.options.spiral_type = spiral_type;
				graph_modification_utils::set_proto_node_input_for_selected_layers(
					context.document,
					spiral::IDENTIFIER,
					spiral::SpiralTypeInput::INDEX,
					TaggedValue::SpiralType(spiral_type),
					responses,
				);
			}
			ShapeOptionsUpdate::Turns(turns) => {
				self.options.turns = turns;
				graph_modification_utils::set_proto_node_input_for_selected_layers(context.document, spiral::IDENTIFIER, spiral::TurnsInput::INDEX, TaggedValue::F64(turns), responses);
			}
			ShapeOptionsUpdate::GridType(grid_type) => {
				self.options.grid_type = grid_type;
				graph_modification_utils::set_proto_node_input_for_selected_layers(context.document, grid::IDENTIFIER, grid::GridTypeInput::INDEX, TaggedValue::GridType(grid_type), responses);
			}
			ShapeOptionsUpdate::ArrowShaftWidth(shaft_width) => {
				self.options.arrow_shaft_width = shaft_width;
				graph_modification_utils::set_proto_node_input_for_selected_layers(context.document, arrow::IDENTIFIER, arrow::ShaftWidthInput::INDEX, TaggedValue::F64(shaft_width), responses);
			}
			ShapeOptionsUpdate::ArrowHeadWidth(head_width) => {
				self.options.arrow_head_width = head_width;
				graph_modification_utils::set_proto_node_input_for_selected_layers(context.document, arrow::IDENTIFIER, arrow::HeadWidthInput::INDEX, TaggedValue::F64(head_width), responses);
			}
			ShapeOptionsUpdate::ArrowHeadLength(head_length) => {
				self.options.arrow_head_length = head_length;
				graph_modification_utils::set_proto_node_input_for_selected_layers(context.document, arrow::IDENTIFIER, arrow::HeadLengthInput::INDEX, TaggedValue::F64(head_length), responses);
			}
		}

		update_dynamic_hints(&self.fsm_state, responses, &self.tool_data);
		self.send_layout(responses, LayoutTarget::ToolOptions);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			ShapeToolFsmState::Ready(_) => actions!(ShapeToolMessageDiscriminant;
				DragStart,
				PointerMove,
				SetShape,
				Abort,
				HideShapeTypeWidget,
				IncreaseSides,
				DecreaseSides,
				NudgeSelectedLayers,
			),
			ShapeToolFsmState::Drawing(_)
			| ShapeToolFsmState::ResizingBounds
			| ShapeToolFsmState::DraggingLineEndpoints
			| ShapeToolFsmState::RotatingBounds
			| ShapeToolFsmState::ModifyingGizmo
			| ShapeToolFsmState::SkewingBounds { .. } => {
				actions!(ShapeToolMessageDiscriminant;
					DragStop,
					Abort,
					PointerMove,
					SetShape,
					HideShapeTypeWidget,
					IncreaseSides,
					DecreaseSides,
					NudgeSelectedLayers,
				)
			}
		}
	}
}

impl ToolMetadata for ShapeTool {
	fn icon_name(&self) -> String {
		"VectorPolygonTool".into()
	}
	fn tooltip_label(&self) -> String {
		"Shape Tool".into()
	}
	fn tool_type(&self) -> ToolType {
		ToolType::Shape
	}
}

impl ToolTransition for ShapeTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			overlay_provider: Some(|context| ShapeToolMessage::Overlays { context }.into()),
			tool_abort: Some(ShapeToolMessage::Abort.into()),
			selection_changed: Some(ShapeToolMessage::SelectionChanged.into()),
			working_color_changed: Some(ShapeToolMessage::WorkingColorChanged.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeToolFsmState {
	Ready(ShapeType),
	Drawing(ShapeType),

	// Gizmos
	DraggingLineEndpoints,
	ModifyingGizmo,

	// Transform cage
	ResizingBounds,
	RotatingBounds,
	SkewingBounds { skew: Key },
}

impl Default for ShapeToolFsmState {
	fn default() -> Self {
		ShapeToolFsmState::Ready(ShapeType::default())
	}
}

#[derive(Clone, Debug, Default)]
pub struct ShapeToolData {
	pub data: Resize,
	auto_panning: AutoPanning,

	// In viewport space
	pub last_mouse_position: DVec2,

	// Hide the dropdown menu when using Line, Rectangle, or Ellipse aliases
	pub hide_shape_option_widget: bool,

	// Shape-specific data
	pub line_data: LineToolData,

	// Used for by transform cage
	pub bounding_box_manager: Option<BoundingBoxManager>,
	layers_dragging: Vec<LayerNodeIdentifier>,
	snap_candidates: Vec<SnapCandidatePoint>,
	skew_edge: EdgeBool,
	cursor: MouseCursorIcon,

	// Current shape which is being drawn
	current_shape: ShapeType,

	// Gizmos
	gizmo_manager: GizmoManager,
}

impl ShapeToolData {
	fn get_snap_candidates(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, viewport: &ViewportMessageHandler) {
		self.snap_candidates.clear();
		for &layer in &self.layers_dragging {
			if (self.snap_candidates.len() as f64) < document.snapping_state.tolerance {
				snapping::get_layer_snap_points(layer, &SnapData::new(document, input, viewport), &mut self.snap_candidates);
			}
			if let Some(bounds) = document.metadata().bounding_box_with_transform(layer, DAffine2::IDENTITY) {
				let quad = document.metadata().transform_to_document(layer) * Quad::from_box(bounds);
				snapping::get_bbox_points(quad, &mut self.snap_candidates, snapping::BBoxSnapValues::BOUNDING_BOX, document);
			}
		}
	}

	fn transform_cage_mouse_icon(&mut self, input: &InputPreprocessorMessageHandler) -> MouseCursorIcon {
		let dragging_bounds = self
			.bounding_box_manager
			.as_mut()
			.and_then(|bounding_box| bounding_box.check_selected_edges(input.pointer.position))
			.is_some();

		self.bounding_box_manager.as_ref().map_or(MouseCursorIcon::Crosshair, |bounds| {
			let cursor_icon = bounds.get_cursor(input, true, dragging_bounds, Some(self.skew_edge));
			if cursor_icon == MouseCursorIcon::Default { MouseCursorIcon::Crosshair } else { cursor_icon }
		})
	}

	fn shape_tool_modifier_keys() -> [Key; 3] {
		[Key::Alt, Key::Shift, Key::Control]
	}

	fn decrease_or_increase_sides(&self, document: &DocumentMessageHandler, shape_type: ShapeType, responses: &mut VecDeque<Message>, decrease: bool) {
		if let Some(layer) = self.data.layer {
			match shape_type {
				ShapeType::Star | ShapeType::Polygon => Polygon::decrease_or_increase_sides(decrease, layer, document, responses),
				ShapeType::Spiral => Spiral::update_turns(decrease, layer, document, responses),
				_ => {}
			}
		}

		responses.add(NodeGraphMessage::RunDocumentGraph);
	}
}

impl Fsm for ShapeToolFsmState {
	type ToolData = ShapeToolData;
	type ToolOptions = ShapeToolOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		ToolActionMessageContext {
			document,
			input,
			shape_editor,
			viewport,
			..
		}: &mut ToolActionMessageContext,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let all_selected_layers_line_or_arrow = document
			.network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(&document.network_interface)
			.all(|layer| graph_modification_utils::get_line_id(layer, &document.network_interface).is_some() || graph_modification_utils::get_arrow_id(layer, &document.network_interface).is_some());

		let ToolMessage::Shape(event) = event else { return self };

		match (self, event) {
			(_, ShapeToolMessage::Overlays { context: mut overlay_context }) => {
				let mouse_position = tool_data
					.data
					.snap_manager
					.indicator_pos()
					.map(|pos| document.metadata().document_to_viewport.transform_point2(pos))
					.unwrap_or(input.pointer.position);

				if matches!(self, Self::Ready(_)) && !input.keyboard.key(Key::Control) {
					tool_data.gizmo_manager.handle_actions(mouse_position, document, responses);
					tool_data.gizmo_manager.overlays(document, input, shape_editor, mouse_position, &mut overlay_context);
				}

				if matches!(self, ShapeToolFsmState::ModifyingGizmo) && !input.keyboard.key(Key::Control) {
					tool_data.gizmo_manager.dragging_overlays(document, input, shape_editor, mouse_position, &mut overlay_context);
					let cursor = tool_data.gizmo_manager.mouse_cursor_icon().unwrap_or(MouseCursorIcon::Crosshair);
					tool_data.cursor = cursor;
					responses.add(FrontendMessage::UpdateMouseCursor { cursor });
				}

				let modifying_transform_cage = matches!(self, ShapeToolFsmState::ResizingBounds | ShapeToolFsmState::RotatingBounds | ShapeToolFsmState::SkewingBounds { .. });
				let hovering_over_gizmo = tool_data.gizmo_manager.hovering_over_gizmo();

				// Check if hovering over a line/arrow endpoint (using data from previous overlay pass)
				let hovering_over_endpoint = tool_data.line_data.selected_layers_with_position.iter().any(|(layer, endpoints)| {
					let transform = document.metadata().transform_to_viewport(*layer);
					endpoints
						.iter()
						.any(|&local_pos| (transform.transform_point2(local_pos) - input.pointer.position).length_squared() < BOUNDS_SELECT_THRESHOLD.powi(2))
				});

				if !matches!(self, ShapeToolFsmState::ModifyingGizmo) && !modifying_transform_cage && !hovering_over_gizmo && !hovering_over_endpoint {
					tool_data.data.snap_manager.draw_overlays(SnapData::new(document, input, viewport), &mut overlay_context);
				}

				if modifying_transform_cage && !matches!(self, ShapeToolFsmState::ModifyingGizmo) {
					transform_cage_overlays(document, tool_data, &mut overlay_context);
					responses.add(FrontendMessage::UpdateMouseCursor { cursor: tool_data.cursor });
				}

				if input.keyboard.key(Key::Control) && matches!(self, ShapeToolFsmState::Ready(_)) {
					anchor_overlays(document, &mut overlay_context);
					responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
				} else if matches!(self, ShapeToolFsmState::Ready(_)) {
					Line::overlays(document, tool_data, input.pointer.position, &mut overlay_context);
					Arrow::overlays(document, tool_data, input.pointer.position, &mut overlay_context);

					if all_selected_layers_line_or_arrow {
						let cursor = if hovering_over_endpoint { MouseCursorIcon::Default } else { MouseCursorIcon::Crosshair };
						tool_data.cursor = cursor;
						responses.add(FrontendMessage::UpdateMouseCursor { cursor });
						return self;
					}

					if !hovering_over_gizmo {
						transform_cage_overlays(document, tool_data, &mut overlay_context);
					}

					let dragging_bounds = tool_data
						.bounding_box_manager
						.as_mut()
						.and_then(|bounding_box| bounding_box.check_selected_edges(input.pointer.position))
						.is_some();

					if let Some(bounds) = tool_data.bounding_box_manager.as_mut() {
						let edges = bounds.check_selected_edges(input.pointer.position);
						let is_skewing = matches!(self, ShapeToolFsmState::SkewingBounds { .. });
						let is_near_square = edges.is_some_and(|hover_edge| bounds.over_extended_edge_midpoint(input.pointer.position, hover_edge));
						if is_skewing || (dragging_bounds && is_near_square && !hovering_over_gizmo) {
							bounds.render_skew_gizmos(&mut overlay_context, tool_data.skew_edge);
						}
						if dragging_bounds
							&& !is_skewing && !hovering_over_gizmo
							&& let Some(edges) = edges
						{
							tool_data.skew_edge = bounds.get_closest_edge(edges, input.pointer.position);
						}
					}

					let cursor = tool_data
						.gizmo_manager
						.mouse_cursor_icon()
						.or_else(|| hovering_over_endpoint.then_some(MouseCursorIcon::Default))
						.unwrap_or_else(|| tool_data.transform_cage_mouse_icon(input));

					tool_data.cursor = cursor;
					responses.add(FrontendMessage::UpdateMouseCursor { cursor });
				}

				if matches!(self, ShapeToolFsmState::Drawing(_) | ShapeToolFsmState::DraggingLineEndpoints) {
					Line::overlays(document, tool_data, input.pointer.position, &mut overlay_context);
					Arrow::overlays(document, tool_data, input.pointer.position, &mut overlay_context);

					if tool_options.shape_type == ShapeType::Circle {
						tool_data.gizmo_manager.overlays(document, input, shape_editor, mouse_position, &mut overlay_context);
					}
				}

				self
			}
			(ShapeToolFsmState::Ready(_), ShapeToolMessage::IncreaseSides) => {
				if matches!(tool_options.shape_type, ShapeType::Star | ShapeType::Polygon) {
					responses.add(ShapeToolMessage::UpdateOptions {
						options: ShapeOptionsUpdate::Vertices(tool_options.vertices + 1),
					});
				}

				if matches!(tool_options.shape_type, ShapeType::Spiral) {
					responses.add(ShapeToolMessage::UpdateOptions {
						options: ShapeOptionsUpdate::Turns(tool_options.turns + 1.),
					});
				}

				self
			}
			(ShapeToolFsmState::Ready(_), ShapeToolMessage::DecreaseSides) => {
				if matches!(tool_options.shape_type, ShapeType::Star | ShapeType::Polygon) {
					responses.add(ShapeToolMessage::UpdateOptions {
						options: ShapeOptionsUpdate::Vertices((tool_options.vertices - 1).max(3)),
					});
				}

				if matches!(tool_options.shape_type, ShapeType::Spiral) {
					responses.add(ShapeToolMessage::UpdateOptions {
						options: ShapeOptionsUpdate::Turns((tool_options.turns - 1.).max(1.)),
					});
				}
				self
			}
			(
				ShapeToolFsmState::Ready(_),
				ShapeToolMessage::NudgeSelectedLayers {
					delta_x,
					delta_y,
					resize,
					resize_opposite_corner,
				},
			) => {
				responses.add(DocumentMessage::NudgeSelectedLayers {
					delta_x,
					delta_y,
					resize,
					resize_opposite_corner,
				});

				self
			}
			(ShapeToolFsmState::Drawing(_), ShapeToolMessage::NudgeSelectedLayers { .. }) => {
				let increase = input.keyboard.key(Key::ArrowUp);
				let decrease = input.keyboard.key(Key::ArrowDown);

				if increase {
					responses.add(ShapeToolMessage::IncreaseSides);
					return self;
				}

				if decrease {
					responses.add(ShapeToolMessage::DecreaseSides);
					return self;
				}
				self
			}
			(ShapeToolFsmState::Drawing(_), ShapeToolMessage::IncreaseSides) => {
				tool_data.decrease_or_increase_sides(document, tool_options.shape_type, responses, false);
				self
			}
			(ShapeToolFsmState::Drawing(_), ShapeToolMessage::DecreaseSides) => {
				tool_data.decrease_or_increase_sides(document, tool_options.shape_type, responses, true);
				self
			}
			(ShapeToolFsmState::Ready(_), ShapeToolMessage::DragStart) => {
				tool_data.line_data.drag_start = input.pointer.position;

				// Snapped position in viewport space
				let mouse_pos = tool_data
					.data
					.snap_manager
					.indicator_pos()
					.map(|pos| document.metadata().document_to_viewport.transform_point2(pos))
					.unwrap_or(input.pointer.position);

				tool_data.line_data.drag_current = mouse_pos;

				if tool_data.gizmo_manager.handle_click() && !input.keyboard.key(Key::Accel) {
					tool_data.data.drag_start = document.metadata().document_to_viewport.inverse().transform_point2(mouse_pos);
					responses.add(DocumentMessage::StartTransaction);

					let cursor = tool_data.gizmo_manager.mouse_cursor_icon().unwrap_or(MouseCursorIcon::Crosshair);
					tool_data.cursor = cursor;
					responses.add(FrontendMessage::UpdateMouseCursor { cursor });
					// Send a PointerMove message to refresh the cursor icon
					responses.add(ShapeToolMessage::PointerMove {
						modifier: ShapeToolData::shape_tool_modifier_keys(),
					});

					responses.add(DocumentMessage::StartTransaction);

					return ShapeToolFsmState::ModifyingGizmo;
				}

				// If clicked on endpoints of a selected line or arrow, drag its endpoints
				if let Some((layer, _, _)) = closest_point(
					document,
					mouse_pos,
					SNAP_POINT_TOLERANCE,
					document.network_interface.selected_nodes().selected_visible_and_unlocked_layers(&document.network_interface),
					|_| false,
				) && clicked_on_shape_endpoints(layer, document, input, tool_data)
					&& !input.keyboard.key(Key::Control)
				{
					responses.add(DocumentMessage::StartTransaction);
					return ShapeToolFsmState::DraggingLineEndpoints;
				}

				let (resize, rotate, skew) = transforming_transform_cage(document, &mut tool_data.bounding_box_manager, input, responses, &mut tool_data.layers_dragging, None);

				if !input.keyboard.key(Key::Control) {
					// Helper function to update cursor and send pointer move message
					let update_cursor_and_pointer = |tool_data: &mut ShapeToolData, responses: &mut VecDeque<Message>| {
						let cursor = tool_data.transform_cage_mouse_icon(input);
						tool_data.cursor = cursor;
						responses.add(FrontendMessage::UpdateMouseCursor { cursor });
						responses.add(ShapeToolMessage::PointerMove {
							modifier: ShapeToolData::shape_tool_modifier_keys(),
						});
					};

					match (resize, rotate, skew) {
						(true, false, false) => {
							tool_data.get_snap_candidates(document, input, viewport);
							update_cursor_and_pointer(tool_data, responses);

							return ShapeToolFsmState::ResizingBounds;
						}
						(false, true, false) => {
							tool_data.data.drag_start = mouse_pos;
							update_cursor_and_pointer(tool_data, responses);

							return ShapeToolFsmState::RotatingBounds;
						}
						(false, false, true) => {
							tool_data.get_snap_candidates(document, input, viewport);
							update_cursor_and_pointer(tool_data, responses);

							return ShapeToolFsmState::SkewingBounds { skew: Key::Control };
						}
						_ => {}
					}
				};

				match tool_data.current_shape {
					ShapeType::Polygon | ShapeType::Star | ShapeType::Circle | ShapeType::Arc | ShapeType::Spiral | ShapeType::Grid | ShapeType::Rectangle | ShapeType::Ellipse => {
						tool_data.data.start(document, input, viewport);
					}
					ShapeType::Arrow | ShapeType::Line => {
						let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.pointer.position));
						let snapped = tool_data
							.data
							.snap_manager
							.free_snap(&SnapData::new(document, input, viewport), &point, SnapTypeConfiguration::default());
						tool_data.data.drag_start = snapped.snapped_point_document;
					}
				}

				responses.add(DocumentMessage::StartTransaction);

				let node = match tool_data.current_shape {
					ShapeType::Polygon => Polygon::create_node(tool_options.vertices),
					ShapeType::Star => Star::create_node(tool_options.vertices),
					ShapeType::Circle => Circle::create_node(),
					ShapeType::Arc => Arc::create_node(tool_options.arc_type),
					ShapeType::Spiral => Spiral::create_node(tool_options.spiral_type, tool_options.turns),
					ShapeType::Grid => Grid::create_node(tool_options.grid_type),
					ShapeType::Arrow => Arrow::create_node(tool_options.arrow_shaft_width, tool_options.arrow_head_width, tool_options.arrow_head_length),
					ShapeType::Line => Line::create_node(),
					ShapeType::Rectangle => Rectangle::create_node(),
					ShapeType::Ellipse => Ellipse::create_node(),
				};

				let nodes = vec![(NodeId(0), node)];
				let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, document.new_layer_bounding_artboard(input, viewport), responses);

				let defered_responses = &mut VecDeque::new();

				match tool_data.current_shape {
					ShapeType::Polygon | ShapeType::Star | ShapeType::Circle | ShapeType::Arc | ShapeType::Spiral | ShapeType::Grid | ShapeType::Rectangle | ShapeType::Ellipse => {
						defered_responses.add(GraphOperationMessage::TransformSet {
							layer,
							transform: DAffine2::from_scale_angle_translation(DVec2::ONE, 0., input.pointer.position),
							transform_in: TransformIn::Viewport,
							skip_rerender: false,
						});

						tool_options.drawing.apply_stroke_to_new_layer(layer, defered_responses);
						tool_options.drawing.fill.apply_fill(layer, defered_responses);
					}
					ShapeType::Arrow => {
						let viewport_drag_start = tool_data.data.viewport_drag_start(document);
						defered_responses.add(GraphOperationMessage::TransformSet {
							layer,
							transform: DAffine2::from_scale_angle_translation(DVec2::ONE, 0., viewport_drag_start),
							transform_in: TransformIn::Viewport,
							skip_rerender: false,
						});

						tool_data.line_data.weight = tool_options.drawing.effective_line_weight();
						tool_data.line_data.editing_layer = Some(layer);
						tool_options.drawing.apply_stroke_to_new_layer(layer, defered_responses);
						tool_options.drawing.fill.apply_fill(layer, defered_responses);
					}
					ShapeType::Line => {
						let viewport_drag_start = tool_data.data.viewport_drag_start(document);
						defered_responses.add(GraphOperationMessage::TransformSet {
							layer,
							transform: DAffine2::from_scale_angle_translation(DVec2::ONE, 0., viewport_drag_start),
							transform_in: TransformIn::Viewport,
							skip_rerender: false,
						});

						tool_data.line_data.weight = tool_options.drawing.effective_line_weight();
						tool_data.line_data.editing_layer = Some(layer);
						tool_options.drawing.apply_stroke_to_new_layer(layer, defered_responses);
					}
				}

				tool_data.data.layer = Some(layer);

				responses.add(DeferMessage::AfterGraphRun {
					messages: defered_responses.drain(..).collect(),
				});
				responses.add(NodeGraphMessage::RunDocumentGraph);

				ShapeToolFsmState::Drawing(tool_data.current_shape)
			}
			(ShapeToolFsmState::Drawing(shape), ShapeToolMessage::PointerMove { modifier }) => {
				let Some(layer) = tool_data.data.layer else {
					return ShapeToolFsmState::Ready(shape);
				};

				match tool_data.current_shape {
					ShapeType::Polygon => Polygon::update_shape(document, input, viewport, layer, tool_data, modifier, responses),
					ShapeType::Star => Star::update_shape(document, input, viewport, layer, tool_data, modifier, responses),
					ShapeType::Circle => Circle::update_shape(document, input, viewport, layer, tool_data, modifier, responses),
					ShapeType::Arc => Arc::update_shape(document, input, viewport, layer, tool_data, modifier, responses),
					ShapeType::Spiral => Spiral::update_shape(document, input, viewport, layer, tool_data, responses),
					ShapeType::Grid => Grid::update_shape(document, input, layer, tool_options.grid_type, tool_data, modifier, responses),
					ShapeType::Arrow => Arrow::update_shape(document, input, viewport, layer, tool_data, modifier, responses),
					ShapeType::Line => Line::update_shape(document, input, viewport, layer, tool_data, modifier, responses),
					ShapeType::Rectangle => Rectangle::update_shape(document, input, viewport, layer, tool_data, modifier, responses),
					ShapeType::Ellipse => Ellipse::update_shape(document, input, viewport, layer, tool_data, modifier, responses),
				}

				// Auto-panning
				let messages = [ShapeToolMessage::PointerOutsideViewport { modifier }.into(), ShapeToolMessage::PointerMove { modifier }.into()];
				tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);

				self
			}
			(ShapeToolFsmState::DraggingLineEndpoints, ShapeToolMessage::PointerMove { modifier }) => {
				let Some(layer) = tool_data.line_data.editing_layer else {
					return ShapeToolFsmState::Ready(tool_data.current_shape);
				};

				if graph_modification_utils::get_arrow_id(layer, &document.network_interface).is_some() {
					Arrow::update_shape(document, input, viewport, layer, tool_data, modifier, responses);
				} else {
					Line::update_shape(document, input, viewport, layer, tool_data, modifier, responses);
				}

				// Auto-panning
				let messages = [ShapeToolMessage::PointerOutsideViewport { modifier }.into(), ShapeToolMessage::PointerMove { modifier }.into()];
				tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);

				self
			}
			(ShapeToolFsmState::ModifyingGizmo, ShapeToolMessage::PointerMove { .. }) => {
				tool_data.gizmo_manager.handle_update(tool_data.data.viewport_drag_start(document), document, input, responses);

				responses.add(OverlaysMessage::Draw);

				ShapeToolFsmState::ModifyingGizmo
			}
			(ShapeToolFsmState::ResizingBounds, ShapeToolMessage::PointerMove { modifier }) => {
				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					let messages = [ShapeToolMessage::PointerOutsideViewport { modifier }.into(), ShapeToolMessage::PointerMove { modifier }.into()];
					resize_bounds(
						document,
						responses,
						bounds,
						&mut tool_data.layers_dragging,
						&mut tool_data.data.snap_manager,
						&mut tool_data.snap_candidates,
						input,
						viewport,
						input.keyboard.key(modifier[0]),
						input.keyboard.key(modifier[1]),
						ToolType::Shape,
					);
					tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);
				}

				responses.add(OverlaysMessage::Draw);
				ShapeToolFsmState::ResizingBounds
			}
			(ShapeToolFsmState::RotatingBounds, ShapeToolMessage::PointerMove { modifier }) => {
				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					rotate_bounds(
						document,
						responses,
						bounds,
						&mut tool_data.layers_dragging,
						tool_data.data.drag_start,
						input.pointer.position,
						input.keyboard.key(modifier[1]),
						ToolType::Shape,
					);
				}

				ShapeToolFsmState::RotatingBounds
			}
			(ShapeToolFsmState::SkewingBounds { skew }, ShapeToolMessage::PointerMove { .. }) => {
				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					skew_bounds(
						document,
						responses,
						bounds,
						input.keyboard.key(skew),
						&mut tool_data.layers_dragging,
						input.pointer.position,
						ToolType::Shape,
					);
				}

				ShapeToolFsmState::SkewingBounds { skew }
			}

			(_, ShapeToolMessage::PointerMove { .. }) => {
				let dragging_bounds = tool_data
					.bounding_box_manager
					.as_mut()
					.and_then(|bounding_box| bounding_box.check_selected_edges(input.pointer.position))
					.is_some();

				let cursor = tool_data.bounding_box_manager.as_ref().map_or(MouseCursorIcon::Crosshair, |bounds| {
					let cursor = bounds.get_cursor(input, true, dragging_bounds, Some(tool_data.skew_edge));
					if cursor == MouseCursorIcon::Default { MouseCursorIcon::Crosshair } else { cursor }
				});

				if tool_data.cursor != cursor {
					tool_data.cursor = cursor;
					responses.add(FrontendMessage::UpdateMouseCursor { cursor });
				}

				tool_data.data.snap_manager.preview_draw(&SnapData::new(document, input, viewport), input.pointer.position);

				responses.add(OverlaysMessage::Draw);
				self
			}
			(ShapeToolFsmState::ResizingBounds | ShapeToolFsmState::SkewingBounds { .. }, ShapeToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, viewport, responses)
					&& let Some(bounds) = &mut tool_data.bounding_box_manager
				{
					bounds.center_of_transformation += shift;
					bounds.original_bound_transform.translation += shift;
				}

				self
			}
			(ShapeToolFsmState::Ready(_), ShapeToolMessage::PointerOutsideViewport { .. }) => self,
			(_, ShapeToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				let _ = tool_data.auto_panning.shift_viewport(input, viewport, responses);
				self
			}
			(
				ShapeToolFsmState::Drawing(_)
				| ShapeToolFsmState::DraggingLineEndpoints
				| ShapeToolFsmState::ResizingBounds
				| ShapeToolFsmState::RotatingBounds
				| ShapeToolFsmState::SkewingBounds { .. }
				| ShapeToolFsmState::ModifyingGizmo,
				ShapeToolMessage::DragStop,
			) => {
				input.pointer.finish_transaction(tool_data.data.drag_start, responses);
				tool_data.data.cleanup(responses);

				tool_data.gizmo_manager.handle_cleanup();

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				tool_data.line_data.dragging_endpoint = None;
				tool_data.line_data.editing_layer = None;

				responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });

				ShapeToolFsmState::Ready(tool_data.current_shape)
			}
			(
				ShapeToolFsmState::Drawing(_)
				| ShapeToolFsmState::DraggingLineEndpoints
				| ShapeToolFsmState::ResizingBounds
				| ShapeToolFsmState::RotatingBounds
				| ShapeToolFsmState::SkewingBounds { .. }
				| ShapeToolFsmState::ModifyingGizmo,
				ShapeToolMessage::Abort,
			) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.data.cleanup(responses);
				tool_data.line_data.dragging_endpoint = None;
				tool_data.line_data.editing_layer = None;

				tool_data.gizmo_manager.handle_cleanup();

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				tool_data.cursor = MouseCursorIcon::Crosshair;
				responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });

				ShapeToolFsmState::Ready(tool_data.current_shape)
			}
			(_, ShapeToolMessage::WorkingColorChanged) => {
				responses.add(ShapeToolMessage::UpdateOptions {
					options: ShapeOptionsUpdate::WorkingColorsChanged,
				});
				self
			}
			(_, ShapeToolMessage::SetShape { shape }) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.data.cleanup(responses);
				tool_data.current_shape = shape;
				// Update hints for the new shape (without updating options.shape_type)
				update_dynamic_hints(&ShapeToolFsmState::Ready(shape), responses, tool_data);
				ShapeToolFsmState::Ready(shape)
			}
			(_, ShapeToolMessage::SyncShapeWithOptions) => {
				// Sync current_shape with the dropdown selection when returning from alias tools
				tool_data.current_shape = tool_options.shape_type;
				update_dynamic_hints(&ShapeToolFsmState::Ready(tool_options.shape_type), responses, tool_data);
				ShapeToolFsmState::Ready(tool_options.shape_type)
			}
			(_, ShapeToolMessage::HideShapeTypeWidget { hide }) => {
				tool_data.hide_shape_option_widget = hide;
				responses.add(ToolMessage::RefreshToolOptions);
				self
			}
			_ => self,
		}
	}

	fn update_hints(&self, _responses: &mut VecDeque<Message>) {
		// Moved logic to update_dynamic_hints
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
	}
}

fn update_dynamic_hints(state: &ShapeToolFsmState, responses: &mut VecDeque<Message>, tool_data: &ShapeToolData) {
	let hint_data = match state {
		ShapeToolFsmState::Ready(_) => {
			let hint_groups = match tool_data.current_shape {
				ShapeType::Polygon | ShapeType::Star => vec![
					HintGroup(vec![
						HintInfo::mouse(MouseMotion::LmbDrag, "Draw Polygon"),
						HintInfo::keys([Key::Shift], "Constrain Regular").prepend_plus(),
						HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
					]),
					HintGroup(vec![HintInfo::multi_keys([[Key::BracketLeft], [Key::BracketRight]], "Decrease/Increase Sides")]),
				],
				ShapeType::Circle => vec![HintGroup(vec![
					HintInfo::mouse(MouseMotion::LmbDrag, "Draw Circle"),
					HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
				])],
				ShapeType::Arc => vec![HintGroup(vec![
					HintInfo::mouse(MouseMotion::LmbDrag, "Draw Arc"),
					HintInfo::keys([Key::Shift], "Constrain Arc").prepend_plus(),
					HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
				])],
				ShapeType::Spiral => vec![
					HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Draw Spiral")]),
					HintGroup(vec![HintInfo::multi_keys([[Key::BracketLeft], [Key::BracketRight]], "Decrease/Increase Turns")]),
				],
				ShapeType::Grid => vec![HintGroup(vec![
					HintInfo::mouse(MouseMotion::LmbDrag, "Draw Grid"),
					HintInfo::keys([Key::Shift], "Constrain Regular").prepend_plus(),
					HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
				])],
				ShapeType::Arrow => vec![HintGroup(vec![
					HintInfo::mouse(MouseMotion::LmbDrag, "Draw Arrow"),
					HintInfo::keys([Key::Shift], "15° Increments").prepend_plus(),
					HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
					HintInfo::keys([Key::Control], "Lock Angle").prepend_plus(),
				])],
				ShapeType::Line => vec![HintGroup(vec![
					HintInfo::mouse(MouseMotion::LmbDrag, "Draw Line"),
					HintInfo::keys([Key::Shift], "15° Increments").prepend_plus(),
					HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
					HintInfo::keys([Key::Control], "Lock Angle").prepend_plus(),
				])],
				ShapeType::Rectangle => vec![HintGroup(vec![
					HintInfo::mouse(MouseMotion::LmbDrag, "Draw Rectangle"),
					HintInfo::keys([Key::Shift], "Constrain Square").prepend_plus(),
					HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
				])],
				ShapeType::Ellipse => vec![HintGroup(vec![
					HintInfo::mouse(MouseMotion::LmbDrag, "Draw Ellipse"),
					HintInfo::keys([Key::Shift], "Constrain Circular").prepend_plus(),
					HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
				])],
			};
			HintData(hint_groups)
		}
		ShapeToolFsmState::Drawing(shape) => {
			let mut common_hint_group = vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])];
			let tool_hint_group = match shape {
				ShapeType::Polygon | ShapeType::Star | ShapeType::Arc => HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Regular"), HintInfo::keys([Key::Alt], "From Center")]),
				ShapeType::Circle => HintGroup(vec![HintInfo::keys([Key::Alt], "From Center")]),
				ShapeType::Spiral => HintGroup(vec![]),
				ShapeType::Grid => HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Regular"), HintInfo::keys([Key::Alt], "From Center")]),
				ShapeType::Arrow => HintGroup(vec![
					HintInfo::keys([Key::Shift], "15° Increments"),
					HintInfo::keys([Key::Alt], "From Center"),
					HintInfo::keys([Key::Control], "Lock Angle"),
				]),
				ShapeType::Line => HintGroup(vec![
					HintInfo::keys([Key::Shift], "15° Increments"),
					HintInfo::keys([Key::Alt], "From Center"),
					HintInfo::keys([Key::Control], "Lock Angle"),
				]),
				ShapeType::Rectangle => HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Square"), HintInfo::keys([Key::Alt], "From Center")]),
				ShapeType::Ellipse => HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Circular"), HintInfo::keys([Key::Alt], "From Center")]),
			};

			if !tool_hint_group.0.is_empty() {
				common_hint_group.push(tool_hint_group);
			}

			if matches!(shape, ShapeType::Polygon | ShapeType::Star) {
				common_hint_group.push(HintGroup(vec![HintInfo::multi_keys([[Key::BracketLeft], [Key::BracketRight]], "Decrease/Increase Sides")]));
			}

			if matches!(shape, ShapeType::Spiral) {
				common_hint_group.push(HintGroup(vec![HintInfo::multi_keys([[Key::BracketLeft], [Key::BracketRight]], "Decrease/Increase Turns")]));
			}

			HintData(common_hint_group)
		}
		ShapeToolFsmState::DraggingLineEndpoints => HintData(vec![
			HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
			HintGroup(vec![
				HintInfo::keys([Key::Shift], "15° Increments"),
				HintInfo::keys([Key::Alt], "From Center"),
				HintInfo::keys([Key::Control], "Lock Angle"),
			]),
		]),
		ShapeToolFsmState::ResizingBounds => HintData(vec![
			HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
			HintGroup(vec![HintInfo::keys([Key::Alt], "From Pivot"), HintInfo::keys([Key::Shift], "Preserve Aspect Ratio")]),
		]),
		ShapeToolFsmState::RotatingBounds => HintData(vec![
			HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
			HintGroup(vec![HintInfo::keys([Key::Shift], "15° Increments")]),
		]),
		ShapeToolFsmState::SkewingBounds { .. } => HintData(vec![
			HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
			HintGroup(vec![HintInfo::keys([Key::Control], "Unlock Slide")]),
		]),
		ShapeToolFsmState::ModifyingGizmo => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])]),
	};
	hint_data.send_layout(responses);
}
