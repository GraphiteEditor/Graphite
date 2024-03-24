use super::tool_prelude::*;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::resize::Resize;
use crate::messages::tool::common_functionality::snapping::SnapData;

use graph_craft::document::NodeId;
use graphene_core::uuid::generate_uuid;
use graphene_core::vector::style::{Fill, Stroke};
use graphene_core::Color;

#[derive(Default)]
pub struct PolygonTool {
	fsm_state: PolygonToolFsmState,
	tool_data: PolygonToolData,
	options: PolygonOptions,
}

pub struct PolygonOptions {
	line_weight: f64,
	fill: ToolColorOptions,
	stroke: ToolColorOptions,
	vertices: u32,
	polygon_type: PolygonType,
}

impl Default for PolygonOptions {
	fn default() -> Self {
		Self {
			vertices: 5,
			line_weight: 5.,
			fill: ToolColorOptions::new_secondary(),
			stroke: ToolColorOptions::new_primary(),
			polygon_type: PolygonType::Convex,
		}
	}
}

#[impl_message(Message, ToolMessage, Polygon)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum PolygonToolMessage {
	// Standard messages
	Overlays(OverlayContext),
	Abort,
	WorkingColorChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	PointerMove { center: Key, lock_ratio: Key },
	PointerOutsideViewport { center: Key, lock_ratio: Key },
	UpdateOptions(PolygonOptionsUpdate),
}

#[derive(PartialEq, Copy, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum PolygonType {
	Convex = 0,
	Star = 1,
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum PolygonOptionsUpdate {
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	LineWeight(f64),
	PolygonType(PolygonType),
	StrokeColor(Option<Color>),
	StrokeColorType(ToolColorType),
	Vertices(u32),
	WorkingColors(Option<Color>, Option<Color>),
}

impl ToolMetadata for PolygonTool {
	fn icon_name(&self) -> String {
		"VectorPolygonTool".into()
	}
	fn tooltip(&self) -> String {
		"Polygon Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Polygon
	}
}

fn create_sides_widget(vertices: u32) -> WidgetHolder {
	NumberInput::new(Some(vertices as f64))
		.label("Sides")
		.int()
		.min(3.)
		.max(1000.)
		.mode(NumberInputMode::Increment)
		.on_update(|number_input: &NumberInput| PolygonToolMessage::UpdateOptions(PolygonOptionsUpdate::Vertices(number_input.value.unwrap() as u32)).into())
		.widget_holder()
}

fn create_star_option_widget(polygon_type: PolygonType) -> WidgetHolder {
	let entries = vec![
		RadioEntryData::new("convex")
			.label("Convex")
			.on_update(move |_| PolygonToolMessage::UpdateOptions(PolygonOptionsUpdate::PolygonType(PolygonType::Convex)).into()),
		RadioEntryData::new("star")
			.label("Star")
			.on_update(move |_| PolygonToolMessage::UpdateOptions(PolygonOptionsUpdate::PolygonType(PolygonType::Star)).into()),
	];
	RadioInput::new(entries).selected_index(Some(polygon_type as u32)).widget_holder()
}

