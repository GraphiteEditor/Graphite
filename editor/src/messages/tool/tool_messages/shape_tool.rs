use super::tool_prelude::*;
use crate::consts::DEFAULT_STROKE_WIDTH;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::resize::Resize;
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapData, SnapTypeConfiguration};
use crate::messages::tool::shapes::{Ellipse, Line, LineEnd, LineInitData, Rectangle, ShapeToolModifierKey, ShapeType};
use graph_craft::document::NodeId;
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
}

impl Default for ShapeToolOptions {
	fn default() -> Self {
		Self {
			line_weight: DEFAULT_STROKE_WIDTH,
			fill: ToolColorOptions::new_secondary(),
			stroke: ToolColorOptions::new_primary(),
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
	PointerMove(ShapeToolModifierKey),
	PointerOutsideViewport(ShapeToolModifierKey),
	UpdateOptions(ShapeOptionsUpdate),
	SetShape(ShapeType),
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
		let mut widgets = self.options.fill.create_widgets(
			"Fill",
			true,
			|_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::FillColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::FillColorType(color_type.clone())).into()),
			|color: &ColorInput| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::FillColor(color.value.as_solid())).into(),
		);

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

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
			ShapeOptionsUpdate::FillColorType(color_type) => self.options.fill.color_type = color_type,
			ShapeOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			ShapeOptionsUpdate::StrokeColor(color) => {
				self.options.stroke.custom_color = color;
				self.options.stroke.color_type = ToolColorType::Custom;
			}
			ShapeOptionsUpdate::StrokeColorType(color_type) => self.options.stroke.color_type = color_type,
			ShapeOptionsUpdate::WorkingColors(primary, secondary) => {
				self.options.stroke.primary_working_color = primary;
				self.options.stroke.secondary_working_color = secondary;
				self.options.fill.primary_working_color = primary;
				self.options.fill.secondary_working_color = secondary;
			}
		}

		self.send_layout(responses, LayoutTarget::ToolOptions);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			ShapeToolFsmState::Ready => actions!(ShapeToolMessageDiscriminant;
				DragStart,
				PointerMove,
				SetShape
			),
			ShapeToolFsmState::Drawing => actions!(ShapeToolMessageDiscriminant;
				DragStop,
				Abort,
				PointerMove,
				SetShape
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ShapeToolFsmState {
	#[default]
	Ready,
	Drawing,
}

#[derive(Clone, Debug, Default)]
pub struct ShapeToolData {
	pub data: Resize,
	pub drag_current: DVec2,
	pub angle: f64,
	pub weight: f64,
	pub selected_layers_with_position: HashMap<LayerNodeIdentifier, [DVec2; 2]>,
	pub dragging_endpoint: Option<LineEnd>,
	auto_panning: AutoPanning,
	current_shape: ShapeType,
}

impl Fsm for ShapeToolFsmState {
	type ToolData = ShapeToolData;
	type ToolOptions = ShapeToolOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		ToolActionHandlerData {
			document, global_tool_data, input, ..
		}: &mut ToolActionHandlerData,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let shape_data = &mut tool_data.data;

		let ToolMessage::Shape(event) = event else { return self };
		match (self, event) {
			(_, ShapeToolMessage::Overlays(mut overlay_context)) => {
				shape_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);
				self
			}
			(ShapeToolFsmState::Ready, ShapeToolMessage::DragStart) => {
				match tool_data.current_shape {
					ShapeType::Ellipse | ShapeType::Rectangle => shape_data.start(document, input),
					ShapeType::Line => {
						let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position));
						let snapped = shape_data.snap_manager.free_snap(&SnapData::new(document, input), &point, SnapTypeConfiguration::default());
						shape_data.drag_start = snapped.snapped_point_document;
					}
				}

				responses.add(DocumentMessage::StartTransaction);

				let node = match tool_data.current_shape {
					ShapeType::Rectangle => Rectangle::create_node(),
					ShapeType::Ellipse => Ellipse::create_node(),
					ShapeType::Line => Line::create_node(&document, LineInitData { drag_start: shape_data.drag_start }),
				};
				let nodes = vec![(NodeId(0), node)];
				let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, document.new_layer_bounding_artboard(input), responses);

				responses.add(Message::StartBuffer);

				tool_options.stroke.apply_stroke(tool_options.line_weight, layer, responses);
				match tool_data.current_shape {
					ShapeType::Ellipse | ShapeType::Rectangle => {
						responses.add(GraphOperationMessage::TransformSet {
							layer,
							transform: DAffine2::from_scale_angle_translation(DVec2::ONE, 0., input.mouse.position),
							transform_in: TransformIn::Viewport,
							skip_rerender: false,
						});

						tool_options.fill.apply_fill(layer, responses);
					}
					ShapeType::Line => {
						tool_data.angle = 0.;
						tool_data.weight = tool_options.line_weight;
					}
				}

				shape_data.layer = Some(layer);

				ShapeToolFsmState::Drawing
			}
			(ShapeToolFsmState::Drawing, ShapeToolMessage::PointerMove(modifier)) => {
				let Some(layer) = shape_data.layer else { return ShapeToolFsmState::Ready };
				if match tool_data.current_shape {
					ShapeType::Rectangle => Rectangle::update_shape(&document, &input, layer, tool_data, modifier, responses),
					ShapeType::Ellipse => Ellipse::update_shape(&document, &input, layer, tool_data, modifier, responses),
					ShapeType::Line => Line::update_shape(&document, &input, layer, tool_data, modifier, responses),
				} {
					return if tool_data.current_shape == ShapeType::Line { ShapeToolFsmState::Ready } else { self };
				}

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
			(ShapeToolFsmState::Drawing, ShapeToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				let _ = tool_data.auto_panning.shift_viewport(input, responses);

				ShapeToolFsmState::Drawing
			}
			(state, ShapeToolMessage::PointerOutsideViewport(modifier)) => {
				// Auto-panning
				let messages = [ShapeToolMessage::PointerOutsideViewport(modifier).into(), ShapeToolMessage::PointerMove(modifier).into()];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(ShapeToolFsmState::Drawing, ShapeToolMessage::DragStop) => {
				input.mouse.finish_transaction(shape_data.viewport_drag_start(document), responses);
				shape_data.cleanup(responses);

				ShapeToolFsmState::Ready
			}
			(ShapeToolFsmState::Drawing, ShapeToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				shape_data.cleanup(responses);

				ShapeToolFsmState::Ready
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

				tool_data.current_shape = shape;
				ShapeToolFsmState::Ready
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			ShapeToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Draw Shape"),
				HintInfo::keys([Key::Shift], "Constrain Square").prepend_plus(),
				HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
			])]),
			ShapeToolFsmState::Drawing => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Square"), HintInfo::keys([Key::Alt], "From Center")]),
			]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
	}
}
