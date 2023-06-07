use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, WidgetCallback, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widget_prelude::{ColorInput, NumberInput, WidgetHolder};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::resize::Resize;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use graphene_core::vector::style::{Fill, Stroke};
use graphene_core::Color;

use glam::DVec2;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct ShapeTool {
	fsm_state: ShapeToolFsmState,
	tool_data: ShapeToolData,
	options: ShapeOptions,
}

pub struct ShapeOptions {
	line_weight: f64,
	fill: ToolColorOptions,
	stroke: ToolColorOptions,
	vertices: u32,
}

impl Default for ShapeOptions {
	fn default() -> Self {
		Self {
			vertices: 5,
			line_weight: 5.,
			fill: ToolColorOptions::new_secondary(),
			stroke: ToolColorOptions::new_primary(),
		}
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Shape)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum ShapeToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	WorkingColorChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	Resize {
		center: Key,
		lock_ratio: Key,
	},
	UpdateOptions(ShapeOptionsUpdate),
}

#[remain::sorted]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum ShapeOptionsUpdate {
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	LineWeight(f64),
	StrokeColor(Option<Color>),
	StrokeColorType(ToolColorType),
	Vertices(u32),
	WorkingColors(Option<Color>, Option<Color>),
}

impl ToolMetadata for ShapeTool {
	fn icon_name(&self) -> String {
		"VectorShapeTool".into()
	}
	fn tooltip(&self) -> String {
		"Shape Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Shape
	}
}

fn create_sides_widget(vertices: u32) -> WidgetHolder {
	NumberInput::new(Some(vertices as f64))
		.label("Sides")
		.int()
		.min(3.)
		.max(1000.)
		.mode(crate::messages::layout::utility_types::widget_prelude::NumberInputMode::Increment)
		.on_update(|number_input: &NumberInput| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::Vertices(number_input.value.unwrap() as u32)).into())
		.widget_holder()
}

fn create_weight_widget(line_weight: f64) -> WidgetHolder {
	NumberInput::new(Some(line_weight))
		.unit(" px")
		.label("Weight")
		.min(0.)
		.on_update(|number_input: &NumberInput| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
		.widget_holder()
}

impl PropertyHolder for ShapeTool {
	fn properties(&self) -> Layout {
		let mut widgets = vec![create_sides_widget(self.options.vertices)];

		widgets.push(WidgetHolder::section_separator());

		widgets.append(&mut self.options.fill.create_widgets(
			"Fill",
			true,
			WidgetCallback::new(|_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::FillColor(None)).into()),
			|color_type: ToolColorType| WidgetCallback::new(move |_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::FillColorType(color_type.clone())).into()),
			WidgetCallback::new(|color: &ColorInput| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::FillColor(color.value)).into()),
		));

		widgets.push(WidgetHolder::section_separator());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			WidgetCallback::new(|_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::StrokeColor(None)).into()),
			|color_type: ToolColorType| WidgetCallback::new(move |_| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			WidgetCallback::new(|color: &ColorInput| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::StrokeColor(color.value)).into()),
		));
		widgets.push(WidgetHolder::unrelated_separator());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}
impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for ShapeTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		if let ToolMessage::Shape(ShapeToolMessage::UpdateOptions(action)) = message {
			match action {
				ShapeOptionsUpdate::Vertices(vertices) => self.options.vertices = vertices,
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

			responses.add(LayoutMessage::SendLayout {
				layout: self.properties(),
				layout_target: LayoutTarget::ToolOptions,
			});

			return;
		}

		self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &self.options, responses, true);
	}

	fn actions(&self) -> ActionList {
		use ShapeToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(ShapeToolMessageDiscriminant;
				DragStart,
			),
			Drawing => actions!(ShapeToolMessageDiscriminant;
				DragStop,
				Abort,
				Resize,
			),
		}
	}
}

impl ToolTransition for ShapeTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
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
struct ShapeToolData {
	data: Resize,
}

impl Fsm for ShapeToolFsmState {
	type ToolData = ShapeToolData;
	type ToolOptions = ShapeOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		ToolActionHandlerData {
			document,
			global_tool_data,
			input,
			render_data,
			..
		}: &mut ToolActionHandlerData,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use ShapeToolFsmState::*;
		use ShapeToolMessage::*;

		let mut shape_data = &mut tool_data.data;

		if let ToolMessage::Shape(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					shape_data.start(responses, document, input, render_data);
					responses.add(DocumentMessage::StartTransaction);
					let layer_path = document.get_path_for_new_layer();
					shape_data.path = Some(layer_path.clone());

					let subpath = bezier_rs::Subpath::new_regular_polygon(DVec2::ZERO, tool_options.vertices as u64, 1.);
					graph_modification_utils::new_vector_layer(vec![subpath], layer_path.clone(), responses);

					let fill_color = tool_options.fill.active_color();
					responses.add(GraphOperationMessage::FillSet {
						layer: layer_path.clone(),
						fill: if let Some(color) = fill_color { Fill::Solid(color) } else { Fill::None },
					});

					responses.add(GraphOperationMessage::StrokeSet {
						layer: layer_path,
						stroke: Stroke::new(tool_options.stroke.active_color(), tool_options.line_weight),
					});

					Drawing
				}
				(state, Resize { center, lock_ratio }) => {
					if let Some(message) = shape_data.calculate_transform(responses, document, input, center, lock_ratio, false) {
						responses.add(message);
					}

					state
				}
				(Drawing, DragStop) => {
					input.mouse.finish_transaction(shape_data.viewport_drag_start(document), responses);
					shape_data.cleanup(responses);

					Ready
				}
				(Drawing, Abort) => {
					responses.add(DocumentMessage::AbortTransaction);

					shape_data.cleanup(responses);

					Ready
				}
				(_, WorkingColorChanged) => {
					responses.add(ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::WorkingColors(
						Some(global_tool_data.primary_color),
						Some(global_tool_data.secondary_color),
					)));
					self
				}
				_ => self,
			}
		} else {
			self
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			ShapeToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Draw Shape"),
				HintInfo::keys([Key::Shift], "Constrain 1:1 Aspect").prepend_plus(),
				HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
			])]),
			ShapeToolFsmState::Drawing => HintData(vec![HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain 1:1 Aspect"), HintInfo::keys([Key::Alt], "From Center")])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
	}
}
