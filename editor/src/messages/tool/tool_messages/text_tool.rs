use crate::application::generate_uuid;
use crate::consts::{COLOR_ACCENT, SELECTION_TOLERANCE};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widgets::input_widgets::{FontInput, NumberInput};
use crate::messages::portfolio::document::node_graph::new_text_network;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use document_legacy::intersection::Quad;
use document_legacy::layers::style::{self, Fill, RenderData, Stroke};
use document_legacy::LayerId;
use document_legacy::Operation;

use glam::{DAffine2, DVec2};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeInput, NodeNetwork};
use graph_craft::NodeIdentifier;
use graphene_core::text::{load_face, Font};
use graphene_core::Color;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct TextTool {
	fsm_state: TextToolFsmState,
	tool_data: TextToolData,
	options: TextOptions,
}

pub struct TextOptions {
	font_size: u32,
	font_name: String,
	font_style: String,
}

impl Default for TextOptions {
	fn default() -> Self {
		Self {
			font_size: 24,
			font_name: "Merriweather".into(),
			font_style: "Normal (400)".into(),
		}
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Text)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum TextToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

	#[remain::unsorted]
	DocumentIsDirty,

	// Tool-specific messages
	CommitText,
	EditSelected,
	Interact,
	TextChange {
		new_text: String,
	},
	UpdateBounds {
		new_text: String,
	},
	UpdateOptions(TextOptionsUpdate),
}

#[remain::sorted]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum TextOptionsUpdate {
	Font { family: String, style: String },
	FontSize(u32),
}

impl ToolMetadata for TextTool {
	fn icon_name(&self) -> String {
		"VectorTextTool".into()
	}
	fn tooltip(&self) -> String {
		"Text Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Text
	}
}

impl PropertyHolder for TextTool {
	fn properties(&self) -> Layout {
		let font = FontInput {
			is_style_picker: false,
			font_family: self.options.font_name.clone(),
			font_style: self.options.font_style.clone(),
			on_update: WidgetCallback::new(|font_input: &FontInput| {
				TextToolMessage::UpdateOptions(TextOptionsUpdate::Font {
					family: font_input.font_family.clone(),
					style: font_input.font_style.clone(),
				})
				.into()
			}),
			..Default::default()
		}
		.widget_holder();
		let style = FontInput {
			is_style_picker: true,
			font_family: self.options.font_name.clone(),
			font_style: self.options.font_style.clone(),
			on_update: WidgetCallback::new(|font_input: &FontInput| {
				TextToolMessage::UpdateOptions(TextOptionsUpdate::Font {
					family: font_input.font_family.clone(),
					style: font_input.font_style.clone(),
				})
				.into()
			}),
			..Default::default()
		}
		.widget_holder();
		let size = NumberInput::new(Some(self.options.font_size as f64))
			.unit(" px")
			.label("Size")
			.int()
			.min(1.)
			.on_update(|number_input: &NumberInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::FontSize(number_input.value.unwrap() as u32)).into())
			.widget_holder();
		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row {
			widgets: vec![font, WidgetHolder::related_separator(), style, WidgetHolder::related_separator(), size],
		}]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for TextTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		if let ToolMessage::Text(TextToolMessage::UpdateOptions(action)) = message {
			match action {
				TextOptionsUpdate::Font { family, style } => {
					self.options.font_name = family;
					self.options.font_style = style;

					self.register_properties(responses, LayoutTarget::ToolOptions);
				}
				TextOptionsUpdate::FontSize(font_size) => self.options.font_size = font_size,
			}
			return;
		}

		self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &self.options, responses, true);
	}

	fn actions(&self) -> ActionList {
		use TextToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(TextToolMessageDiscriminant;
				Interact,
			),
			Editing => actions!(TextToolMessageDiscriminant;
				Interact,
				Abort,
				CommitText,
			),
		}
	}
}

