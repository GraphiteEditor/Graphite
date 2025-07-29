use super::tool_prelude::*;
use crate::consts::DEFAULT_BRUSH_SIZE;
use crate::messages::portfolio::document::graph_operation::transform_utils::get_current_transform;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::FlowType;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use graph_craft::document::NodeId;
use graph_craft::document::value::TaggedValue;
use graphene_std::Color;
use graphene_std::brush::brush_stroke::{BrushInputSample, BrushStroke, BrushStyle};
use graphene_std::raster::BlendMode;

const BRUSH_MAX_SIZE: f64 = 5000.;

#[derive(PartialEq, Copy, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum DrawMode {
	Draw = 0,
	Erase,
	Restore,
}

#[derive(Default, ExtractField)]
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
	blend_mode: BlendMode,
	draw_mode: DrawMode,
}

impl Default for BrushOptions {
	fn default() -> Self {
		Self {
			diameter: DEFAULT_BRUSH_SIZE,
			hardness: 0.,
			flow: 100.,
			spacing: 20.,
			color: ToolColorOptions::default(),
			blend_mode: BlendMode::Normal,
			draw_mode: DrawMode::Draw,
		}
	}
}

#[impl_message(Message, ToolMessage, Brush)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum BrushToolMessage {
	// Standard messages
	Abort,
	WorkingColorChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	PointerMove,
	UpdateOptions(BrushToolMessageOptionsUpdate),
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum BrushToolMessageOptionsUpdate {
	BlendMode(BlendMode),
	ChangeDiameter(f64),
	Color(Option<Color>),
	ColorType(ToolColorType),
	Diameter(f64),
	DrawMode(DrawMode),
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

impl LayoutHolder for BrushTool {
	fn layout(&self) -> Layout {
		let mut widgets = vec![
			NumberInput::new(Some(self.options.diameter))
				.label("Diameter")
				.min(1.)
				.max(BRUSH_MAX_SIZE) /* Anything bigger would cause the application to be unresponsive and eventually die */
				.unit(" px")
				.on_update(|number_input: &NumberInput| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::Diameter(number_input.value.unwrap())).into())
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			NumberInput::new(Some(self.options.hardness))
				.label("Hardness")
				.min(0.)
				.max(100.)
				.mode_range()
				.unit("%")
				.on_update(|number_input: &NumberInput| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::Hardness(number_input.value.unwrap())).into())
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			NumberInput::new(Some(self.options.flow))
				.label("Flow")
				.min(1.)
				.max(100.)
				.mode_range()
				.unit("%")
				.on_update(|number_input: &NumberInput| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::Flow(number_input.value.unwrap())).into())
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			NumberInput::new(Some(self.options.spacing))
				.label("Spacing")
				.min(1.)
				.max(100.)
				.mode_range()
				.unit("%")
				.on_update(|number_input: &NumberInput| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::Spacing(number_input.value.unwrap())).into())
				.widget_holder(),
		];

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		let draw_mode_entries: Vec<_> = [DrawMode::Draw, DrawMode::Erase, DrawMode::Restore]
			.into_iter()
			.map(|draw_mode| {
				RadioEntryData::new(format!("{draw_mode:?}"))
					.label(format!("{draw_mode:?}"))
					.on_update(move |_| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::DrawMode(draw_mode)).into())
			})
			.collect();
		widgets.push(RadioInput::new(draw_mode_entries).selected_index(Some(self.options.draw_mode as u32)).widget_holder());

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.color.create_widgets(
			"Color",
			false,
			|_| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::Color(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::ColorType(color_type.clone())).into()),
			|color: &ColorInput| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::Color(color.value.as_solid().map(|color| color.to_linear_srgb()))).into(),
		));

		widgets.push(Separator::new(SeparatorType::Related).widget_holder());

		let blend_mode_entries: Vec<Vec<_>> = BlendMode::list()
			.iter()
			.map(|group| {
				group
					.iter()
					.map(|blend_mode| {
						MenuListEntry::new(format!("{blend_mode:?}"))
							.label(blend_mode.to_string())
							.on_commit(|_| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::BlendMode(*blend_mode)).into())
					})
					.collect()
			})
			.collect();
		widgets.push(
			DropdownInput::new(blend_mode_entries)
				.selected_index(self.options.blend_mode.index_in_list().map(|index| index as u32))
				.tooltip("The blend mode used with the background when performing a brush stroke. Only used in draw mode.")
				.disabled(self.options.draw_mode != DrawMode::Draw)
				.widget_holder(),
		);

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for BrushTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		let ToolMessage::Brush(BrushToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.data, context, &self.options, responses, true);
			return;
		};
		match action {
			BrushToolMessageOptionsUpdate::BlendMode(blend_mode) => self.options.blend_mode = blend_mode,
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
				self.send_layout(responses, LayoutTarget::ToolOptions);
			}
			BrushToolMessageOptionsUpdate::Diameter(diameter) => self.options.diameter = diameter,
			BrushToolMessageOptionsUpdate::DrawMode(draw_mode) => self.options.draw_mode = draw_mode,
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

		self.send_layout(responses, LayoutTarget::ToolOptions);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			BrushToolFsmState::Ready => actions!(BrushToolMessageDiscriminant;
				DragStart,
				DragStop,
				UpdateOptions,
			),
			BrushToolFsmState::Drawing => actions!(BrushToolMessageDiscriminant;
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
	layer: Option<LayerNodeIdentifier>,
	transform: DAffine2,
}

