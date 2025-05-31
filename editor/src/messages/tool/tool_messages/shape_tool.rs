use std::vec;

use super::path_tool::DraggingState;
use super::tool_prelude::*;
use crate::consts::{BOUNDS_SELECT_THRESHOLD, DEFAULT_STROKE_WIDTH, SNAP_POINT_TOLERANCE};
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, *};
use crate::messages::tool::common_functionality::resize::Resize;
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapData, SnapTypeConfiguration};
use crate::messages::tool::common_functionality::utility_functions::closest_point;
use crate::messages::tool::shapes::convex_shape::Convex;
use crate::messages::tool::shapes::line_shape::LineToolData;
use crate::messages::tool::shapes::shape_utility::{ShapeToolModifierKey, ShapeType};
use crate::messages::tool::shapes::star_shape::Star;
use crate::messages::tool::shapes::{Ellipse, Line, LineEnd, Rectangle};
use graph_craft::document::NodeId;
use graph_craft::document::value::TaggedValue;
use graphene_core::Color;

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
}

impl Default for ShapeToolOptions {
	fn default() -> Self {
		Self {
			line_weight: DEFAULT_STROKE_WIDTH,
			fill: ToolColorOptions::new_secondary(),
			stroke: ToolColorOptions::new_primary(),
			shape_type: ShapeType::Convex,
			vertices: 5,
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
}

fn create_sides_widget(vertices: u32) -> WidgetHolder {
	NumberInput::new(Some(vertices as f64))
		.label("Sides")
		.int()
		.min(3.0)
		.max(1000.0)
		.mode(NumberInputMode::Increment)
		.on_update(|number_input: &NumberInput| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::Vertices(number_input.value.unwrap() as u32)).into())
		.widget_holder()
}

fn create_shape_option_widget(shape_type: ShapeType) -> WidgetHolder {
	let entries = vec![vec![
		MenuListEntry::new("convex")
			.label("Convex")
			.on_commit(move |_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::ShapeType(ShapeType::Convex)).into()),
		MenuListEntry::new("star")
			.label("Star")
			.on_commit(move |_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::ShapeType(ShapeType::Star)).into()),
		MenuListEntry::new("rectangle")
			.label("Rectangle")
			.on_commit(move |_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::ShapeType(ShapeType::Rectangle)).into()),
		MenuListEntry::new("ellipse")
			.label("Ellipse")
			.on_commit(move |_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::ShapeType(ShapeType::Ellipse)).into()),
		MenuListEntry::new("line")
			.label("Line")
			.on_commit(move |_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::ShapeType(ShapeType::Line)).into()),
	]];
	DropdownInput::new(entries).selected_index(Some(shape_type as u32)).widget_holder()
}

