use super::tool_prelude::*;
use crate::consts::{DEFAULT_STROKE_WIDTH, SNAP_POINT_TOLERANCE};
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::gizmos::gizmo_manager::GizmoManager;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;
use crate::messages::tool::common_functionality::resize::Resize;
use crate::messages::tool::common_functionality::shapes::line_shape::{LineToolData, clicked_on_line_endpoints};
use crate::messages::tool::common_functionality::shapes::polygon_shape::Polygon;
use crate::messages::tool::common_functionality::shapes::shape_utility::{ShapeToolModifierKey, ShapeType, anchor_overlays, transform_cage_overlays};
use crate::messages::tool::common_functionality::shapes::star_shape::Star;
use crate::messages::tool::common_functionality::shapes::{Ellipse, Line, Rectangle};
use crate::messages::tool::common_functionality::snapping::{self, SnapCandidatePoint, SnapData, SnapTypeConfiguration};
use crate::messages::tool::common_functionality::transformation_cage::{BoundingBoxManager, EdgeBool};
use crate::messages::tool::common_functionality::utility_functions::{closest_point, resize_bounds, rotate_bounds, skew_bounds, transforming_transform_cage};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graphene_std::Color;
use graphene_std::renderer::Quad;
use graphene_std::vector::misc::ArcType;
use std::vec;

#[derive(Default)]
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
}

#[impl_message(Message, ToolMessage, Shape)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ShapeToolMessage {
	// Standard messages
	Overlays(OverlayContext),
	Abort,
	WorkingColorChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	HideShapeTypeWidget(bool),
	PointerMove(ShapeToolModifierKey),
	PointerOutsideViewport(ShapeToolModifierKey),
	UpdateOptions(ShapeOptionsUpdate),
	SetShape(ShapeType),

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
		.on_update(|number_input: &NumberInput| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::Vertices(number_input.value.unwrap() as u32)).into())
		.widget_holder()
}

fn create_shape_option_widget(shape_type: ShapeType) -> WidgetHolder {
	let entries = vec![vec![
		MenuListEntry::new("Polygon")
			.label("Polygon")
			.on_commit(move |_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::ShapeType(ShapeType::Polygon)).into()),
		MenuListEntry::new("Star")
			.label("Star")
			.on_commit(move |_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::ShapeType(ShapeType::Star)).into()),
	]];
	DropdownInput::new(entries).selected_index(Some(shape_type as u32)).widget_holder()
}

fn create_weight_widget(line_weight: f64) -> WidgetHolder {
	NumberInput::new(Some(line_weight))
		.unit(" px")
		.label("Weight")
		.min(0.)
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
		.widget_holder()
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
		}

		if self.options.shape_type != ShapeType::Line {
			widgets.append(&mut self.options.fill.create_widgets(
				"Fill",
				true,
				|_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::FillColor(None)).into(),
				|color_type: ToolColorType| WidgetCallback::new(move |_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::FillColorType(color_type.clone())).into()),
				|color: &ColorInput| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::FillColor(color.value.as_solid().map(|color| color.to_linear_srgb()))).into(),
			));

			widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		}

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::StrokeColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			|color: &ColorInput| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::StrokeColor(color.value.as_solid().map(|color| color.to_linear_srgb()))).into(),
		));
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for ShapeTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let ToolMessage::Shape(ShapeToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &self.options, responses, true);
			return;
		};
		match action {
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
		}

		self.fsm_state.update_hints(responses);
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
			overlay_provider: Some(|overlay_context| ShapeToolMessage::Overlays(overlay_context).into()),
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
	gizmo_manger: GizmoManager,
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
}

