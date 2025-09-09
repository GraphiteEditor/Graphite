use super::tool_prelude::*;
use crate::consts::{DEFAULT_STROKE_WIDTH, SNAP_POINT_TOLERANCE};
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::gizmos::gizmo_manager::GizmoManager;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::resize::Resize;
use crate::messages::tool::common_functionality::shapes::arc_shape::Arc;
use crate::messages::tool::common_functionality::shapes::circle_shape::Circle;
use crate::messages::tool::common_functionality::shapes::grid_shape::Grid;
use crate::messages::tool::common_functionality::shapes::line_shape::{LineToolData, clicked_on_line_endpoints};
use crate::messages::tool::common_functionality::shapes::polygon_shape::Polygon;
use crate::messages::tool::common_functionality::shapes::shape_utility::{ShapeToolModifierKey, ShapeType, anchor_overlays, transform_cage_overlays};
use crate::messages::tool::common_functionality::shapes::spiral_shape::Spiral;
use crate::messages::tool::common_functionality::shapes::star_shape::Star;
use crate::messages::tool::common_functionality::shapes::{Ellipse, Line, Rectangle};
use crate::messages::tool::common_functionality::snapping::{self, SnapCandidatePoint, SnapData, SnapTypeConfiguration};
use crate::messages::tool::common_functionality::transformation_cage::{BoundingBoxManager, EdgeBool};
use crate::messages::tool::common_functionality::utility_functions::{closest_point, resize_bounds, rotate_bounds, skew_bounds, transforming_transform_cage};
use graph_craft::document::NodeId;
use graphene_std::Color;
use graphene_std::renderer::Quad;
use graphene_std::vector::misc::{ArcType, GridType, SpiralType};
use std::vec;

#[derive(Default, ExtractField)]
pub struct ShapeTool {
	fsm_state: ShapeToolFsmState,
	tool_data: ShapeToolData,
	options: ShapeToolOptions,
}

pub struct ShapeToolOptions {
	line_weight: f64,
	fill: ToolColorOptions,
	stroke: ToolColorOptions,
	vertices: u32,
	shape_type: ShapeType,
	arc_type: ArcType,
	grid_type: GridType,
	spiral_type: SpiralType,
	turns: f64,
}

impl Default for ShapeToolOptions {
	fn default() -> Self {
		Self {
			line_weight: DEFAULT_STROKE_WIDTH,
			fill: ToolColorOptions::new_secondary(),
			stroke: ToolColorOptions::new_primary(),
			vertices: 5,
			shape_type: ShapeType::Polygon,
			arc_type: ArcType::Open,
			spiral_type: SpiralType::Archimedean,
			turns: 5.,
			grid_type: GridType::Rectangular,
		}
	}
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ShapeOptionsUpdate {
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	LineWeight(f64),
	StrokeColor(Option<Color>),
	StrokeColorType(ToolColorType),
	WorkingColors(Option<Color>, Option<Color>),
	Vertices(u32),
	ShapeType(ShapeType),
	ArcType(ArcType),
	SpiralType(SpiralType),
	Turns(f64),
	GridType(GridType),
}

#[impl_message(Message, ToolMessage, Shape)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ShapeToolMessage {
	// Standard messages
	Overlays { context: OverlayContext },
	Abort,
	WorkingColorChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	HideShapeTypeWidget { hide: bool },
	PointerMove { modifier: ShapeToolModifierKey },
	PointerOutsideViewport { modifier: ShapeToolModifierKey },
	UpdateOptions { options: ShapeOptionsUpdate },
	SetShape { shape: ShapeType },

	IncreaseSides,
	DecreaseSides,

	NudgeSelectedLayers { delta_x: f64, delta_y: f64, resize: Key, resize_opposite_corner: Key },
}

fn create_sides_widget(vertices: u32) -> WidgetHolder {
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
		.widget_holder()
}

fn create_turns_widget(turns: f64) -> WidgetHolder {
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
		.widget_holder()
}

fn create_shape_option_widget(shape_type: ShapeType) -> WidgetHolder {
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
	]];
	DropdownInput::new(entries).selected_index(Some(shape_type as u32)).widget_holder()
}

fn create_arc_type_widget(arc_type: ArcType) -> WidgetHolder {
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
	RadioInput::new(entries).selected_index(Some(arc_type as u32)).widget_holder()
}