fn create_weight_widget(line_weight: f64) -> WidgetHolder {
	NumberInput::new(Some(line_weight))
		.unit(" px")
		.label("Weight")
		.min(0.0)
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

			if self.options.shape_type == ShapeType::Convex || self.options.shape_type == ShapeType::Star {
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
				|color: &ColorInput| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::FillColor(color.value.as_solid())).into(),
			));

			widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		}

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::StrokeColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			|color: &ColorInput| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::StrokeColor(color.value.as_solid())).into(),
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
			}
			ShapeOptionsUpdate::Vertices(vertices) => {
				self.options.vertices = vertices;
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
				HideShapeTypeWidget
			),
			ShapeToolFsmState::Drawing(_) => actions!(ShapeToolMessageDiscriminant;
				DragStop,
				Abort,
				PointerMove,
				SetShape,
				HideShapeTypeWidget
			),
			ShapeToolFsmState::LineDraggingEndpoints => actions!(ShapeToolMessageDiscriminant;
				DragStop,
				Abort,
				PointerMove,
				SetShape,
				HideShapeTypeWidget
			),
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
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
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
enum ShapeToolFsmState {
	Ready(ShapeType),
	Drawing(ShapeType),
	LineDraggingEndpoints,
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
	pub hide_shape_option_widget: bool,
	pub line_data: LineToolData,
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
			..
		}: &mut ToolActionHandlerData,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let shape_data = &mut tool_data.data;
		let line_data = &mut tool_data.line_data;

		let ToolMessage::Shape(event) = event else {
			return self;
		};
		match (self, event) {
			(_, ShapeToolMessage::Overlays(mut overlay_context)) => {
				shape_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);
				Line::overlays(document, tool_data, &mut overlay_context);

				self
			}
			(ShapeToolFsmState::Ready(_), ShapeToolMessage::DragStart) => {
				let intersectins = document.click(input);

				log::info!("{:?}", intersectins);

				if let Some((layer, _, _)) = closest_point(
					document,
					input.mouse.position,
					SNAP_POINT_TOLERANCE,
					document.network_interface.selected_nodes().selected_visible_and_unlocked_layers(&document.network_interface),
					|_| false,
					preferences,
				) {
					if check_clicked_on_endpoints(layer, document, input, line_data) {
						return ShapeToolFsmState::LineDraggingEndpoints;
					}
				}

				match tool_options.shape_type {
					ShapeType::Convex | ShapeType::Star | ShapeType::Ellipse | ShapeType::Rectangle => shape_data.start(document, input),
					ShapeType::Line => {
						let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position));
						let snapped = shape_data.snap_manager.free_snap(&SnapData::new(document, input), &point, SnapTypeConfiguration::default());
						line_data.drag_start = snapped.snapped_point_document;
						line_data.drag_begin = document.metadata().document_to_viewport.transform_point2(line_data.drag_start);
					}
				}

				responses.add(DocumentMessage::StartTransaction);

				if tool_options.shape_type == ShapeType::Line && Line::dragging_endpoints(document, input, line_data) {
					return ShapeToolFsmState::Drawing(tool_options.shape_type);
				}

				let node = match tool_options.shape_type {
					ShapeType::Convex => Convex::create_node(tool_options.vertices),
					ShapeType::Star => Star::create_node(tool_options.vertices),
					ShapeType::Rectangle => Rectangle::create_node(),
					ShapeType::Ellipse => Ellipse::create_node(),
					ShapeType::Line => Line::create_node(&document, line_data.drag_start),
				};

				let nodes = vec![(NodeId(0), node)];
				let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, document.new_layer_bounding_artboard(input), responses);

				responses.add(Message::StartBuffer);

				tool_options.stroke.apply_stroke(tool_options.line_weight, layer, responses);
				match tool_options.shape_type {
					ShapeType::Ellipse | ShapeType::Rectangle | ShapeType::Convex | ShapeType::Star => {
						responses.add(GraphOperationMessage::TransformSet {
							layer,
							transform: DAffine2::from_scale_angle_translation(DVec2::ONE, 0.0, input.mouse.position),
							transform_in: TransformIn::Viewport,
							skip_rerender: false,
						});

						tool_options.fill.apply_fill(layer, responses);
					}
					ShapeType::Line => {
						line_data.angle = 0.0;
						line_data.weight = tool_options.line_weight;
						line_data.editing_layer = Some(layer);
					}
				}

				shape_data.layer = Some(layer);

				ShapeToolFsmState::Drawing(tool_options.shape_type)
			}
			(ShapeToolFsmState::Drawing(shape), ShapeToolMessage::PointerMove(modifier)) => {
				let Some(layer) = shape_data.layer else {
					return ShapeToolFsmState::Ready(shape);
				};
				if match tool_options.shape_type {
					ShapeType::Rectangle => Rectangle::update_shape(&document, &input, layer, tool_data, modifier, responses),
					ShapeType::Ellipse => Ellipse::update_shape(&document, &input, layer, tool_data, modifier, responses),
					ShapeType::Line => Line::update_shape(&document, &input, layer, tool_data, modifier, responses),
					ShapeType::Convex => Convex::update_shape(&document, &input, layer, tool_data, modifier, responses),
					ShapeType::Star => Star::update_shape(&document, &input, layer, tool_data, modifier, responses),
				} {
					return if tool_options.shape_type == ShapeType::Line { ShapeToolFsmState::Ready(shape) } else { self };
				}

				// Auto-panning
				let messages = [ShapeToolMessage::PointerOutsideViewport(modifier).into(), ShapeToolMessage::PointerMove(modifier).into()];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				self
			}
			(ShapeToolFsmState::LineDraggingEndpoints, ShapeToolMessage::PointerMove(modifier)) => {
				log::info!("reaching here");
				let Some(layer) = line_data.editing_layer else {
					return ShapeToolFsmState::Ready(tool_options.shape_type);
				};

				Line::update_shape(&document, &input, layer, tool_data, modifier, responses);
				// Auto-panning
				let messages = [ShapeToolMessage::PointerOutsideViewport(modifier).into(), ShapeToolMessage::PointerMove(modifier).into()];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				self
			}
			(_, ShapeToolMessage::PointerMove { .. }) => {
				shape_data.snap_manager.preview_draw(&SnapData::new(document, input), input.mouse.position);
				responses.add(OverlaysMessage::Draw);
				self
			}
			(ShapeToolFsmState::Drawing(_) | ShapeToolFsmState::LineDraggingEndpoints, ShapeToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				let _ = tool_data.auto_panning.shift_viewport(input, responses);

				ShapeToolFsmState::Drawing(tool_options.shape_type)
			}
			(state, ShapeToolMessage::PointerOutsideViewport(modifier)) => {
				// Auto-panning
				let messages = [ShapeToolMessage::PointerOutsideViewport(modifier).into(), ShapeToolMessage::PointerMove(modifier).into()];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(ShapeToolFsmState::Drawing(_) | ShapeToolFsmState::LineDraggingEndpoints, ShapeToolMessage::DragStop) => {
				input.mouse.finish_transaction(shape_data.viewport_drag_start(document), responses);
				shape_data.cleanup(responses);

				ShapeToolFsmState::Ready(tool_options.shape_type)
			}
			(ShapeToolFsmState::Drawing(shape), ShapeToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				shape_data.cleanup(responses);

				ShapeToolFsmState::Ready(shape)
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
				shape_data.cleanup(responses);
				responses.add(ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::ShapeType(shape)));

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
				let hint_infos = match shape {
					ShapeType::Convex | ShapeType::Star => vec![
						HintInfo::mouse(MouseMotion::LmbDrag, "Draw Polygon"),
						HintInfo::keys([Key::Shift], "Constrain Regular").prepend_plus(),
						HintInfo::keys([Key::Alt], "From Center").prepend_plus(), // HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
					],
					ShapeType::Ellipse => vec![
						HintInfo::mouse(MouseMotion::LmbDrag, "Draw Ellipse"),
						HintInfo::keys([Key::Shift], "Constrain Circular").prepend_plus(),
						HintInfo::keys([Key::Alt], "From Center").prepend_plus(), // HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
					],
					ShapeType::Line => vec![
						HintInfo::mouse(MouseMotion::LmbDrag, "Draw Line"),
						HintInfo::keys([Key::Shift], "15° Increments").prepend_plus(),
						HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
						HintInfo::keys([Key::Control], "Lock Angle").prepend_plus(),
					],
					ShapeType::Rectangle => vec![
						HintInfo::mouse(MouseMotion::LmbDrag, "Draw Rectangle"),
						HintInfo::keys([Key::Shift], "Constrain Square").prepend_plus(),
						HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
					],
				};
				HintData(vec![HintGroup(hint_infos)])
			}
			ShapeToolFsmState::Drawing(shape) => {
				let mut common_hint_group = vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])];
				let tool_hint_group = match shape {
					ShapeType::Convex | ShapeType::Star => HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Regular"), HintInfo::keys([Key::Alt], "From Center")]),
					ShapeType::Rectangle => HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Square"), HintInfo::keys([Key::Alt], "From Center")]),
					ShapeType::Ellipse => HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Circular"), HintInfo::keys([Key::Alt], "From Center")]),
					ShapeType::Line => HintGroup(vec![
						HintInfo::keys([Key::Shift], "15° Increments"),
						HintInfo::keys([Key::Alt], "From Center"),
						HintInfo::keys([Key::Control], "Lock Angle"),
					]),
				};
				common_hint_group.push(tool_hint_group);
				HintData(common_hint_group)
			}
			ShapeToolFsmState::LineDraggingEndpoints => {
				let mut common_hint_group = vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])];
				HintData(common_hint_group)
			}
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
	}
}