impl ToolTransition for TextTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			document_dirty: Some(TextToolMessage::DocumentIsDirty.into()),
			tool_abort: Some(TextToolMessage::Abort.into()),
			selection_changed: None,
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TextToolFsmState {
	#[default]
	Ready,
	Editing,
}
#[derive(Clone, Debug, Default)]
struct TextToolData {
	layer_path: Vec<LayerId>,
	overlays: Vec<Vec<LayerId>>,
}

impl TextToolData {
	/// Set the editing state of the currently modifying layer
	fn set_editing(&self, editable: bool, document: &DocumentMessageHandler, render_data: &RenderData, responses: &mut VecDeque<Message>) {
		let path = self.layer_path.clone();
		responses.add(Operation::SetLayerVisibility { path, visible: !editable });

		if editable {
			if let Some(frontend_message) = self.generate_front(document, render_data, responses) {
				responses.add(frontend_message);
			}
		} else {
			responses.add(FrontendMessage::DisplayRemoveEditableTextbox);
		}
	}
	fn generate_front(&self, document: &DocumentMessageHandler, render_data: &RenderData, responses: &mut VecDeque<Message>) -> Option<FrontendMessage> {
		let transform = document.document_legacy.multiply_transforms(&self.layer_path).ok()?;
		let layer = document.document_legacy.layer(&self.layer_path).ok()?;
		let color = layer
			.style()
			.ok()
			.map_or(Color::BLACK, |style| if let Fill::Solid(solid_color) = style.fill() { *solid_color } else { Color::BLACK });

		let network = get_network(&self.layer_path, document)?;
		let node_id = get_text_node_id(network)?;
		let node = network.nodes.get(&node_id)?;

		let (text, font, font_size) = extract_props(node)?;

		Some(FrontendMessage::DisplayEditableTextbox {
			text: text.clone(),
			line_width: None,
			font_size,
			color,
			url: render_data.font_cache.get_preview_url(font).cloned().unwrap_or_default(),
			transform: transform.to_cols_array(),
		})
	}
}

fn extract_props(node: &DocumentNode) -> Option<(&String, &Font, f64)> {
	let NodeInput::Value{tagged_value:TaggedValue::String(text),..} =& node.inputs[0] else { return None; };
	let NodeInput::Value{tagged_value:TaggedValue::Font(font),..} =& node.inputs[1] else { return None; };
	let NodeInput::Value{tagged_value:TaggedValue::F64(font_size),..} =& node.inputs[2] else { return None; };
	Some((text, font, *font_size))
}

fn transform_from_box(pos1: DVec2, pos2: DVec2) -> [f64; 6] {
	DAffine2::from_scale_angle_translation((pos2 - pos1).round(), 0., pos1.round() - DVec2::splat(0.5)).to_cols_array()
}

fn resize_overlays(overlays: &mut Vec<Vec<LayerId>>, responses: &mut VecDeque<Message>, newlen: usize) {
	while overlays.len() > newlen {
		let operation = Operation::DeleteLayer { path: overlays.pop().unwrap() };
		responses.add(DocumentMessage::Overlays(operation.into()));
	}
	while overlays.len() < newlen {
		let path = vec![generate_uuid()];
		overlays.push(path.clone());

		let operation = Operation::AddRect {
			path,
			transform: DAffine2::ZERO.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Fill::None),
			insert_index: -1,
		};
		responses.add(DocumentMessage::Overlays(operation.into()));
	}
}

fn update_overlays(document: &DocumentMessageHandler, tool_data: &mut TextToolData, responses: &mut VecDeque<Message>, render_data: &RenderData) {
	let visible_text_layers = document.selected_visible_text_layers().collect::<Vec<_>>();
	resize_overlays(&mut tool_data.overlays, responses, visible_text_layers.len());

	let bounds = visible_text_layers
		.into_iter()
		.zip(&tool_data.overlays)
		.filter_map(|(layer_path, overlay_path)| {
			document
				.document_legacy
				.layer(layer_path)
				.unwrap()
				.aabb_for_transform(document.document_legacy.multiply_transforms(layer_path).unwrap(), render_data)
				.map(|bounds| (bounds, overlay_path))
		})
		.collect::<Vec<_>>();

	let new_len = bounds.len();

	for (bounds, overlay_path) in bounds {
		let operation = Operation::SetLayerTransformInViewport {
			path: overlay_path.to_vec(),
			transform: transform_from_box(bounds[0], bounds[1]),
		};
		responses.add(DocumentMessage::Overlays(operation.into()));
	}
	resize_overlays(&mut tool_data.overlays, responses, new_len);
}