impl BrushToolData {
	fn load_existing_strokes(&mut self, document: &DocumentMessageHandler) -> Option<LayerNodeIdentifier> {
		self.transform = DAffine2::IDENTITY;

		if document.network_interface.selected_nodes().selected_layers(document.metadata()).count() != 1 {
			return None;
		}
		let layer = document.network_interface.selected_nodes().selected_layers(document.metadata()).next()?;

		self.layer = Some(layer);
		for node_id in document.network_interface.upstream_flow_back_from_nodes(vec![layer.to_node()], &[], FlowType::HorizontalFlow) {
			let Some(node) = document.network_interface.document_network().nodes.get(&node_id) else {
				continue;
			};
			let Some(reference) = document.network_interface.reference(&node_id, &[]) else {
				continue;
			};

			if *reference == Some("Brush".to_string()) && node_id != layer.to_node() {
				let points_input = node.inputs.get(1)?;
				let Some(TaggedValue::BrushStrokes(strokes)) = points_input.as_value() else { continue };
				self.strokes.clone_from(strokes);

				return Some(layer);
			}

			if *reference == Some("Transform".to_string()) {
				self.transform = get_current_transform(&node.inputs) * self.transform;
			}
		}

		self.transform = DAffine2::IDENTITY;
		None
	}

	fn update_strokes(&self, responses: &mut VecDeque<Message>) {
		let Some(layer) = self.layer else { return };
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
		tool_action_data: &mut ToolActionMessageContext,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let ToolActionMessageContext {
			document, global_tool_data, input, ..
		} = tool_action_data;

		let ToolMessage::Brush(event) = event else { return self };
		match (self, event) {
			(BrushToolFsmState::Ready, BrushToolMessage::DragStart) => {
				responses.add(DocumentMessage::StartTransaction);
				let loaded_layer = tool_data.load_existing_strokes(document);

				if let Some(layer) = loaded_layer {
					let pos = document
						.network_interface
						.document_metadata()
						.downstream_transform_to_viewport(layer)
						.inverse()
						.transform_point2(input.mouse.position);
					let layer_position = tool_data.transform.inverse().transform_point2(pos);
					let layer_document_scale = document.metadata().downstream_transform_to_viewport(layer) * tool_data.transform;

					// TODO: Also scale it based on the input image ('Background' input).
					// TODO: Resizing the input image results in a different brush size from the chosen diameter.
					let layer_scale = 0.0001_f64 // Safety against division by zero
						.max((layer_document_scale.matrix2 * glam::DVec2::X).length())
						.max((layer_document_scale.matrix2 * glam::DVec2::Y).length());

					// Start a new stroke with a single sample
					let blend_mode = match tool_options.draw_mode {
						DrawMode::Draw => tool_options.blend_mode,
						DrawMode::Erase => BlendMode::Erase,
						DrawMode::Restore => BlendMode::Restore,
					};
					tool_data.strokes.push(BrushStroke {
						trace: vec![BrushInputSample { position: layer_position }],
						style: BrushStyle {
							color: tool_options.color.active_color().unwrap_or_default(),
							diameter: tool_options.diameter / layer_scale,
							hardness: tool_options.hardness,
							flow: tool_options.flow,
							spacing: tool_options.spacing,
							blend_mode,
						},
					});

					tool_data.update_strokes(responses);
					BrushToolFsmState::Drawing
				}
				// Create the new layer, wait for the render output to return its transform, and then create the rest of the layer
				else {
					new_brush_layer(document, responses);
					responses.add(NodeGraphMessage::RunDocumentGraph);
					responses.add(DeferMessage::AfterGraphRun {
						messages: vec![BrushToolMessage::DragStart.into()],
					});
					BrushToolFsmState::Ready
				}
			}

			(BrushToolFsmState::Drawing, BrushToolMessage::PointerMove) => {
				if let Some(layer) = tool_data.layer {
					if let Some(stroke) = tool_data.strokes.last_mut() {
						let layer_position = document
							.network_interface
							.document_metadata()
							.downstream_transform_to_viewport(layer)
							.inverse()
							.transform_point2(input.mouse.position);
						let layer_position = tool_data.transform.inverse().transform_point2(layer_position);

						stroke.trace.push(BrushInputSample { position: layer_position })
					}
				}
				tool_data.update_strokes(responses);

				BrushToolFsmState::Drawing
			}

			(BrushToolFsmState::Drawing, BrushToolMessage::DragStop) => {
				if !tool_data.strokes.is_empty() {
					responses.add(DocumentMessage::EndTransaction);
				} else {
					responses.add(DocumentMessage::AbortTransaction);
				}
				tool_data.strokes.clear();

				BrushToolFsmState::Ready
			}
			(BrushToolFsmState::Drawing, BrushToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
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
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			BrushToolFsmState::Ready => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Draw")]),
				HintGroup(vec![HintInfo::multi_keys([[Key::BracketLeft], [Key::BracketRight]], "Shrink/Grow Brush")]),
			]),
			BrushToolFsmState::Drawing => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

fn new_brush_layer(document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) -> LayerNodeIdentifier {
	responses.add(DocumentMessage::DeselectAllLayers);

	let brush_node = resolve_document_node_type("Brush").expect("Brush node does not exist").default_node_template();

	let id = NodeId::new();
	responses.add(GraphOperationMessage::NewCustomLayer {
		id,
		nodes: vec![(NodeId(0), brush_node)],
		parent: document.new_layer_parent(true),
		insert_index: 0,
	});
	responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![id] });

	LayerNodeIdentifier::new_unchecked(id)
}