fn create_weight_widget(line_weight: f64) -> WidgetHolder {
	NumberInput::new(Some(line_weight))
		.unit(" px")
		.label("Weight")
		.min(0.)
		.max((1_u64 << std::f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| PolygonToolMessage::UpdateOptions(PolygonOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
		.widget_holder()
}

impl LayoutHolder for PolygonTool {
	fn layout(&self) -> Layout {
		let mut widgets = vec![
			create_star_option_widget(self.options.polygon_type),
			Separator::new(SeparatorType::Related).widget_holder(),
			create_sides_widget(self.options.vertices),
		];

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.fill.create_widgets(
			"Fill",
			true,
			|_| PolygonToolMessage::UpdateOptions(PolygonOptionsUpdate::FillColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| PolygonToolMessage::UpdateOptions(PolygonOptionsUpdate::FillColorType(color_type.clone())).into()),
			|color: &ColorButton| PolygonToolMessage::UpdateOptions(PolygonOptionsUpdate::FillColor(color.value)).into(),
		));

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| PolygonToolMessage::UpdateOptions(PolygonOptionsUpdate::StrokeColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| PolygonToolMessage::UpdateOptions(PolygonOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			|color: &ColorButton| PolygonToolMessage::UpdateOptions(PolygonOptionsUpdate::StrokeColor(color.value)).into(),
		));
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}
impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for PolygonTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let ToolMessage::Polygon(PolygonToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &self.options, responses, true);
			return;
		};
		match action {
			PolygonOptionsUpdate::Vertices(vertices) => self.options.vertices = vertices,
			PolygonOptionsUpdate::PolygonType(polygon_type) => self.options.polygon_type = polygon_type,
			PolygonOptionsUpdate::FillColor(color) => {
				self.options.fill.custom_color = color;
				self.options.fill.color_type = ToolColorType::Custom;
			}
			PolygonOptionsUpdate::FillColorType(color_type) => self.options.fill.color_type = color_type,
			PolygonOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			PolygonOptionsUpdate::StrokeColor(color) => {
				self.options.stroke.custom_color = color;
				self.options.stroke.color_type = ToolColorType::Custom;
			}
			PolygonOptionsUpdate::StrokeColorType(color_type) => self.options.stroke.color_type = color_type,
			PolygonOptionsUpdate::WorkingColors(primary, secondary) => {
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
			PolygonToolFsmState::Ready => actions!(PolygonToolMessageDiscriminant;
				DragStart,
				PointerMove,
			),
			PolygonToolFsmState::Drawing => actions!(PolygonToolMessageDiscriminant;
				DragStop,
				Abort,
				PointerMove,
			),
		}
	}
}

impl ToolTransition for PolygonTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			overlay_provider: Some(|overlay_context| PolygonToolMessage::Overlays(overlay_context).into()),
			tool_abort: Some(PolygonToolMessage::Abort.into()),
			working_color_changed: Some(PolygonToolMessage::WorkingColorChanged.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PolygonToolFsmState {
	#[default]
	Ready,
	Drawing,
}

#[derive(Clone, Debug, Default)]
struct PolygonToolData {
	data: Resize,
	auto_panning: AutoPanning,
}

impl Fsm for PolygonToolFsmState {
	type ToolData = PolygonToolData;
	type ToolOptions = PolygonOptions;

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document, global_tool_data, input, ..
		} = tool_action_data;

		let polygon_data = &mut tool_data.data;

		let ToolMessage::Polygon(event) = event else {
			return self;
		};
		match (self, event) {
			(_, PolygonToolMessage::Overlays(mut overlay_context)) => {
				polygon_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);
				self
			}
			(PolygonToolFsmState::Ready, PolygonToolMessage::DragStart) => {
				polygon_data.start(document, input);
				responses.add(DocumentMessage::StartTransaction);

				let subpath = match tool_options.polygon_type {
					PolygonType::Convex => bezier_rs::Subpath::new_regular_polygon(DVec2::ZERO, tool_options.vertices as u64, 1.),
					PolygonType::Star => bezier_rs::Subpath::new_star_polygon(DVec2::ZERO, tool_options.vertices as u64, 1., 0.5),
				};
				let layer = graph_modification_utils::new_vector_layer(vec![subpath], NodeId(generate_uuid()), document.new_layer_parent(), responses);
				polygon_data.layer = Some(layer);

				let fill_color = tool_options.fill.active_color();
				responses.add(GraphOperationMessage::FillSet {
					layer,
					fill: if let Some(color) = fill_color { Fill::Solid(color) } else { Fill::None },
				});

				responses.add(GraphOperationMessage::StrokeSet {
					layer,
					stroke: Stroke::new(tool_options.stroke.active_color(), tool_options.line_weight),
				});

				PolygonToolFsmState::Drawing
			}
			(PolygonToolFsmState::Drawing, PolygonToolMessage::PointerMove { center, lock_ratio }) => {
				if let Some(message) = polygon_data.calculate_transform(document, input, center, lock_ratio, false) {
					responses.add(message);
				}

				// Auto-panning
				let messages = [
					PolygonToolMessage::PointerOutsideViewport { center, lock_ratio }.into(),
					PolygonToolMessage::PointerMove { center, lock_ratio }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				self
			}
			(_, PolygonToolMessage::PointerMove { .. }) => {
				polygon_data.snap_manager.preview_draw(&SnapData::new(document, input), input.mouse.position);
				responses.add(OverlaysMessage::Draw);
				self
			}
			(PolygonToolFsmState::Drawing, PolygonToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				let _ = tool_data.auto_panning.shift_viewport(input, responses);

				PolygonToolFsmState::Drawing
			}
			(state, PolygonToolMessage::PointerOutsideViewport { center, lock_ratio }) => {
				// Auto-panning
				let messages = [
					PolygonToolMessage::PointerOutsideViewport { center, lock_ratio }.into(),
					PolygonToolMessage::PointerMove { center, lock_ratio }.into(),
				];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(PolygonToolFsmState::Drawing, PolygonToolMessage::DragStop) => {
				input.mouse.finish_transaction(polygon_data.viewport_drag_start(document), responses);
				polygon_data.cleanup(responses);

				PolygonToolFsmState::Ready
			}
			(PolygonToolFsmState::Drawing, PolygonToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);

				polygon_data.cleanup(responses);

				PolygonToolFsmState::Ready
			}
			(_, PolygonToolMessage::WorkingColorChanged) => {
				responses.add(PolygonToolMessage::UpdateOptions(PolygonOptionsUpdate::WorkingColors(
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
			PolygonToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Draw Polygon"),
				HintInfo::keys([Key::Shift], "Constrain Regular").prepend_plus(),
				HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
			])]),
			PolygonToolFsmState::Drawing => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Regular"), HintInfo::keys([Key::Alt], "From Center")]),
			]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
	}
}
