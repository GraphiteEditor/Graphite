use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::MouseMotion;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, WidgetCallback, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::layout::utility_types::widgets::input_widgets::NumberInput;
use crate::messages::portfolio::document::node_graph::transform_utils::get_current_transform;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use document_legacy::layers::layer_layer::CachedOutputData;
use document_legacy::LayerId;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput, NodeNetwork};
use graphene_core::raster::ImageFrame;
use graphene_core::vector::brush_stroke::{BrushInputSample, BrushStroke, BrushStyle};
use graphene_core::Color;

use glam::DAffine2;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct BrushTool {
	fsm_state: BrushToolFsmState,
	data: BrushToolData,
	options: BrushOptions,
}

pub struct BrushOptions {
	diameter: f64,
	hardness: f64,
	flow: f64,
	spacing: f64,
	color: ToolColorOptions,
}

impl Default for BrushOptions {
	fn default() -> Self {
		Self {
			diameter: 40.,
			hardness: 0.,
			flow: 100.,
			spacing: 20.,
			color: ToolColorOptions::default(),
		}
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Brush)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum BrushToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	WorkingColorChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	PointerMove,
	UpdateOptions(BrushToolMessageOptionsUpdate),
}

#[remain::sorted]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum BrushToolMessageOptionsUpdate {
	ChangeDiameter(f64),
	Color(Option<Color>),
	ColorType(ToolColorType),
	Diameter(f64),
	Flow(f64),
	Hardness(f64),
	Spacing(f64),
	WorkingColors(Option<Color>, Option<Color>),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BrushToolFsmState {
	#[default]
	Ready,
	Drawing,
}

impl ToolMetadata for BrushTool {
	fn icon_name(&self) -> String {
		"RasterBrushTool".into()
	}
	fn tooltip(&self) -> String {
		"Brush Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Brush
	}
}

impl PropertyHolder for BrushTool {
	fn properties(&self) -> Layout {
		let mut widgets = vec![
			NumberInput::new(Some(self.options.diameter))
				.label("Diameter")
				.min(1.)
				.unit(" px")
				.on_update(|number_input: &NumberInput| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::Diameter(number_input.value.unwrap())).into())
				.widget_holder(),
			WidgetHolder::related_separator(),
			NumberInput::new(Some(self.options.hardness))
				.label("Hardness")
				.min(0.)
				.max(100.)
				.unit("%")
				.on_update(|number_input: &NumberInput| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::Hardness(number_input.value.unwrap())).into())
				.widget_holder(),
			WidgetHolder::related_separator(),
			NumberInput::new(Some(self.options.flow))
				.label("Flow")
				.min(1.)
				.max(100.)
				.unit("%")
				.on_update(|number_input: &NumberInput| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::Flow(number_input.value.unwrap())).into())
				.widget_holder(),
			WidgetHolder::related_separator(),
			NumberInput::new(Some(self.options.spacing))
				.label("Spacing")
				.min(1.)
				.max(100.)
				.unit("%")
				.on_update(|number_input: &NumberInput| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::Spacing(number_input.value.unwrap())).into())
				.widget_holder(),
		];

		widgets.push(WidgetHolder::section_separator());

		widgets.append(&mut self.options.color.create_widgets(
			"Color",
			false,
			WidgetCallback::new(|_| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::Color(None)).into()),
			|color_type: ToolColorType| WidgetCallback::new(move |_| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::ColorType(color_type.clone())).into()),
			WidgetCallback::new(|color: &ColorInput| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::Color(color.value)).into()),
		));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for BrushTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		if let ToolMessage::Brush(BrushToolMessage::UpdateOptions(action)) = message {
			match action {
				BrushToolMessageOptionsUpdate::ChangeDiameter(change) => {
					let needs_rounding = ((self.options.diameter + change.abs() / 2.) % change.abs() - change.abs() / 2.).abs() > 0.5;
					if needs_rounding && change > 0. {
						self.options.diameter = (self.options.diameter / change.abs()).ceil() * change.abs();
					} else if needs_rounding && change < 0. {
						self.options.diameter = (self.options.diameter / change.abs()).floor() * change.abs();
					} else {
						self.options.diameter = (self.options.diameter / change.abs()).round() * change.abs() + change;
					}
					self.options.diameter = self.options.diameter.max(1.);
					self.register_properties(responses, LayoutTarget::ToolOptions);
				}
				BrushToolMessageOptionsUpdate::Diameter(diameter) => self.options.diameter = diameter,
				BrushToolMessageOptionsUpdate::Hardness(hardness) => self.options.hardness = hardness,
				BrushToolMessageOptionsUpdate::Flow(flow) => self.options.flow = flow,
				BrushToolMessageOptionsUpdate::Spacing(spacing) => self.options.spacing = spacing,
				BrushToolMessageOptionsUpdate::Color(color) => {
					self.options.color.custom_color = color;
					self.options.color.color_type = ToolColorType::Custom;
				}
				BrushToolMessageOptionsUpdate::ColorType(color_type) => self.options.color.color_type = color_type,
				BrushToolMessageOptionsUpdate::WorkingColors(primary, secondary) => {
					self.options.color.primary_working_color = primary;
					self.options.color.secondary_working_color = secondary;
				}
			}

			responses.add(LayoutMessage::SendLayout {
				layout: self.properties(),
				layout_target: LayoutTarget::ToolOptions,
			});

			return;
		}

		self.fsm_state.process_event(message, &mut self.data, tool_data, &self.options, responses, true);
	}

	fn actions(&self) -> ActionList {
		use BrushToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(BrushToolMessageDiscriminant;
				DragStart,
				DragStop,
				Abort,
				UpdateOptions,
			),
			Drawing => actions!(BrushToolMessageDiscriminant;
				DragStop,
				PointerMove,
				Abort,
				UpdateOptions,
			),
		}
	}
}