fn create_weight_widget(line_weight: f64) -> WidgetHolder {
	NumberInput::new(Some(line_weight))
		.unit(" px")
		.label("Weight")
		.min(0.)
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| {
			ShapeToolMessage::UpdateOptions {
				options: ShapeOptionsUpdate::LineWeight(number_input.value.unwrap()),
			}
			.into()
		})
		.widget_holder()
}

fn create_spiral_type_widget(spiral_type: SpiralType) -> WidgetHolder {
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
	DropdownInput::new(entries).selected_index(Some(spiral_type as u32)).widget_holder()
}

fn create_grid_type_widget(grid_type: GridType) -> WidgetHolder {
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
	RadioInput::new(entries).selected_index(Some(grid_type as u32)).widget_holder()
}

impl LayoutHolder for ShapeTool {
	fn layout(&self) -> Layout {
		let mut widgets = vec![];

		if !self.tool_data.hide_shape_option_widget {
			widgets.push(create_shape_option_widget(self.options.shape_type));
			widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

			if self.options.shape_type == ShapeType::Polygon || self.options.shape_type == ShapeType::Star {
				widgets.push(create_sides_widget(self.options.vertices));
				widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
			}

			if self.options.shape_type == ShapeType::Arc {
				widgets.push(create_arc_type_widget(self.options.arc_type));
				widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
			}
		}

		if self.options.shape_type == ShapeType::Spiral {
			widgets.push(create_spiral_type_widget(self.options.spiral_type));
			widgets.push(Separator::new(SeparatorType::Related).widget_holder());

			widgets.push(create_turns_widget(self.options.turns));
			widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		}

		if self.options.shape_type == ShapeType::Grid {
			widgets.push(create_grid_type_widget(self.options.grid_type));
			widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		}

		if self.options.shape_type != ShapeType::Line {
			widgets.append(&mut self.options.fill.create_widgets(
				"Fill",
				true,
				|_| {
					ShapeToolMessage::UpdateOptions {
						options: ShapeOptionsUpdate::FillColor(None),
					}
					.into()
				},
				|color_type: ToolColorType| {
					WidgetCallback::new(move |_| {
						ShapeToolMessage::UpdateOptions {
							options: ShapeOptionsUpdate::FillColorType(color_type.clone()),
						}
						.into()
					})
				},
				|color: &ColorInput| {
					ShapeToolMessage::UpdateOptions {
						options: ShapeOptionsUpdate::FillColor(color.value.as_solid().map(|color| color.to_linear_srgb())),
					}
					.into()
				},
			));

			widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		}

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| {
				ShapeToolMessage::UpdateOptions {
					options: ShapeOptionsUpdate::StrokeColor(None),
				}
				.into()
			},
			|color_type: ToolColorType| {
				WidgetCallback::new(move |_| {
					ShapeToolMessage::UpdateOptions {
						options: ShapeOptionsUpdate::StrokeColorType(color_type.clone()),
					}
					.into()
				})
			},
			|color: &ColorInput| {
				ShapeToolMessage::UpdateOptions {
					options: ShapeOptionsUpdate::StrokeColor(color.value.as_solid().map(|color| color.to_linear_srgb())),
				}
				.into()
			},
		));
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for ShapeTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		let ToolMessage::Shape(ShapeToolMessage::UpdateOptions { options }) = message else {
			self.fsm_state.process_event(message, &mut self.tool_data, context, &self.options, responses, true);
			return;
		};
		match options {
			ShapeOptionsUpdate::FillColor(color) => {
				self.options.fill.custom_color = color;
				self.options.fill.color_type = ToolColorType::Custom;
			}
			ShapeOptionsUpdate::FillColorType(color_type) => {
				self.options.fill.color_type = color_type;
			}
			ShapeOptionsUpdate::LineWeight(line_weight) => {
				self.options.line_weight = line_weight;
			}
			ShapeOptionsUpdate::StrokeColor(color) => {
				self.options.stroke.custom_color = color;
				self.options.stroke.color_type = ToolColorType::Custom;
			}
			ShapeOptionsUpdate::StrokeColorType(color_type) => {
				self.options.stroke.color_type = color_type;
			}
			ShapeOptionsUpdate::WorkingColors(primary, secondary) => {
				self.options.stroke.primary_working_color = primary;
				self.options.stroke.secondary_working_color = secondary;
				self.options.fill.primary_working_color = primary;
				self.options.fill.secondary_working_color = secondary;
			}
			ShapeOptionsUpdate::ShapeType(shape) => {
				self.options.shape_type = shape;
				self.tool_data.current_shape = shape;
			}
			ShapeOptionsUpdate::Vertices(vertices) => {
				self.options.vertices = vertices;
			}
			ShapeOptionsUpdate::ArcType(arc_type) => {
				self.options.arc_type = arc_type;
			}
			ShapeOptionsUpdate::SpiralType(spiral_type) => {
				self.options.spiral_type = spiral_type;
			}
			ShapeOptionsUpdate::Turns(turns) => {
				self.options.turns = turns;
			}
			ShapeOptionsUpdate::GridType(grid_type) => {
				self.options.grid_type = grid_type;
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
	fn tooltip(&self) -> String {
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
	fn get_snap_candidates(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler) {
		self.snap_candidates.clear();
		for &layer in &self.layers_dragging {
			if (self.snap_candidates.len() as f64) < document.snapping_state.tolerance {
				snapping::get_layer_snap_points(layer, &SnapData::new(document, input), &mut self.snap_candidates);
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
			.and_then(|bounding_box| bounding_box.check_selected_edges(input.mouse.position))
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
			global_tool_data,
			input,
			preferences,
			shape_editor,
			..
		}: &mut ToolActionMessageContext,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let all_selected_layers_line = document
			.network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(&document.network_interface)
			.all(|layer| graph_modification_utils::get_line_id(layer, &document.network_interface).is_some());

		let ToolMessage::Shape(event) = event else { return self };

		match (self, event) {
			(_, ShapeToolMessage::Overlays { context: mut overlay_context }) => {
				let mouse_position = tool_data
					.data
					.snap_manager
					.indicator_pos()
					.map(|pos| document.metadata().document_to_viewport.transform_point2(pos))
					.unwrap_or(input.mouse.position);

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

				if !matches!(self, ShapeToolFsmState::ModifyingGizmo) && !modifying_transform_cage && !hovering_over_gizmo {
					tool_data.data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);
				}

				if modifying_transform_cage && !matches!(self, ShapeToolFsmState::ModifyingGizmo) {
					transform_cage_overlays(document, tool_data, &mut overlay_context);
					responses.add(FrontendMessage::UpdateMouseCursor { cursor: tool_data.cursor });
				}

				if input.keyboard.key(Key::Control) && matches!(self, ShapeToolFsmState::Ready(_)) {
					anchor_overlays(document, &mut overlay_context);
					responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
				} else if matches!(self, ShapeToolFsmState::Ready(_)) {
					Line::overlays(document, tool_data, &mut overlay_context);

					if all_selected_layers_line {
						return self;
					}

					if !hovering_over_gizmo {
						transform_cage_overlays(document, tool_data, &mut overlay_context);
					}

					let dragging_bounds = tool_data
						.bounding_box_manager
						.as_mut()
						.and_then(|bounding_box| bounding_box.check_selected_edges(input.mouse.position))
						.is_some();

					if let Some(bounds) = tool_data.bounding_box_manager.as_mut() {
						let edges = bounds.check_selected_edges(input.mouse.position);
						let is_skewing = matches!(self, ShapeToolFsmState::SkewingBounds { .. });
						let is_near_square = edges.is_some_and(|hover_edge| bounds.over_extended_edge_midpoint(input.mouse.position, hover_edge));
						if is_skewing || (dragging_bounds && is_near_square && !hovering_over_gizmo) {
							bounds.render_skew_gizmos(&mut overlay_context, tool_data.skew_edge);
						}
						if dragging_bounds
							&& !is_skewing && !hovering_over_gizmo
							&& let Some(edges) = edges
						{
							tool_data.skew_edge = bounds.get_closest_edge(edges, input.mouse.position);
						}
					}

					let cursor = tool_data.gizmo_manager.mouse_cursor_icon().unwrap_or_else(|| tool_data.transform_cage_mouse_icon(input));

					tool_data.cursor = cursor;
					responses.add(FrontendMessage::UpdateMouseCursor { cursor });
				}

				if matches!(self, ShapeToolFsmState::Drawing(_) | ShapeToolFsmState::DraggingLineEndpoints) {
					Line::overlays(document, tool_data, &mut overlay_context);
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
				tool_data.line_data.drag_start = input.mouse.position;

				// Snapped position in viewport space
				let mouse_pos = tool_data
					.data
					.snap_manager
					.indicator_pos()
					.map(|pos| document.metadata().document_to_viewport.transform_point2(pos))
					.unwrap_or(input.mouse.position);

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

				// If clicked on endpoints of a selected line, drag its endpoints
				if let Some((layer, _, _)) = closest_point(
					document,
					mouse_pos,
					SNAP_POINT_TOLERANCE,
					document.network_interface.selected_nodes().selected_visible_and_unlocked_layers(&document.network_interface),
					|_| false,
					preferences,
				) && clicked_on_line_endpoints(layer, document, input, tool_data)
					&& !input.keyboard.key(Key::Control)
				{
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
							tool_data.get_snap_candidates(document, input);
							update_cursor_and_pointer(tool_data, responses);

							return ShapeToolFsmState::ResizingBounds;
						}
						(false, true, false) => {
							tool_data.data.drag_start = mouse_pos;
							update_cursor_and_pointer(tool_data, responses);

							return ShapeToolFsmState::RotatingBounds;
						}
						(false, false, true) => {
							tool_data.get_snap_candidates(document, input);
							update_cursor_and_pointer(tool_data, responses);

							return ShapeToolFsmState::SkewingBounds { skew: Key::Control };
						}
						_ => {}
					}
				};

				match tool_data.current_shape {
					ShapeType::Polygon | ShapeType::Star | ShapeType::Circle | ShapeType::Arc | ShapeType::Spiral | ShapeType::Grid | ShapeType::Rectangle | ShapeType::Ellipse => {
						tool_data.data.start(document, input)
					}
					ShapeType::Line => {
						let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position));
						let snapped = tool_data.data.snap_manager.free_snap(&SnapData::new(document, input), &point, SnapTypeConfiguration::default());
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
					ShapeType::Rectangle => Rectangle::create_node(),
					ShapeType::Ellipse => Ellipse::create_node(),
					ShapeType::Line => Line::create_node(document, tool_data.data.drag_start),
				};

				let nodes = vec![(NodeId(0), node)];
				let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, document.new_layer_bounding_artboard(input), responses);

				let defered_responses = &mut VecDeque::new();

				match tool_data.current_shape {
					ShapeType::Polygon | ShapeType::Star | ShapeType::Circle | ShapeType::Arc | ShapeType::Spiral | ShapeType::Grid | ShapeType::Rectangle | ShapeType::Ellipse => {
						defered_responses.add(GraphOperationMessage::TransformSet {
							layer,
							transform: DAffine2::from_scale_angle_translation(DVec2::ONE, 0., input.mouse.position),
							transform_in: TransformIn::Viewport,
							skip_rerender: false,
						});

						tool_options.fill.apply_fill(layer, defered_responses);
					}
					ShapeType::Line => {
						tool_data.line_data.weight = tool_options.line_weight;
						tool_data.line_data.editing_layer = Some(layer);
					}
				}
				tool_options.stroke.apply_stroke(tool_options.line_weight, layer, defered_responses);

				tool_options.stroke.apply_stroke(tool_options.line_weight, layer, defered_responses);
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
					ShapeType::Polygon => Polygon::update_shape(document, input, layer, tool_data, modifier, responses),
					ShapeType::Star => Star::update_shape(document, input, layer, tool_data, modifier, responses),
					ShapeType::Circle => Circle::update_shape(document, input, layer, tool_data, modifier, responses),
					ShapeType::Arc => Arc::update_shape(document, input, layer, tool_data, modifier, responses),
					ShapeType::Spiral => Spiral::update_shape(document, input, layer, tool_data, responses),
					ShapeType::Grid => Grid::update_shape(document, input, layer, tool_options.grid_type, tool_data, modifier, responses),
					ShapeType::Rectangle => Rectangle::update_shape(document, input, layer, tool_data, modifier, responses),
					ShapeType::Ellipse => Ellipse::update_shape(document, input, layer, tool_data, modifier, responses),
					ShapeType::Line => Line::update_shape(document, input, layer, tool_data, modifier, responses),
				}

				// Auto-panning
				let messages = [ShapeToolMessage::PointerOutsideViewport { modifier }.into(), ShapeToolMessage::PointerMove { modifier }.into()];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				self
			}
			(ShapeToolFsmState::DraggingLineEndpoints, ShapeToolMessage::PointerMove { modifier }) => {
				let Some(layer) = tool_data.line_data.editing_layer else {
					return ShapeToolFsmState::Ready(tool_data.current_shape);
				};

				Line::update_shape(document, input, layer, tool_data, modifier, responses);
				// Auto-panning
				let messages = [ShapeToolMessage::PointerOutsideViewport { modifier }.into(), ShapeToolMessage::PointerMove { modifier }.into()];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

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
						input.keyboard.key(modifier[0]),
						input.keyboard.key(modifier[1]),
						ToolType::Shape,
					);
					tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);
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
						input.mouse.position,
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
						input.mouse.position,
						ToolType::Shape,
					);
				}

				ShapeToolFsmState::SkewingBounds { skew }
			}

			(_, ShapeToolMessage::PointerMove { .. }) => {
				let dragging_bounds = tool_data
					.bounding_box_manager
					.as_mut()
					.and_then(|bounding_box| bounding_box.check_selected_edges(input.mouse.position))
					.is_some();

				let cursor = tool_data.bounding_box_manager.as_ref().map_or(MouseCursorIcon::Crosshair, |bounds| {
					let cursor = bounds.get_cursor(input, true, dragging_bounds, Some(tool_data.skew_edge));
					if cursor == MouseCursorIcon::Default { MouseCursorIcon::Crosshair } else { cursor }
				});

				if tool_data.cursor != cursor {
					tool_data.cursor = cursor;
					responses.add(FrontendMessage::UpdateMouseCursor { cursor });
				}

				tool_data.data.snap_manager.preview_draw(&SnapData::new(document, input), input.mouse.position);

				responses.add(OverlaysMessage::Draw);
				self
			}
			(ShapeToolFsmState::ResizingBounds | ShapeToolFsmState::SkewingBounds { .. }, ShapeToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, responses)
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
				let _ = tool_data.auto_panning.shift_viewport(input, responses);
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
				input.mouse.finish_transaction(tool_data.data.drag_start, responses);
				tool_data.data.cleanup(responses);

				tool_data.gizmo_manager.handle_cleanup();

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				tool_data.line_data.dragging_endpoint = None;

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
					options: ShapeOptionsUpdate::WorkingColors(Some(global_tool_data.primary_color), Some(global_tool_data.secondary_color)),
				});
				self
			}
			(_, ShapeToolMessage::SetShape { shape }) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.data.cleanup(responses);
				tool_data.current_shape = shape;
				responses.add(ShapeToolMessage::UpdateOptions {
					options: ShapeOptionsUpdate::ShapeType(shape),
				});

				responses.add(ShapeToolMessage::UpdateOptions {
					options: ShapeOptionsUpdate::ShapeType(shape),
				});
				ShapeToolFsmState::Ready(shape)
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
				ShapeType::Spiral => vec![
					HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Draw Spiral")]),
					HintGroup(vec![HintInfo::multi_keys([[Key::BracketLeft], [Key::BracketRight]], "Decrease/Increase Turns")]),
				],
				ShapeType::Ellipse => vec![HintGroup(vec![
					HintInfo::mouse(MouseMotion::LmbDrag, "Draw Ellipse"),
					HintInfo::keys([Key::Shift], "Constrain Circular").prepend_plus(),
					HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
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
				ShapeType::Circle => vec![HintGroup(vec![
					HintInfo::mouse(MouseMotion::LmbDrag, "Draw Circle"),
					HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
				])],
				ShapeType::Arc => vec![HintGroup(vec![
					HintInfo::mouse(MouseMotion::LmbDrag, "Draw Arc"),
					HintInfo::keys([Key::Shift], "Constrain Arc").prepend_plus(),
					HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
				])],
				ShapeType::Grid => vec![HintGroup(vec![
					HintInfo::mouse(MouseMotion::LmbDrag, "Draw Grid"),
					HintInfo::keys([Key::Shift], "Constrain Regular").prepend_plus(),
					HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
				])],
			};
			HintData(hint_groups)
		}
		ShapeToolFsmState::Drawing(shape) => {
			let mut common_hint_group = vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])];
			let tool_hint_group = match shape {
				ShapeType::Polygon | ShapeType::Star | ShapeType::Arc => HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Regular"), HintInfo::keys([Key::Alt], "From Center")]),
				ShapeType::Rectangle => HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Square"), HintInfo::keys([Key::Alt], "From Center")]),
				ShapeType::Ellipse => HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Circular"), HintInfo::keys([Key::Alt], "From Center")]),
				ShapeType::Grid => HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Regular"), HintInfo::keys([Key::Alt], "From Center")]),
				ShapeType::Line => HintGroup(vec![
					HintInfo::keys([Key::Shift], "15° Increments"),
					HintInfo::keys([Key::Alt], "From Center"),
					HintInfo::keys([Key::Control], "Lock Angle"),
				]),
				ShapeType::Circle => HintGroup(vec![HintInfo::keys([Key::Alt], "From Center")]),
				ShapeType::Spiral => HintGroup(vec![]),
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
	responses.add(FrontendMessage::UpdateInputHints { hint_data });
}