impl Fsm for ShapeToolFsmState {
	type ToolData = ShapeToolData;
	type ToolOptions = ShapeToolOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		ToolActionHandlerData {
			document,
			global_tool_data,
			input,
			preferences,
			shape_editor,
			..
		}: &mut ToolActionHandlerData,
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
			(_, ShapeToolMessage::Overlays(mut overlay_context)) => {
				let mouse_position = tool_data
					.data
					.snap_manager
					.indicator_pos()
					.map(|pos| document.metadata().document_to_viewport.transform_point2(pos))
					.unwrap_or(input.mouse.position);
				let is_resizing_or_rotating = matches!(self, ShapeToolFsmState::ResizingBounds | ShapeToolFsmState::SkewingBounds { .. } | ShapeToolFsmState::RotatingBounds);

				if matches!(self, Self::Ready(_)) && !input.keyboard.key(Key::Control) {
					tool_data.gizmo_manger.handle_actions(mouse_position, document, responses);
					tool_data.gizmo_manger.overlays(document, input, shape_editor, mouse_position, &mut overlay_context);
				}

				if matches!(self, ShapeToolFsmState::ModifyingGizmo) && !input.keyboard.key(Key::Control) {
					tool_data.gizmo_manger.dragging_overlays(document, input, shape_editor, mouse_position, &mut overlay_context);
				}

				let modifying_transform_cage = matches!(self, ShapeToolFsmState::ResizingBounds | ShapeToolFsmState::RotatingBounds | ShapeToolFsmState::SkewingBounds { .. });
				let hovering_over_gizmo = tool_data.gizmo_manger.hovering_over_gizmo();

				if !is_resizing_or_rotating && !matches!(self, ShapeToolFsmState::ModifyingGizmo) && !modifying_transform_cage && !hovering_over_gizmo {
					tool_data.data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);
				}

				if modifying_transform_cage && !matches!(self, ShapeToolFsmState::ModifyingGizmo) {
					transform_cage_overlays(document, tool_data, &mut overlay_context);
				}

