#![allow(clippy::too_many_arguments)]

use crate::application::generate_uuid;
use crate::consts::{COLOR_ACCENT, SELECTION_TOLERANCE};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widgets::input_widgets::{ColorInput, FontInput, NumberInput};
use crate::messages::portfolio::document::node_graph::new_text_network;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use document_legacy::intersection::Quad;
use document_legacy::layers::layer_info::Layer;
use document_legacy::layers::style::{self, Fill, RenderData, Stroke};
use document_legacy::LayerId;
use document_legacy::Operation;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, NodeId, NodeInput, NodeNetwork};
use graphene_core::text::{load_face, Font};
use graphene_core::Color;

use glam::{DAffine2, DVec2};
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
	fill: ToolColorOptions,
}

impl Default for TextOptions {
	fn default() -> Self {
		Self {
			font_size: 24,
			font_name: "Merriweather".into(),
			font_style: "Normal (400)".into(),
			fill: ToolColorOptions::new_primary(),
		}
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Text)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum TextToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	DocumentIsDirty,
	#[remain::unsorted]
	WorkingColorChanged,

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
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum TextOptionsUpdate {
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	Font { family: String, style: String },
	FontSize(u32),
	WorkingColors(Option<Color>, Option<Color>),
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

fn create_text_widgets(tool: &TextTool) -> Vec<WidgetHolder> {
	let font = FontInput {
		is_style_picker: false,
		font_family: tool.options.font_name.clone(),
		font_style: tool.options.font_style.clone(),
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
		font_family: tool.options.font_name.clone(),
		font_style: tool.options.font_style.clone(),
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
	let size = NumberInput::new(Some(tool.options.font_size as f64))
		.unit(" px")
		.label("Size")
		.int()
		.min(1.)
		.on_update(|number_input: &NumberInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::FontSize(number_input.value.unwrap() as u32)).into())
		.widget_holder();
	vec![font, WidgetHolder::related_separator(), style, WidgetHolder::related_separator(), size]
}

impl PropertyHolder for TextTool {
	fn properties(&self) -> Layout {
		let mut widgets = create_text_widgets(self);

		widgets.push(WidgetHolder::section_separator());

		widgets.append(&mut self.options.fill.create_widgets(
			"Fill",
			true,
			WidgetCallback::new(|_| TextToolMessage::UpdateOptions(TextOptionsUpdate::FillColor(None)).into()),
			|color_type: ToolColorType| WidgetCallback::new(move |_| TextToolMessage::UpdateOptions(TextOptionsUpdate::FillColorType(color_type.clone())).into()),
			WidgetCallback::new(|color: &ColorInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::FillColor(color.value)).into()),
		));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
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
				TextOptionsUpdate::FillColor(color) => {
					self.options.fill.custom_color = color;
					self.options.fill.color_type = ToolColorType::Custom;
				}
				TextOptionsUpdate::FillColorType(color_type) => self.options.fill.color_type = color_type,
				TextOptionsUpdate::WorkingColors(primary, secondary) => {
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
			selection_changed: Some(TextToolMessage::DocumentIsDirty.into()),
			working_color_changed: Some(TextToolMessage::WorkingColorChanged.into()),
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TextToolFsmState {
	#[default]
	Ready,
	Editing,
}
#[derive(Clone, Debug)]
pub struct EditingText {
	text: String,
	font: Font,
	font_size: f64,
	color: Option<Color>,
	transform: DAffine2,
}

#[derive(Clone, Debug, Default)]
struct TextToolData {
	layer_path: Vec<LayerId>,
	overlays: Vec<Vec<LayerId>>,
	editing_text: Option<EditingText>,
	new_text: String,
}

impl TextToolData {
	/// Set the editing state of the currently modifying layer
	fn set_editing(&self, editable: bool, render_data: &RenderData, responses: &mut VecDeque<Message>) {
		let path = self.layer_path.clone();
		responses.add(Operation::SetLayerVisibility { path, visible: !editable });

		if let Some(editing_text) = self.editing_text.as_ref().filter(|_| editable) {
			responses.add(FrontendMessage::DisplayEditableTextbox {
				text: editing_text.text.clone(),
				line_width: None,
				font_size: editing_text.font_size,
				color: editing_text.color.unwrap_or(Color::BLACK),
				url: render_data.font_cache.get_preview_url(&editing_text.font).cloned().unwrap_or_default(),
				transform: editing_text.transform.to_cols_array(),
			});
		} else {
			responses.add(FrontendMessage::DisplayRemoveEditableTextbox);
		}
	}

	fn load_layer_text_node(&mut self, document: &DocumentMessageHandler) -> Option<()> {
		let transform = document.document_legacy.multiply_transforms(&self.layer_path).ok()?;
		let layer = document.document_legacy.layer(&self.layer_path).ok()?;
		let color = layer
			.style()
			.ok()
			.map_or(Color::BLACK, |style| if let Fill::Solid(solid_color) = style.fill() { *solid_color } else { Color::BLACK });

		let network = get_network(&self.layer_path, document)?;
		let node_id = get_text_node_id(network)?;
		let node = network.nodes.get(&node_id)?;

		let (text, font, font_size) = Self::extract_text_node_inputs(node)?;
		self.editing_text = Some(EditingText {
			text: text.clone(),
			font: font.clone(),
			font_size,
			color: Some(color),
			transform,
		});
		self.new_text = text.clone();
		Some(())
	}

	fn start_editing_layer(&mut self, layer_path: &[LayerId], tool_state: TextToolFsmState, document: &DocumentMessageHandler, render_data: &RenderData, responses: &mut VecDeque<Message>) {
		if tool_state == TextToolFsmState::Editing {
			self.set_editing(false, render_data, responses);
		}

		self.layer_path = layer_path.into();
		self.load_layer_text_node(document);

		responses.add(DocumentMessage::StartTransaction);

		self.set_editing(true, render_data, responses);

		let replacement_selected_layers = vec![self.layer_path.clone()];
		responses.add(DocumentMessage::SetSelectedLayers { replacement_selected_layers });
	}

	fn extract_text_node_inputs(node: &DocumentNode) -> Option<(&String, &Font, f64)> {
		let NodeInput::Value { tagged_value: TaggedValue::String(text), .. } = &node.inputs[1] else { return None; };
		let NodeInput::Value { tagged_value: TaggedValue::Font(font), .. } = &node.inputs[2] else { return None; };
		let NodeInput::Value { tagged_value: TaggedValue::F64(font_size), .. } = &node.inputs[3] else { return None; };
		Some((text, font, *font_size))
	}

	fn interact(&mut self, state: TextToolFsmState, mouse: DVec2, document: &DocumentMessageHandler, render_data: &RenderData, responses: &mut VecDeque<Message>) -> TextToolFsmState {
		let tolerance = DVec2::splat(SELECTION_TOLERANCE);
		let quad = Quad::from_box([mouse - tolerance, mouse + tolerance]);

		// Check if the user has selected an existing text layer
		if let Some(clicked_text_layer_path) = document.document_legacy.intersects_quad_root(quad, render_data).last().filter(|l| is_text_layer(document, l)) {
			self.start_editing_layer(clicked_text_layer_path, state, document, render_data, responses);

			TextToolFsmState::Editing
		}
		// Create new text
		else if let Some(editing_text) = self.editing_text.as_ref().filter(|_| state == TextToolFsmState::Ready) {
			responses.add(DocumentMessage::StartTransaction);

			let network = new_text_network(String::new(), editing_text.font.clone(), editing_text.font_size as f32);

			responses.add(Operation::AddFrame {
				path: self.layer_path.clone(),
				insert_index: -1,
				transform: DAffine2::ZERO.to_cols_array(),
				network,
			});
			responses.add(GraphOperationMessage::FillSet {
				layer: self.layer_path.clone(),
				fill: if editing_text.color.is_some() { Fill::Solid(editing_text.color.unwrap()) } else { Fill::None },
			});
			responses.add(GraphOperationMessage::TransformSet {
				layer: self.layer_path.clone(),
				transform: editing_text.transform,
				transform_in: TransformIn::Viewport,
				skip_rerender: true,
			});

			self.set_editing(true, render_data, responses);

			let replacement_selected_layers = vec![self.layer_path.clone()];

			responses.add(DocumentMessage::SetSelectedLayers { replacement_selected_layers });

			TextToolFsmState::Editing
		} else {
			// Removing old text as editable
			self.set_editing(false, render_data, responses);

			resize_overlays(&mut self.overlays, responses, 0);

			TextToolFsmState::Ready
		}
	}

	pub fn update_bounds_overlay(&mut self, document: &DocumentMessageHandler, render_data: &RenderData, responses: &mut VecDeque<Message>) -> Option<()> {
		resize_overlays(&mut self.overlays, responses, 1);

		let editing_text = self.editing_text.as_ref()?;
		let buzz_face = render_data.font_cache.get(&editing_text.font).map(|data| load_face(data));
		let far = graphene_core::text::bounding_box(&self.new_text, buzz_face, editing_text.font_size, None);
		let quad = Quad::from_box([DVec2::ZERO, far]);

		let transformed_quad = document.document_legacy.multiply_transforms(&self.layer_path).ok()? * quad;
		let bounds = transformed_quad.bounding_box();

		let operation = Operation::SetLayerTransformInViewport {
			path: self.overlays[0].clone(),
			transform: transform_from_box(bounds[0], bounds[1]),
		};
		responses.add(DocumentMessage::Overlays(operation.into()));
		Some(())
	}

	fn get_bounds(&self, text: &str, render_data: &RenderData) -> Option<[DVec2; 2]> {
		let editing_text = self.editing_text.as_ref()?;
		let buzz_face = render_data.font_cache.get(&editing_text.font).map(|data| load_face(data));
		let subpaths = graphene_core::text::to_path(text, buzz_face, editing_text.font_size, None);
		let bounds = subpaths.iter().filter_map(|subpath| subpath.bounding_box());
		let combined_bounds = bounds.reduce(|a, b| [a[0].min(b[0]), a[1].max(b[1])]).unwrap_or_default();
		Some(combined_bounds)
	}

	fn fix_text_bounds(&self, new_text: &str, _document: &DocumentMessageHandler, render_data: &RenderData, responses: &mut VecDeque<Message>) -> Option<()> {
		let layer = self.layer_path.clone();
		let old_bounds = self.get_bounds(&self.editing_text.as_ref()?.text, render_data)?;
		let new_bounds = self.get_bounds(new_text, render_data)?;
		responses.add(GraphOperationMessage::UpdateBounds { layer, old_bounds, new_bounds });

		Some(())
	}
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
			style: style::PathStyle::new(Some(Stroke::new(Some(COLOR_ACCENT), 1.0)), Fill::None),
			insert_index: -1,
		};
		responses.add(DocumentMessage::Overlays(operation.into()));
	}
}

fn update_overlays(document: &DocumentMessageHandler, tool_data: &mut TextToolData, responses: &mut VecDeque<Message>, render_data: &RenderData) {
	let get_bounds = |layer: &Layer, path: &[LayerId], document: &DocumentMessageHandler, render_data: &RenderData| {
		let node_graph = layer.as_layer_network().ok()?;
		let node_id = get_text_node_id(node_graph)?;
		let document_node = node_graph.nodes.get(&node_id)?;
		let (text, font, font_size) = TextToolData::extract_text_node_inputs(document_node)?;
		let buzz_face = render_data.font_cache.get(font).map(|data| load_face(data));
		let far = graphene_core::text::bounding_box(text, buzz_face, font_size, None);
		let quad = Quad::from_box([DVec2::ZERO, far]);
		let multiplied = document.document_legacy.multiply_transforms(path).ok()? * quad;
		Some(multiplied.bounding_box())
	};
	let bounds = document.selected_layers().filter_map(|path| match document.document_legacy.layer(path) {
		Ok(layer) => get_bounds(layer, path, document, render_data),
		Err(_) => None,
	});
	let bounds = bounds.collect::<Vec<_>>();

	let new_len = bounds.len();

	for (bounds, overlay_path) in bounds.iter().zip(&tool_data.overlays) {
		let operation = Operation::SetLayerTransformInViewport {
			path: overlay_path.to_vec(),
			transform: transform_from_box(bounds[0], bounds[1]),
		};
		responses.add(DocumentMessage::Overlays(operation.into()));
	}
	resize_overlays(&mut tool_data.overlays, responses, new_len);
}

fn get_network<'a>(layer_path: &[LayerId], document: &'a DocumentMessageHandler) -> Option<&'a NodeNetwork> {
	let layer = document.document_legacy.layer(layer_path).ok()?;
	layer.as_layer_network().ok()
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

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, transition_data: &mut ToolActionHandlerData, tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document,
			global_tool_data,
			input,
			render_data,
			..
		} = transition_data;
		if let ToolMessage::Text(event) = event {
			match (self, event) {
				(TextToolFsmState::Editing, TextToolMessage::DocumentIsDirty) => {
					responses.add(FrontendMessage::DisplayEditableTextboxTransform {
						transform: document.document_legacy.multiply_transforms(&tool_data.layer_path).ok().unwrap_or_default().to_cols_array(),
					});
					tool_data.update_bounds_overlay(document, render_data, responses);
					TextToolFsmState::Editing
				}
				(state, TextToolMessage::DocumentIsDirty) => {
					update_overlays(document, tool_data, responses, render_data);

					state
				}
				(state, TextToolMessage::Interact) => {
					tool_data.editing_text = Some(EditingText {
						text: String::new(),
						transform: DAffine2::from_translation(input.mouse.position),
						font_size: tool_options.font_size as f64,
						font: Font::new(tool_options.font_name.clone(), tool_options.font_style.clone()),
						color: tool_options.fill.active_color(),
					});
					tool_data.new_text = String::new();
					tool_data.layer_path = document.get_path_for_new_layer();

					tool_data.interact(state, input.mouse.position, document, render_data, responses)
				}
				(state, TextToolMessage::EditSelected) => {
					if let Some(layer_path) = can_edit_selected(document) {
						tool_data.start_editing_layer(&layer_path, state, document, render_data, responses);
						return TextToolFsmState::Editing;
					}

					state
				}
				(state, TextToolMessage::Abort) => {
					if state == TextToolFsmState::Editing {
						tool_data.set_editing(false, render_data, responses);
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
					tool_data.fix_text_bounds(&new_text, document, render_data, responses);
					responses.add(NodeGraphMessage::SetQualifiedInputValue {
						layer_path,
						node_path: vec![get_text_node_id(network).unwrap()],
						input_index: 1,
						value: TaggedValue::String(new_text),
					});

					tool_data.set_editing(false, render_data, responses);

					resize_overlays(&mut tool_data.overlays, responses, 0);

					TextToolFsmState::Ready
				}
				(TextToolFsmState::Editing, TextToolMessage::UpdateBounds { new_text }) => {
					tool_data.new_text = new_text;
					tool_data.update_bounds_overlay(document, render_data, responses);
					TextToolFsmState::Editing
				}
				(_, TextToolMessage::WorkingColorChanged) => {
					responses.add(TextToolMessage::UpdateOptions(TextOptionsUpdate::WorkingColors(
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