impl ToolTransition for BrushTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(BrushToolMessage::Abort.into()),
			working_color_changed: Some(BrushToolMessage::WorkingColorChanged.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Debug, Default)]
struct BrushToolData {
	strokes: Vec<BrushStroke>,
	layer_path: Vec<LayerId>,
	node_path: Vec<NodeId>,
	transform: DAffine2,
}

impl BrushToolData {
	fn load_existing_strokes(&mut self, document: &DocumentMessageHandler) -> Option<&Vec<LayerId>> {
		self.transform = DAffine2::IDENTITY;
		if document.selected_layers().count() != 1 {
			return None;
		}
		self.layer_path = document.selected_layers().next()?.to_vec();
		let layer = document.document_legacy.layer(&self.layer_path).ok().and_then(|layer| layer.as_layer().ok())?;
		let network = &layer.network;
		for (node, _node_id) in network.primary_flow() {
			if node.name == "Brush" {
				let points_input = node.inputs.get(3)?;
				let NodeInput::Value { tagged_value: TaggedValue::BrushStrokes(strokes), .. } = points_input else {
					continue;
				};
				self.strokes = strokes.clone();

				return Some(&self.layer_path);
			} else if node.name == "Transform" {
				self.transform = get_current_transform(&node.inputs) * self.transform;
			}
		}

		self.transform = DAffine2::IDENTITY;

		matches!(layer.cached_output_data, CachedOutputData::BlobURL(_) | CachedOutputData::SurfaceId(_)).then_some(&self.layer_path)
	}

	fn update_strokes(&self, brush_options: &BrushOptions, responses: &mut VecDeque<Message>) {
		let layer = self.layer_path.clone();
		let strokes = self.strokes.clone();
		responses.add(GraphOperationMessage::Brush { layer, strokes });
	}
}

impl Fsm for BrushToolFsmState {
	type ToolData = BrushToolData;
	type ToolOptions = BrushOptions;

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
		let document_position = (document.document_legacy.root.transform).inverse().transform_point2(input.mouse.position);
		let layer_position = tool_data.transform.inverse().transform_point2(document_position);

		if let ToolMessage::Brush(event) = event {
			match (self, event) {
				(BrushToolFsmState::Ready, BrushToolMessage::DragStart) => {
					responses.add(DocumentMessage::StartTransaction);
					let layer_path = tool_data.load_existing_strokes(document);
					let new_layer = layer_path.is_none();
					if new_layer {
						responses.add(DocumentMessage::DeselectAllLayers);
						tool_data.layer_path = document.get_path_for_new_layer();
					}
					let layer_position = tool_data.transform.inverse().transform_point2(document_position);
					// TODO: Also scale it based on the input image ('Background' parameter).
					// TODO: Resizing the input image results in a different brush size from the chosen diameter.
					let layer_scale = 0.0001_f64 // Safety against division by zero
						.max((tool_data.transform.matrix2 * glam::DVec2::X).length())
						.max((tool_data.transform.matrix2 * glam::DVec2::Y).length());

					// Start a new stroke with a single sample
					tool_data.strokes.push(BrushStroke {
						trace: vec![BrushInputSample { position: layer_position }],
						style: BrushStyle {
							color: tool_options.color.active_color().unwrap_or_default(),
							diameter: tool_options.diameter / layer_scale,
							hardness: tool_options.hardness,
							flow: tool_options.flow,
							spacing: tool_options.spacing,
						},
					});

					if new_layer {
						add_brush_render(tool_options, tool_data, responses);
					}
					tool_data.update_strokes(tool_options, responses);

					BrushToolFsmState::Drawing
				}

				(BrushToolFsmState::Drawing, BrushToolMessage::PointerMove) => {
					if let Some(stroke) = tool_data.strokes.last_mut() {
						stroke.trace.push(BrushInputSample { position: layer_position })
					}
					tool_data.update_strokes(tool_options, responses);

					BrushToolFsmState::Drawing
				}

				(BrushToolFsmState::Drawing, BrushToolMessage::DragStop) | (BrushToolFsmState::Drawing, BrushToolMessage::Abort) => {
					if !tool_data.strokes.is_empty() {
						responses.add(DocumentMessage::CommitTransaction);
					} else {
						responses.add(DocumentMessage::AbortTransaction);
					}

					tool_data.strokes.clear();

					BrushToolFsmState::Ready
				}

				(_, BrushToolMessage::WorkingColorChanged) => {
					responses.add(BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::WorkingColors(
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
			BrushToolFsmState::Ready => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Draw Stroke")])]),
			BrushToolFsmState::Drawing => HintData(vec![]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

fn add_brush_render(tool_options: &BrushOptions, data: &BrushToolData, responses: &mut VecDeque<Message>) {
	let mut network = NodeNetwork::default();
	let output_node = network.push_output_node();
	if let Some(node) = network.nodes.get_mut(&output_node) {
		node.inputs.push(NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true))
	}
	graph_modification_utils::new_custom_layer(network, data.layer_path.clone(), responses);
}