fn set_edit_layer(layer_path: &[LayerId], tool_state: TextToolFsmState, tool_data: &mut TextToolData, document: &DocumentMessageHandler, render_data: &RenderData, responses: &mut VecDeque<Message>) {
	if tool_state == TextToolFsmState::Editing {
		tool_data.set_editing(false, document, render_data, responses);
	}

	tool_data.layer_path = layer_path.into();

	responses.add(DocumentMessage::StartTransaction);

	tool_data.set_editing(true, document, render_data, responses);

	let replacement_selected_layers = vec![tool_data.layer_path.clone()];
	responses.add(DocumentMessage::SetSelectedLayers { replacement_selected_layers });
}

fn get_network<'a>(layer_path: &[LayerId], document: &'a DocumentMessageHandler) -> Option<&'a NodeNetwork> {
	let layer = document.document_legacy.layer(layer_path).ok()?;
	layer.as_node_graph().ok()
}

fn get_text_node_id(network: &NodeNetwork) -> Option<NodeId> {
	network.nodes.iter().find(|(_, node)| node.name == "Text").map(|(&id, _)| id)
}
fn is_text_layer(document: &DocumentMessageHandler, layer_path: &[LayerId]) -> bool {
	let Some(network) = get_network(layer_path, document) else { return false; };
	get_text_node_id(network).is_some()
}

fn can_edit_selected(document: &DocumentMessageHandler) -> Option<Vec<LayerId>> {
	let mut selected_layers = document.selected_layers();

	let layer_path = selected_layers.next()?.to_vec();
	// Check that only one layer is selected
	if selected_layers.next().is_some() {
		return None;
	}

	if !is_text_layer(document, &layer_path) {
		return None;
	}

	Some(layer_path)
}