				if input.keyboard.key(Key::Control) && matches!(self, ShapeToolFsmState::Ready(_)) {
					anchor_overlays(document, &mut overlay_context);
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
						if is_skewing || (dragging_bounds && is_near_square && !is_resizing_or_rotating && !hovering_over_gizmo) {
							bounds.render_skew_gizmos(&mut overlay_context, tool_data.skew_edge);
						}
						if !is_skewing && dragging_bounds && !hovering_over_gizmo {
							if let Some(edges) = edges {
								tool_data.skew_edge = bounds.get_closest_edge(edges, input.mouse.position);
							}
						}
					}
				}

				if matches!(self, ShapeToolFsmState::Drawing(_) | ShapeToolFsmState::DraggingLineEndpoints) {
					Line::overlays(document, tool_data, &mut overlay_context);
				}

				self
			}
			(ShapeToolFsmState::Ready(_), ShapeToolMessage::IncreaseSides) => {
				responses.add(ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::Vertices(tool_options.vertices + 1)));
				self
			}
			(ShapeToolFsmState::Ready(_), ShapeToolMessage::DecreaseSides) => {
				responses.add(ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::Vertices((tool_options.vertices - 1).max(3))));
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
				if let Some(layer) = tool_data.data.layer {
					let Some(node_id) = graph_modification_utils::get_polygon_id(layer, &document.network_interface).or(graph_modification_utils::get_star_id(layer, &document.network_interface))
					else {
						return self;
					};

					let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface)
						.find_node_inputs("Regular Polygon")
						.or(NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Star"))
					else {
						return self;
					};

					let Some(&TaggedValue::U32(n)) = node_inputs.get(1).unwrap().as_value() else {
						return self;
					};

					responses.add(ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::Vertices(n + 1)));

					responses.add(NodeGraphMessage::SetInput {
						input_connector: InputConnector::node(node_id, 1),
						input: NodeInput::value(TaggedValue::U32(n + 1), false),
					});
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}

				self
			}
			(ShapeToolFsmState::Drawing(_), ShapeToolMessage::DecreaseSides) => {
				if let Some(layer) = tool_data.data.layer {
					let Some(node_id) = graph_modification_utils::get_polygon_id(layer, &document.network_interface).or(graph_modification_utils::get_star_id(layer, &document.network_interface))
					else {
						return self;
					};

					let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface)
						.find_node_inputs("Regular Polygon")
						.or(NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Star"))
					else {
						return self;
					};

					let Some(&TaggedValue::U32(n)) = node_inputs.get(1).unwrap().as_value() else {
						return self;
					};

					responses.add(ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::Vertices((n - 1).max(3))));

					responses.add(NodeGraphMessage::SetInput {
						input_connector: InputConnector::node(node_id, 1),
						input: NodeInput::value(TaggedValue::U32((n - 1).max(3)), false),
					});
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}

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

				if tool_data.gizmo_manger.handle_click() {
					tool_data.data.drag_start = document.metadata().document_to_viewport.inverse().transform_point2(mouse_pos);
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
				) {
					if clicked_on_line_endpoints(layer, document, input, tool_data) && !input.keyboard.key(Key::Control) {
						return ShapeToolFsmState::DraggingLineEndpoints;
					}
				}

				let (resize, rotate, skew) = transforming_transform_cage(document, &mut tool_data.bounding_box_manager, input, responses, &mut tool_data.layers_dragging);

				if !input.keyboard.key(Key::Control) {
					match (resize, rotate, skew) {
						(true, false, false) => {
							tool_data.get_snap_candidates(document, input);
							return ShapeToolFsmState::ResizingBounds;
						}
						(false, true, false) => {
							tool_data.data.drag_start = mouse_pos;
							return ShapeToolFsmState::RotatingBounds;
						}
						(false, false, true) => {
							tool_data.get_snap_candidates(document, input);
							return ShapeToolFsmState::SkewingBounds { skew: Key::Control };
						}
						_ => {}
					}
				};

				match tool_data.current_shape {
					ShapeType::Polygon | ShapeType::Star | ShapeType::Ellipse | ShapeType::Rectangle => tool_data.data.start(document, input),
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
					ShapeType::Rectangle => Rectangle::create_node(),
					ShapeType::Ellipse => Ellipse::create_node(),
					ShapeType::Line => Line::create_node(document, tool_data.data.drag_start),
				};

				let nodes = vec![(NodeId(0), node)];
				let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, document.new_layer_bounding_artboard(input), responses);

				responses.add(Message::StartBuffer);

				match tool_data.current_shape {
					ShapeType::Ellipse | ShapeType::Rectangle | ShapeType::Polygon | ShapeType::Star => {
						responses.add(GraphOperationMessage::TransformSet {
							layer,
							transform: DAffine2::from_scale_angle_translation(DVec2::ONE, 0., input.mouse.position),
							transform_in: TransformIn::Viewport,
							skip_rerender: false,
						});

						tool_options.fill.apply_fill(layer, responses);
					}
					ShapeType::Line => {
						tool_data.line_data.weight = tool_options.line_weight;
						tool_data.line_data.editing_layer = Some(layer);
					}
				}
				tool_options.stroke.apply_stroke(tool_options.line_weight, layer, responses);

				tool_options.stroke.apply_stroke(tool_options.line_weight, layer, responses);
				tool_data.data.layer = Some(layer);

				ShapeToolFsmState::Drawing(tool_data.current_shape)
			}
			(ShapeToolFsmState::Drawing(shape), ShapeToolMessage::PointerMove(modifier)) => {
				let Some(layer) = tool_data.data.layer else {
					return ShapeToolFsmState::Ready(shape);
				};

				match tool_data.current_shape {
					ShapeType::Rectangle => Rectangle::update_shape(document, input, layer, tool_data, modifier, responses),
					ShapeType::Ellipse => Ellipse::update_shape(document, input, layer, tool_data, modifier, responses),
					ShapeType::Line => Line::update_shape(document, input, layer, tool_data, modifier, responses),
					ShapeType::Polygon => Polygon::update_shape(document, input, layer, tool_data, modifier, responses),
					ShapeType::Star => Star::update_shape(document, input, layer, tool_data, modifier, responses),
				}

				// Auto-panning
				let messages = [ShapeToolMessage::PointerOutsideViewport(modifier).into(), ShapeToolMessage::PointerMove(modifier).into()];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				self
			}
			(ShapeToolFsmState::DraggingLineEndpoints, ShapeToolMessage::PointerMove(modifier)) => {
				let Some(layer) = tool_data.line_data.editing_layer else {
					return ShapeToolFsmState::Ready(tool_data.current_shape);
				};

				Line::update_shape(document, input, layer, tool_data, modifier, responses);
				// Auto-panning
				let messages = [ShapeToolMessage::PointerOutsideViewport(modifier).into(), ShapeToolMessage::PointerMove(modifier).into()];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				self
			}
			(ShapeToolFsmState::ModifyingGizmo, ShapeToolMessage::PointerMove(..)) => {
				responses.add(DocumentMessage::StartTransaction);
				tool_data.gizmo_manger.handle_update(tool_data.data.drag_start, document, input, responses);

				responses.add(OverlaysMessage::Draw);

				ShapeToolFsmState::ModifyingGizmo
			}
			(ShapeToolFsmState::ResizingBounds, ShapeToolMessage::PointerMove(modifier)) => {
				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					let messages = [ShapeToolMessage::PointerOutsideViewport(modifier).into(), ShapeToolMessage::PointerMove(modifier).into()];
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
			(ShapeToolFsmState::RotatingBounds, ShapeToolMessage::PointerMove(modifier)) => {
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
			(ShapeToolFsmState::SkewingBounds { skew }, ShapeToolMessage::PointerMove(_)) => {
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

			(_, ShapeToolMessage::PointerMove(_)) => {
				let dragging_bounds = tool_data
					.bounding_box_manager
					.as_mut()
					.and_then(|bounding_box| bounding_box.check_selected_edges(input.mouse.position))
					.is_some();

				let cursor = tool_data.bounding_box_manager.as_ref().map_or(MouseCursorIcon::Crosshair, |bounds| {
					let cursor = bounds.get_cursor(input, true, dragging_bounds, Some(tool_data.skew_edge));
					if cursor == MouseCursorIcon::Default { MouseCursorIcon::Crosshair } else { cursor }
				});

				if tool_data.cursor != cursor && !input.keyboard.key(Key::Control) && !all_selected_layers_line {
					tool_data.cursor = cursor;
					responses.add(FrontendMessage::UpdateMouseCursor { cursor });
				}

				tool_data.data.snap_manager.preview_draw(&SnapData::new(document, input), input.mouse.position);

				responses.add(OverlaysMessage::Draw);
				self
			}
			(ShapeToolFsmState::ResizingBounds | ShapeToolFsmState::SkewingBounds { .. }, ShapeToolMessage::PointerOutsideViewport(_)) => {
				// Auto-panning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, responses) {
					if let Some(bounds) = &mut tool_data.bounding_box_manager {
						bounds.center_of_transformation += shift;
						bounds.original_bound_transform.translation += shift;
					}
				}

				self
			}
			(ShapeToolFsmState::Ready(_), ShapeToolMessage::PointerOutsideViewport(..)) => self,
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

				tool_data.gizmo_manger.handle_cleanup();

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

				tool_data.gizmo_manger.handle_cleanup();

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });

				ShapeToolFsmState::Ready(tool_data.current_shape)
			}
			(_, ShapeToolMessage::WorkingColorChanged) => {
				responses.add(ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::WorkingColors(
					Some(global_tool_data.primary_color),
					Some(global_tool_data.secondary_color),
				)));
				self
			}
			(_, ShapeToolMessage::SetShape(shape)) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.data.cleanup(responses);
				tool_data.current_shape = shape;

				ShapeToolFsmState::Ready(shape)
			}
			(_, ShapeToolMessage::HideShapeTypeWidget(hide)) => {
				tool_data.hide_shape_option_widget = hide;
				responses.add(ToolMessage::RefreshToolOptions);
				self
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			ShapeToolFsmState::Ready(shape) => {
				let hint_groups = match shape {
					ShapeType::Polygon | ShapeType::Star => vec![
						HintGroup(vec![
							HintInfo::mouse(MouseMotion::LmbDrag, "Draw Polygon"),
							HintInfo::keys([Key::Shift], "Constrain Regular").prepend_plus(),
							HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
						]),
						HintGroup(vec![HintInfo::multi_keys([[Key::BracketLeft], [Key::BracketRight]], "Decrease/Increase Sides")]),
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
				};
				HintData(hint_groups)
			}
			ShapeToolFsmState::Drawing(shape) => {
				let mut common_hint_group = vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])];
				let tool_hint_group = match shape {
					ShapeType::Polygon | ShapeType::Star => HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Regular"), HintInfo::keys([Key::Alt], "From Center")]),
					ShapeType::Rectangle => HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Square"), HintInfo::keys([Key::Alt], "From Center")]),
					ShapeType::Ellipse => HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Circular"), HintInfo::keys([Key::Alt], "From Center")]),
					ShapeType::Line => HintGroup(vec![
						HintInfo::keys([Key::Shift], "15° Increments"),
						HintInfo::keys([Key::Alt], "From Center"),
						HintInfo::keys([Key::Control], "Lock Angle"),
					]),
				};

				common_hint_group.push(tool_hint_group);

				if matches!(shape, ShapeType::Polygon | ShapeType::Star) {
					common_hint_group.push(HintGroup(vec![HintInfo::multi_keys([[Key::BracketLeft], [Key::BracketRight]], "Decrease/Increase Sides")]));
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

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
	}
}
