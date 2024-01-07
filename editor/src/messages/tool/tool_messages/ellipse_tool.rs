use super::tool_prelude::*;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::resize::Resize;
use crate::messages::tool::common_functionality::snapping::SnapData;

use graph_craft::document::NodeId;
use graphene_core::uuid::generate_uuid;
use graphene_core::vector::style::{Fill, Stroke};
use graphene_core::Color;

#[derive(Default)]
pub struct EllipseTool {
	fsm_state: EllipseToolFsmState,
	data: EllipseToolData,
	options: EllipseToolOptions,
}

pub struct EllipseToolOptions {
	line_weight: f64,
	fill: ToolColorOptions,
	stroke: ToolColorOptions,
}

impl Default for EllipseToolOptions {
	fn default() -> Self {
		Self {
			line_weight: 5.,
			fill: ToolColorOptions::new_secondary(),
			stroke: ToolColorOptions::new_primary(),
		}
	}
}

#[remain::sorted]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum EllipseOptionsUpdate {
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	LineWeight(f64),
	StrokeColor(Option<Color>),
	StrokeColorType(ToolColorType),
	WorkingColors(Option<Color>, Option<Color>),
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Ellipse)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum EllipseToolMessage {
	// Standard messages
	#[remain::unsorted]
	Overlays(OverlayContext),
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	WorkingColorChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	PointerMove {
		center: Key,
		lock_ratio: Key,
	},
	UpdateOptions(EllipseOptionsUpdate),
}

impl ToolMetadata for EllipseTool {
	fn icon_name(&self) -> String {
		"VectorEllipseTool".into()
	}
	fn tooltip(&self) -> String {
		"Ellipse Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Ellipse
	}
}

