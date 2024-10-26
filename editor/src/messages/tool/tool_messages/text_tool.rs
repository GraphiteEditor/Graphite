#![allow(clippy::too_many_arguments)]

use super::tool_prelude::*;
use crate::application::generate_uuid;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, is_layer_fed_by_node_of_name};

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graphene_core::renderer::Quad;
use graphene_core::text::{load_face, Font, FontCache};
use graphene_core::vector::style::Fill;
use graphene_core::Color;

#[derive(Default)]
pub struct TextTool {
	fsm_state: TextToolFsmState,
	tool_data: TextToolData,
	options: TextOptions,
}

pub struct TextOptions {
	font_size: f64,
	line_height_ratio: f64,
	character_spacing: f64,
	font_name: String,
	font_style: String,
	fill: ToolColorOptions,
}

impl Default for TextOptions {
	fn default() -> Self {
		Self {
			font_size: 24.,
			line_height_ratio: 1.2,
			character_spacing: 1.,
			font_name: graphene_core::consts::DEFAULT_FONT_FAMILY.into(),
			font_style: graphene_core::consts::DEFAULT_FONT_STYLE.into(),
			fill: ToolColorOptions::new_primary(),
		}
	}
}

#[impl_message(Message, ToolMessage, Text)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum TextToolMessage {
	// Standard messages
	Abort,
	WorkingColorChanged,
	Overlays(OverlayContext),

	// Tool-specific messages
	CommitText,
	EditSelected,
	Interact,
	TextChange { new_text: String },
	UpdateBounds { new_text: String },
	UpdateOptions(TextOptionsUpdate),
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum TextOptionsUpdate {
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	Font { family: String, style: String },
	FontSize(f64),
	LineHeightRatio(f64),
	CharacterSpacing(f64),
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
	let font = FontInput::new(&tool.options.font_name, &tool.options.font_style)
		.is_style_picker(false)
		.on_update(|font_input: &FontInput| {
			TextToolMessage::UpdateOptions(TextOptionsUpdate::Font {
				family: font_input.font_family.clone(),
				style: font_input.font_style.clone(),
			})
			.into()
		})
		.widget_holder();
	let style = FontInput::new(&tool.options.font_name, &tool.options.font_style)
		.is_style_picker(true)
		.on_update(|font_input: &FontInput| {
			TextToolMessage::UpdateOptions(TextOptionsUpdate::Font {
				family: font_input.font_family.clone(),
				style: font_input.font_style.clone(),
			})
			.into()
		})
		.widget_holder();
	let size = NumberInput::new(Some(tool.options.font_size))
		.unit(" px")
		.label("Size")
		.int()
		.min(1.)
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::FontSize(number_input.value.unwrap())).into())
		.widget_holder();
	let line_height_ratio = NumberInput::new(Some(tool.options.line_height_ratio))
		.label("Line Height")
		.int()
		.min(0.)
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		.step(0.1)
		.on_update(|number_input: &NumberInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::LineHeightRatio(number_input.value.unwrap())).into())
		.widget_holder();
	let character_spacing = NumberInput::new(Some(tool.options.character_spacing))
		.label("Character Spacing")
		.int()
		.min(0.)
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		.step(0.1)
		.on_update(|number_input: &NumberInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::CharacterSpacing(number_input.value.unwrap())).into())
		.widget_holder();
	vec![
		font,
		Separator::new(SeparatorType::Related).widget_holder(),
		style,
		Separator::new(SeparatorType::Related).widget_holder(),
		size,
		Separator::new(SeparatorType::Related).widget_holder(),
		line_height_ratio,
		Separator::new(SeparatorType::Related).widget_holder(),
		character_spacing,
	]
}