impl Fsm for TextToolFsmState {
	type ToolData = TextToolData;
	type ToolOptions = TextOptions;

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
		if let ToolMessage::Text(event) = event {
			match (self, event) {
				(state, TextToolMessage::DocumentIsDirty) => {
					update_overlays(document, tool_data, responses, render_data);

					state
				}
				(state, TextToolMessage::Interact) => {
					let mouse_pos = input.mouse.position;
					let tolerance = DVec2::splat(SELECTION_TOLERANCE);
					let quad = Quad::from_box([mouse_pos - tolerance, mouse_pos + tolerance]);

					// Check if the user has selected an existing text layer
					let new_state = if let Some(clicked_text_layer_path) = document.document_legacy.intersects_quad_root(quad, render_data).last().filter(|l| is_text_layer(document, l)) {
						set_edit_layer(clicked_text_layer_path, state, tool_data, document, render_data, responses);

						TextToolFsmState::Editing
					}
					// Create new text
					else if state == TextToolFsmState::Ready {
						responses.add(DocumentMessage::StartTransaction);

						let transform = DAffine2::from_translation(input.mouse.position);
						let font_size = tool_options.font_size;
						let font_name = tool_options.font_name.clone();
						let font_style = tool_options.font_style.clone();
						tool_data.layer_path = document.get_path_for_new_layer();

						let font = Font::new(font_name, font_style);
						let network = new_text_network("hello".to_string(), font.clone(), font_size as f64);
						responses.add(Operation::AddNodeGraphFrame {
							path: tool_data.layer_path.clone(),
							insert_index: -1,
							transform: DAffine2::ZERO.to_cols_array(),
							network,
						});
						responses.add(GraphOperationMessage::FillSet {
							layer: tool_data.layer_path.clone(),
							fill: Fill::solid(global_tool_data.primary_color),
						});
						responses.add(GraphOperationMessage::TransformSet {
							layer: tool_data.layer_path.clone(),
							transform,
							transform_in: TransformIn::Viewport,
							skip_rerender: true,
						});

						responses.add(FrontendMessage::DisplayEditableTextbox {
							text: String::new(),
							line_width: None,
							font_size: font_size as f64,
							color: global_tool_data.primary_color,
							url: render_data.font_cache.get_preview_url(&font).cloned().unwrap_or_default(),
							transform: transform.to_cols_array(),
						});

						let replacement_selected_layers = vec![tool_data.layer_path.clone()];

						responses.add(DocumentMessage::SetSelectedLayers { replacement_selected_layers });

						TextToolFsmState::Editing
					} else {
						// Removing old text as editable
						tool_data.set_editing(false, document, render_data, responses);

						resize_overlays(&mut tool_data.overlays, responses, 0);

						TextToolFsmState::Ready
					};

					new_state
				}
				(state, TextToolMessage::EditSelected) => {
					if let Some(layer_path) = can_edit_selected(document) {
						set_edit_layer(&layer_path, state, tool_data, document, render_data, responses);
						tool_data.layer_path = layer_path;
						return TextToolFsmState::Editing;
					}

					state
				}
				(state, TextToolMessage::Abort) => {
					if state == TextToolFsmState::Editing {
						tool_data.set_editing(false, document, render_data, responses);
					}

					resize_overlays(&mut tool_data.overlays, responses, 0);

					TextToolFsmState::Ready
				}
				(TextToolFsmState::Editing, TextToolMessage::CommitText) => {
					responses.add(FrontendMessage::TriggerTextCommit);

					TextToolFsmState::Editing
				}
				(TextToolFsmState::Editing, TextToolMessage::TextChange { new_text }) => {
					let layer_path = tool_data.layer_path.clone();
					let network = get_network(&layer_path, document).unwrap();
					responses.add(NodeGraphMessage::SetQualifiedInputValue {
						layer_path,
						node_path: vec![get_text_node_id(network).unwrap()],
						input_index: 0,
						value: TaggedValue::String(new_text),
					});

					tool_data.set_editing(false, document, render_data, responses);

					resize_overlays(&mut tool_data.overlays, responses, 0);

					TextToolFsmState::Ready
				}
				(TextToolFsmState::Editing, TextToolMessage::UpdateBounds { new_text }) => {
					resize_overlays(&mut tool_data.overlays, responses, 1);
					let network = get_network(&tool_data.layer_path, document).unwrap();
					let node_id = get_text_node_id(network).unwrap();
					let node = network.nodes.get(&node_id).unwrap();
					let (_text, font, font_size) = extract_props(node).unwrap();

					let buzz_face = render_data.font_cache.get(font).map(|data| load_face(&data));
					let far = graphene_core::text::bounding_box(&new_text, buzz_face, font_size, None);
					let quad = Quad::from_box([DVec2::ZERO, far]);

					let transformed_quad = document.document_legacy.multiply_transforms(&tool_data.layer_path).unwrap() * quad;
					let bounds = transformed_quad.bounding_box();

					let operation = Operation::SetLayerTransformInViewport {
						path: tool_data.overlays[0].clone(),
						transform: transform_from_box(bounds[0], bounds[1]),
					};
					responses.add(DocumentMessage::Overlays(operation.into()));

					TextToolFsmState::Editing
				}
				_ => self,
			}
		} else {
			self
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			TextToolFsmState::Ready => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Add Text"), HintInfo::mouse(MouseMotion::Lmb, "Edit Text")])]),
			TextToolFsmState::Editing => HintData(vec![HintGroup(vec![
				HintInfo::keys([Key::Control, Key::Enter], "Commit Edit").add_mac_keys([Key::Command, Key::Enter]),
				HintInfo::keys([Key::Escape], "Discard Edit"),
			])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Text });
	}
}