fn create_weight_widget(line_weight: f64) -> WidgetHolder {
	NumberInput::new(Some(line_weight))
		.unit(" px")
		.label("Weight")
		.min(0.)
		.max((1_u64 << std::f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| EllipseToolMessage::UpdateOptions(EllipseOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
		.widget_holder()
}

impl LayoutHolder for EllipseTool {
	fn layout(&self) -> Layout {
		let mut widgets = self.options.fill.create_widgets(
			"Fill",
			true,
			|_| EllipseToolMessage::UpdateOptions(EllipseOptionsUpdate::FillColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| EllipseToolMessage::UpdateOptions(EllipseOptionsUpdate::FillColorType(color_type.clone())).into()),
			|color: &ColorButton| EllipseToolMessage::UpdateOptions(EllipseOptionsUpdate::FillColor(color.value)).into(),
		);

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| EllipseToolMessage::UpdateOptions(EllipseOptionsUpdate::StrokeColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| EllipseToolMessage::UpdateOptions(EllipseOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			|color: &ColorButton| EllipseToolMessage::UpdateOptions(EllipseOptionsUpdate::StrokeColor(color.value)).into(),
		));
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for EllipseTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let ToolMessage::Ellipse(EllipseToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.data, tool_data, &self.options, responses, true);
			return;
		};
		match action {
			EllipseOptionsUpdate::FillColor(color) => {
				self.options.fill.custom_color = color;
				self.options.fill.color_type = ToolColorType::Custom;
			}
			EllipseOptionsUpdate::FillColorType(color_type) => self.options.fill.color_type = color_type,
			EllipseOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			EllipseOptionsUpdate::StrokeColor(color) => {
				self.options.stroke.custom_color = color;
				self.options.stroke.color_type = ToolColorType::Custom;
			}
			EllipseOptionsUpdate::StrokeColorType(color_type) => self.options.stroke.color_type = color_type,
			EllipseOptionsUpdate::WorkingColors(primary, secondary) => {
				self.options.stroke.primary_working_color = primary;
				self.options.stroke.secondary_working_color = secondary;
				self.options.fill.primary_working_color = primary;
				self.options.fill.secondary_working_color = secondary;
			}
		}

		self.send_layout(responses, LayoutTarget::ToolOptions);
	}

	fn actions(&self) -> ActionList {
		use EllipseToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(EllipseToolMessageDiscriminant;
				DragStart,
				PointerMove,
			),
			Drawing => actions!(EllipseToolMessageDiscriminant;
				DragStop,
				Abort,
				PointerMove,
			),
		}
	}
}

impl ToolTransition for EllipseTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			overlay_provider: Some(|overlay_context| EllipseToolMessage::Overlays(overlay_context).into()),
			tool_abort: Some(EllipseToolMessage::Abort.into()),
			working_color_changed: Some(EllipseToolMessage::WorkingColorChanged.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum EllipseToolFsmState {
	#[default]
	Ready,
	Drawing,
}

#[derive(Clone, Debug, Default)]
struct EllipseToolData {
	data: Resize,
}

impl Fsm for EllipseToolFsmState {
	type ToolData = EllipseToolData;
	type ToolOptions = EllipseToolOptions;

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document, global_tool_data, input, ..
		} = tool_action_data;

		let shape_data = &mut tool_data.data;

		let ToolMessage::Ellipse(event) = event else {
			return self;
		};
		match (self, event) {
			(_, EllipseToolMessage::Overlays(mut overlay_context)) => {
				shape_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);
				self
			}
			(EllipseToolFsmState::Ready, EllipseToolMessage::DragStart) => {
				shape_data.start(document, input);
				responses.add(DocumentMessage::StartTransaction);

				// Create a new ellipse vector shape
				let subpath = bezier_rs::Subpath::new_ellipse(DVec2::ZERO, DVec2::ONE);
				let manipulator_groups = subpath.manipulator_groups().to_vec();
				let layer = graph_modification_utils::new_vector_layer(vec![subpath], NodeId(generate_uuid()), document.new_layer_parent(), responses);
				graph_modification_utils::set_manipulator_mirror_angle(&manipulator_groups, layer, true, responses);
				shape_data.layer = Some(layer);

				let fill_color = tool_options.fill.active_color();
				responses.add(GraphOperationMessage::FillSet {
					layer,
					fill: if let Some(color) = fill_color { Fill::Solid(color) } else { Fill::None },
				});

				responses.add(GraphOperationMessage::StrokeSet {
					layer,
					stroke: Stroke::new(tool_options.stroke.active_color(), tool_options.line_weight),
				});

				EllipseToolFsmState::Drawing
			}
			(EllipseToolFsmState::Drawing, EllipseToolMessage::PointerMove { center, lock_ratio }) => {
				if let Some(message) = shape_data.calculate_transform(document, input, center, lock_ratio, false) {
					responses.add(message);
				}

				self
			}
			(_, EllipseToolMessage::PointerMove { .. }) => {
				shape_data.snap_manager.preview_draw(&SnapData::new(document, input), input.mouse.position);
				responses.add(OverlaysMessage::Draw);
				self
			}
			(EllipseToolFsmState::Drawing, EllipseToolMessage::DragStop) => {
				input.mouse.finish_transaction(shape_data.viewport_drag_start(document), responses);
				shape_data.cleanup(responses);

				EllipseToolFsmState::Ready
			}
			(EllipseToolFsmState::Drawing, EllipseToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				shape_data.cleanup(responses);

				EllipseToolFsmState::Ready
			}
			(_, EllipseToolMessage::WorkingColorChanged) => {
				responses.add(EllipseToolMessage::UpdateOptions(EllipseOptionsUpdate::WorkingColors(
					Some(global_tool_data.primary_color),
					Some(global_tool_data.secondary_color),
				)));
				self
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			EllipseToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Draw Ellipse"),
				HintInfo::keys([Key::Shift], "Constrain Circular").prepend_plus(),
				HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
			])]),
			EllipseToolFsmState::Drawing => HintData(vec![HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Circular"), HintInfo::keys([Key::Alt], "From Center")])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
	}
}