fn check_clicked_on_endpoints(layer: LayerNodeIdentifier, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, line_data: &mut LineToolData) -> bool {
	let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Line") else {
		return false;
	};

	let (Some(&TaggedValue::DVec2(document_start)), Some(&TaggedValue::DVec2(document_end))) = (node_inputs[1].as_value(), node_inputs[2].as_value()) else {
		return false;
	};

	let transform = document.metadata().transform_to_viewport(layer);
	let viewport_x = transform.transform_vector2(DVec2::X).normalize_or_zero() * BOUNDS_SELECT_THRESHOLD;
	let viewport_y = transform.transform_vector2(DVec2::Y).normalize_or_zero() * BOUNDS_SELECT_THRESHOLD;
	let threshold_x = transform.inverse().transform_vector2(viewport_x).length();
	let threshold_y = transform.inverse().transform_vector2(viewport_y).length();

	let drag_start = input.mouse.position;
	let [start, end] = [document_start, document_end].map(|point| transform.transform_point2(point));

	let start_click = (drag_start.y - start.y).abs() < threshold_y && (drag_start.x - start.x).abs() < threshold_x;
	let end_click = (drag_start.y - end.y).abs() < threshold_y && (drag_start.x - end.x).abs() < threshold_x;

	if start_click || end_click {
		line_data.dragging_endpoint = Some(if end_click { LineEnd::End } else { LineEnd::Start });
		line_data.drag_start = if end_click { document_start } else { document_end };
		line_data.editing_layer = Some(layer);
		return true;
	}
	false
}