impl LayoutHolder for TextTool {
	fn layout(&self) -> Layout {
		let mut widgets = create_text_widgets(self);

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.fill.create_widgets(
			"Fill",
			true,
			|_| TextToolMessage::UpdateOptions(TextOptionsUpdate::FillColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| TextToolMessage::UpdateOptions(TextOptionsUpdate::FillColorType(color_type.clone())).into()),
			|color: &ColorButton| TextToolMessage::UpdateOptions(TextOptionsUpdate::FillColor(color.value.as_solid())).into(),
		));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for TextTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let ToolMessage::Text(TextToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &self.options, responses, true);
			return;
		};
		match action {
			TextOptionsUpdate::Font { family, style } => {
				self.options.font_name = family;
				self.options.font_style = style;

				self.send_layout(responses, LayoutTarget::ToolOptions);
			}
			TextOptionsUpdate::FontSize(font_size) => self.options.font_size = font_size,
			TextOptionsUpdate::LineHeightRatio(line_height_ratio) => self.options.line_height_ratio = line_height_ratio,
			TextOptionsUpdate::CharacterSpacing(character_spacing) => self.options.character_spacing = character_spacing,
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

		self.send_layout(responses, LayoutTarget::ToolOptions);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			TextToolFsmState::Ready => actions!(TextToolMessageDiscriminant;
				Interact,
			),
			TextToolFsmState::Editing => actions!(TextToolMessageDiscriminant;
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
			canvas_transformed: None,
			tool_abort: Some(TextToolMessage::Abort.into()),
			working_color_changed: Some(TextToolMessage::WorkingColorChanged.into()),
			overlay_provider: Some(|overlay_context| TextToolMessage::Overlays(overlay_context).into()),
			..Default::default()
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
	line_height_ratio: f64,
	character_spacing: f64,
	color: Option<Color>,
	transform: DAffine2,
}

#[derive(Clone, Debug, Default)]
struct TextToolData {
	layer: LayerNodeIdentifier,
	editing_text: Option<EditingText>,
	new_text: String,
}

impl TextToolData {
	/// Set the editing state of the currently modifying layer
	fn set_editing(&self, editable: bool, font_cache: &FontCache, responses: &mut VecDeque<Message>) {
		if let Some(editing_text) = self.editing_text.as_ref().filter(|_| editable) {
			responses.add(FrontendMessage::DisplayEditableTextbox {
				text: editing_text.text.clone(),
				line_width: None,
				font_size: editing_text.font_size,
				color: editing_text.color.unwrap_or(Color::BLACK),
				url: font_cache.get_preview_url(&editing_text.font).cloned().unwrap_or_default(),
				transform: editing_text.transform.to_cols_array(),
			});
		} else {
			responses.add(FrontendMessage::DisplayRemoveEditableTextbox);
			// Clear all selected nodes when no longer editing
			responses.add(NodeGraphMessage::SelectedNodesSet { nodes: Vec::new() });
		}
	}

	fn load_layer_text_node(&mut self, document: &DocumentMessageHandler) -> Option<()> {
		let transform = document.metadata().transform_to_viewport(self.layer);
		let color = graph_modification_utils::get_fill_color(self.layer, &document.network_interface).unwrap_or(Color::BLACK);
		let (text, font, font_size, line_height_ratio, character_spacing) = graph_modification_utils::get_text(self.layer, &document.network_interface)?;
		self.editing_text = Some(EditingText {
			text: text.clone(),
			font: font.clone(),
			font_size,
			line_height_ratio,
			character_spacing,
			color: Some(color),
			transform,
		});
		self.new_text.clone_from(text);
		Some(())
	}

	fn start_editing_layer(&mut self, layer: LayerNodeIdentifier, tool_state: TextToolFsmState, document: &DocumentMessageHandler, font_cache: &FontCache, responses: &mut VecDeque<Message>) {
		if layer == LayerNodeIdentifier::ROOT_PARENT {
			log::error!("Cannot edit ROOT_PARENT in TextTooLData")
		}

		if tool_state == TextToolFsmState::Editing {
			self.set_editing(false, font_cache, responses);
		}

		self.layer = layer;
		if self.load_layer_text_node(document).is_some() {
			responses.add(DocumentMessage::AddTransaction);

			self.set_editing(true, font_cache, responses);

			responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![self.layer.to_node()] });
			// Make the rendered text invisible while editing
			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(graph_modification_utils::get_text_id(self.layer, &document.network_interface).unwrap(), 1),
				input: NodeInput::value(TaggedValue::String("".to_string()), false),
			});
			responses.add(NodeGraphMessage::RunDocumentGraph);
		};
	}

	fn interact(
		&mut self,
		state: TextToolFsmState,
		input: &InputPreprocessorMessageHandler,
		document: &DocumentMessageHandler,
		font_cache: &FontCache,
		responses: &mut VecDeque<Message>,
	) -> TextToolFsmState {
		// Check if the user has selected an existing text layer
		if let Some(clicked_text_layer_path) = document.click(input).filter(|&layer| is_layer_fed_by_node_of_name(layer, &document.network_interface, "Text")) {
			self.start_editing_layer(clicked_text_layer_path, state, document, font_cache, responses);

			TextToolFsmState::Editing
		}
		// Create new text
		else if let Some(editing_text) = self.editing_text.as_ref().filter(|_| state == TextToolFsmState::Ready) {
			responses.add(DocumentMessage::AddTransaction);

			self.layer = LayerNodeIdentifier::new_unchecked(NodeId(generate_uuid()));

			responses.add(GraphOperationMessage::NewTextLayer {
				id: self.layer.to_node(),
				text: String::new(),
				font: editing_text.font.clone(),
				size: editing_text.font_size,
				line_height_ratio: editing_text.line_height_ratio,
				character_spacing: editing_text.character_spacing,
				parent: document.new_layer_parent(true),
				insert_index: 0,
			});
			responses.add(Message::StartBuffer);
			responses.add(GraphOperationMessage::FillSet {
				layer: self.layer,
				fill: if editing_text.color.is_some() { Fill::Solid(editing_text.color.unwrap()) } else { Fill::None },
			});
			responses.add(GraphOperationMessage::TransformSet {
				layer: self.layer,
				transform: editing_text.transform,
				transform_in: TransformIn::Viewport,
				skip_rerender: true,
			});

			self.set_editing(true, font_cache, responses);

			responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![self.layer.to_node()] });

			responses.add(NodeGraphMessage::RunDocumentGraph);
			TextToolFsmState::Editing
		} else {
			// Removing old text as editable
			self.set_editing(false, font_cache, responses);

			TextToolFsmState::Ready
		}
	}
}

fn can_edit_selected(document: &DocumentMessageHandler) -> Option<LayerNodeIdentifier> {
	let selected_nodes = document.network_interface.selected_nodes(&[]).unwrap();
	let mut selected_layers = selected_nodes.selected_layers(document.metadata());
	let layer = selected_layers.next()?;

	// Check that only one layer is selected
	if selected_layers.next().is_some() {
		return None;
	}

	if !is_layer_fed_by_node_of_name(layer, &document.network_interface, "Text") {
		return None;
	}

	Some(layer)
}

impl Fsm for TextToolFsmState {
	type ToolData = TextToolData;
	type ToolOptions = TextOptions;

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, transition_data: &mut ToolActionHandlerData, tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document,
			global_tool_data,
			input,
			font_cache,
			..
		} = transition_data;
		let ToolMessage::Text(event) = event else {
			return self;
		};
		match (self, event) {
			(TextToolFsmState::Editing, TextToolMessage::Overlays(mut overlay_context)) => {
				responses.add(FrontendMessage::DisplayEditableTextboxTransform {
					transform: document.metadata().transform_to_viewport(tool_data.layer).to_cols_array(),
				});
				if let Some(editing_text) = tool_data.editing_text.as_ref() {
					let buzz_face = font_cache.get(&editing_text.font).map(|data| load_face(data));
					let far = graphene_core::text::bounding_box(
						&tool_data.new_text,
						buzz_face,
						editing_text.font_size,
						editing_text.line_height_ratio,
						editing_text.character_spacing,
						None,
					);
					if far.x != 0. && far.y != 0. {
						let quad = Quad::from_box([DVec2::ZERO, far]);
						let transformed_quad = document.metadata().transform_to_viewport(tool_data.layer) * quad;
						overlay_context.quad(transformed_quad, None);
					}
				}

				TextToolFsmState::Editing
			}
			(_, TextToolMessage::Overlays(mut overlay_context)) => {
				for layer in document.network_interface.selected_nodes(&[]).unwrap().selected_layers(document.metadata()) {
					let Some((text, font, font_size, line_height_ratio, character_spacing)) = graph_modification_utils::get_text(layer, &document.network_interface) else {
						continue;
					};
					let buzz_face = font_cache.get(font).map(|data| load_face(data));
					let far = graphene_core::text::bounding_box(text, buzz_face, font_size, line_height_ratio, character_spacing, None);
					let quad = Quad::from_box([DVec2::ZERO, far]);
					let multiplied = document.metadata().transform_to_viewport(layer) * quad;
					overlay_context.quad(multiplied, None);
				}

				self
			}
			(state, TextToolMessage::Interact) => {
				tool_data.editing_text = Some(EditingText {
					text: String::new(),
					transform: DAffine2::from_translation(input.mouse.position),
					font_size: tool_options.font_size,
					line_height_ratio: tool_options.line_height_ratio,
					character_spacing: tool_options.character_spacing,
					font: Font::new(tool_options.font_name.clone(), tool_options.font_style.clone()),
					color: tool_options.fill.active_color(),
				});
				tool_data.new_text = String::new();

				tool_data.interact(state, input, document, font_cache, responses)
			}
			(state, TextToolMessage::EditSelected) => {
				if let Some(layer) = can_edit_selected(document) {
					tool_data.start_editing_layer(layer, state, document, font_cache, responses);
					return TextToolFsmState::Editing;
				}

				state
			}
			(state, TextToolMessage::Abort) => {
				if state == TextToolFsmState::Editing {
					tool_data.set_editing(false, font_cache, responses);
				}

				TextToolFsmState::Ready
			}
			(TextToolFsmState::Editing, TextToolMessage::CommitText) => {
				responses.add(FrontendMessage::TriggerTextCommit);

				TextToolFsmState::Editing
			}
			(TextToolFsmState::Editing, TextToolMessage::TextChange { new_text }) => {
				tool_data.set_editing(false, font_cache, responses);

				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(graph_modification_utils::get_text_id(tool_data.layer, &document.network_interface).unwrap(), 1),
					input: NodeInput::value(TaggedValue::String(new_text), false),
				});
				responses.add(NodeGraphMessage::RunDocumentGraph);

				TextToolFsmState::Ready
			}
			(TextToolFsmState::Editing, TextToolMessage::UpdateBounds { new_text }) => {
				tool_data.new_text = new_text;
				responses.add(OverlaysMessage::Draw);
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
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			TextToolFsmState::Ready => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Place Text")]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Edit Text")]),
			]),
			TextToolFsmState::Editing => HintData(vec![
				HintGroup(vec![HintInfo::keys([Key::Escape], "Commit Changes")]),
				HintGroup(vec![HintInfo::keys([Key::Control, Key::Enter], "Commit Changes").add_mac_keys([Key::Command, Key::Enter])]),
			]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Text });
	}
}
