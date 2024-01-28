#![allow(clippy::too_many_arguments)]

use std::sync::Arc;

use super::tool_prelude::*;
use crate::application::generate_uuid;
use crate::consts::{DEFAULT_FONT_FAMILY, DEFAULT_FONT_STYLE, SELECTION_TOLERANCE};
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, is_layer_fed_by_node_of_name};

use glam::Vec2;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::NodeId;
use graphene_core::text::cosmic_text::Edit;
use graphene_core::text::{Font, FontCache, RichText, TextSpan};
use graphene_core::vector::style::Fill;
use graphene_core::vector::VectorData;
use graphene_core::Color;

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
	bold: bool,
	bold_size: f32,
	italic: bool,
	italic_size: f32,
	letter_spacing: f32,
	word_spacing: f32,
	line_spacing: f32,
	kerning: Vec2,
	fill: ToolColorOptions,
}

impl Default for TextOptions {
	fn default() -> Self {
		Self {
			font_size: 24,
			font_name: DEFAULT_FONT_FAMILY.into(),
			font_style: DEFAULT_FONT_STYLE.into(),
			bold: false,
			bold_size: 10.,
			italic: false,
			italic_size: 15.,
			letter_spacing: 0.,
			word_spacing: 0.,
			line_spacing: 1.,
			kerning: Vec2::ZERO,
			fill: ToolColorOptions::new_primary(),
		}
	}
}

impl TextOptions {
	pub fn to_span(&self) -> TextSpan {
		TextSpan {
			font: Arc::new(Font::new(self.font_name.clone(), self.font_style.clone())),
			font_size: self.font_size as f32,
			bold: self.bold.then_some(self.bold_size),
			italic: self.italic.then_some(self.italic_size),
			letter_spacing: self.letter_spacing,
			word_spacing: self.word_spacing,
			line_spacing: self.line_spacing,
			color: self.fill.active_color().unwrap_or(Color::BLACK),
			kerning: self.kerning,
			offset: 0,
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
	Drag,
	EditSelected,
	Interact,
	RefreshFonts,
	Select,
	TextInput { input_type: String, data: Option<String> },
	TextNavigate { key: String, shift: bool, ctrl: bool },
	UpdateOptions(TextOptionsUpdate),
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum TextOptionsUpdate {
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	Font { family: String, style: String },
	FontSize(u32),
	Bold(bool),
	Italic(bool),
	BoldSize(f32),
	ItalicSize(f32),
	LetterSpacing(f32),
	WordSpacing(f32),
	LineSpacing(f32),
	KerningX(f32),
	KerningY(f32),
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

fn get_cursor_index(buffer: &graphene_core::text::cosmic_text::Buffer, cursor: graphene_core::text::cosmic_text::Cursor) -> usize {
	let mut index = 0;
	for line in buffer.lines.iter().take(cursor.line) {
		index += line.text().len() + 1;
	}
	index + cursor.index
}

fn create_text_widgets(tool: &TextTool) -> Vec<WidgetHolder> {
	let span = tool.tool_data.editing_text.as_ref().and_then(|editing| {
		let mut offset = 0;
		let cursor = get_cursor_index(editing.editor.buffer(), editing.editor.cursor());
		editing
			.text
			.spans
			.iter()
			.filter(|span| {
				offset += span.offset;
				offset <= cursor
			})
			.last()
	});
	let family = span.map_or(&tool.options.font_name, |span| &span.font.font_family);
	let style = span.map_or(&tool.options.font_style, |span| &span.font.font_style);
	let font = FontInput::new(family, style)
		.is_style_picker(false)
		.on_update(|font_input: &FontInput| {
			TextToolMessage::UpdateOptions(TextOptionsUpdate::Font {
				family: font_input.font_family.clone(),
				style: font_input.font_style.clone(),
			})
			.into()
		})
		.widget_holder();
	let style = FontInput::new(family, style)
		.is_style_picker(true)
		.on_update(|font_input: &FontInput| {
			TextToolMessage::UpdateOptions(TextOptionsUpdate::Font {
				family: font_input.font_family.clone(),
				style: font_input.font_style.clone(),
			})
			.into()
		})
		.widget_holder();
	let bold = CheckboxInput::new(span.map_or(tool.options.bold, |span| span.bold.is_some()))
		.icon("Bold")
		.on_update(|input: &CheckboxInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::Bold(input.checked)).into());
	let italic = CheckboxInput::new(span.map_or(tool.options.italic, |span| span.italic.is_some()))
		.icon("Italic")
		.on_update(|input: &CheckboxInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::Italic(input.checked)).into());
	let size = NumberInput::new(Some(span.map_or(tool.options.font_size as f64, |span| span.font_size as f64)))
		.unit(" px")
		.label("Size")
		.int()
		.min(1.)
		.max((1_u64 << std::f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::FontSize(number_input.value.unwrap() as u32)).into())
		.widget_holder();

	vec![
		font,
		Separator::new(SeparatorType::Related).widget_holder(),
		style,
		Separator::new(SeparatorType::Related).widget_holder(),
		bold.widget_holder(),
		italic.widget_holder(),
		bold_italic_options(tool, span).widget_holder(),
		Separator::new(SeparatorType::Related).widget_holder(),
		size,
		spacing_options(tool, span).widget_holder(),
	]
}

fn bold_italic_options(tool: &TextTool, span: Option<&TextSpan>) -> PopoverButton {
	PopoverButton::new("Bold and Italic", "Bold and italic customization settings").options_widget(vec![
		LayoutGroup::Row {
			widgets: vec![
				TextLabel::new("Boldness").table_align(true).widget_holder(),
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				NumberInput::new(Some(span.map(|span| span.bold).flatten().unwrap_or(tool.options.bold_size) as f64))
					.min(0.)
					.on_update(|number_input: &NumberInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::BoldSize(number_input.value.unwrap() as f32)).into())
					.widget_holder(),
			],
		},
		LayoutGroup::Row {
			widgets: vec![
				TextLabel::new("Italic slant").table_align(true).widget_holder(),
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				NumberInput::new(Some(span.map(|span| span.italic).flatten().unwrap_or(tool.options.italic_size) as f64))
					.min(0.)
					.on_update(|number_input: &NumberInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::ItalicSize(number_input.value.unwrap() as f32)).into())
					.widget_holder(),
			],
		},
	])
}

fn spacing_options(tool: &TextTool, span: Option<&TextSpan>) -> PopoverButton {
	PopoverButton::new("Text Spacing", "Text spacing customization settings").options_widget(vec![
		LayoutGroup::Row {
			widgets: vec![
				TextLabel::new("Letter spacing").table_align(true).widget_holder(),
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				NumberInput::new(Some(span.map_or(tool.options.letter_spacing, |span| span.letter_spacing) as f64))
					.on_update(|number_input: &NumberInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::LetterSpacing(number_input.value.unwrap() as f32)).into())
					.widget_holder(),
			],
		},
		LayoutGroup::Row {
			widgets: vec![
				TextLabel::new("Word spacing").table_align(true).widget_holder(),
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				NumberInput::new(Some(span.map_or(tool.options.word_spacing, |span| span.word_spacing) as f64))
					.on_update(|number_input: &NumberInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::WordSpacing(number_input.value.unwrap() as f32)).into())
					.widget_holder(),
			],
		},
		LayoutGroup::Row {
			widgets: vec![
				TextLabel::new("Line spacing").table_align(true).widget_holder(),
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				NumberInput::new(Some(span.map_or(tool.options.line_spacing, |span| span.line_spacing) as f64))
					.on_update(|number_input: &NumberInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::LineSpacing(number_input.value.unwrap() as f32)).into())
					.widget_holder(),
			],
		},
		LayoutGroup::Row {
			widgets: vec![
				TextLabel::new("Kerning").table_align(true).widget_holder(),
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				NumberInput::new(Some(span.map_or(tool.options.kerning, |span| span.kerning).x as f64))
					.on_update(|number_input: &NumberInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::KerningX(number_input.value.unwrap() as f32)).into())
					.label("X")
					.widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				NumberInput::new(Some(span.map_or(tool.options.kerning, |span| span.kerning).y as f64))
					.on_update(|number_input: &NumberInput| TextToolMessage::UpdateOptions(TextOptionsUpdate::KerningY(number_input.value.unwrap() as f32)).into())
					.label("Y")
					.widget_holder(),
			],
		},
	])
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
			|color: &ColorButton| TextToolMessage::UpdateOptions(TextOptionsUpdate::FillColor(color.value)).into(),
		));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

fn update_span(tool_data: &mut TextToolData, font_cache: &FontCache, modification: impl Fn(&mut TextSpan)) {
	let Some(editing_text) = &mut tool_data.editing_text else { return };
	let editor = &mut editing_text.editor;
	let selection = editor.select_opt().filter(|&select| select != editor.cursor());
	let (start_index, end_index) = if let Some(selection) = selection {
		let cursor = get_cursor_index(editor.buffer(), editor.cursor());
		let selection = get_cursor_index(editor.buffer(), selection);
		let (start, end) = (cursor.min(selection), cursor.max(selection));

		let mut text_index = editing_text.text.spans.first().map_or(0, |span| span.offset);
		let mut span_index = 0;
		while editing_text.text.spans.get(span_index + 1).map_or(false, |next| text_index + next.offset < start) {
			span_index += 1;
			text_index += editing_text.text.spans[span_index].offset;
		}
		if !editing_text.text.spans.get(span_index + 1).map_or(false, |next| next.offset == start - text_index) {
			if let Some(next) = editing_text.text.spans.get_mut(span_index + 1) {
				next.offset -= start - text_index;
			}
			editing_text.text.spans.insert(span_index + 1, editing_text.text.spans[span_index].clone().offset(start - text_index));
		}
		let start_span = span_index + 1;

		while editing_text.text.spans.get(span_index + 1).map_or(false, |next| text_index + next.offset < end) {
			span_index += 1;
			text_index += editing_text.text.spans[span_index].offset;
		}
		if !editing_text.text.spans.get(span_index + 1).map_or(false, |next| next.offset == end - text_index) {
			if let Some(next) = editing_text.text.spans.get_mut(span_index + 1) {
				next.offset -= end - text_index;
			}
			editing_text.text.spans.insert(span_index + 1, editing_text.text.spans[span_index].clone().offset(end - text_index));
		}

		let end_span = span_index + 1;
		(start_span, end_span)
	} else {
		(0, editing_text.text.spans.len())
	};
	for span in &mut editing_text.text.spans[start_index..end_index] {
		modification(span);
	}
	if let Some(mut font_system) = font_cache.get_system() {
		graphene_core::text::create_buffer(editor.buffer_mut(), &mut font_system, &editing_text.text, font_cache, editing_text.line_length);
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
				self.options.font_name = family.clone();
				self.options.font_style = style.clone();
				let font = Arc::new(Font::new(family, style));
				update_span(&mut self.tool_data, tool_data.font_cache, |span| span.font = font.clone());
			}
			TextOptionsUpdate::FontSize(font_size) => {
				self.options.font_size = font_size;
				update_span(&mut self.tool_data, tool_data.font_cache, |span| span.font_size = font_size as f32);
			}
			TextOptionsUpdate::FillColor(color) => {
				self.options.fill.custom_color = color;
				self.options.fill.color_type = ToolColorType::Custom;
				update_span(&mut self.tool_data, tool_data.font_cache, |span| span.color = color.unwrap_or(Color::BLACK));
			}
			TextOptionsUpdate::FillColorType(color_type) => self.options.fill.color_type = color_type,
			TextOptionsUpdate::WorkingColors(primary, secondary) => {
				self.options.fill.primary_working_color = primary;
				self.options.fill.secondary_working_color = secondary;
			}
			TextOptionsUpdate::Bold(value) => {
				self.options.bold = value;
				update_span(&mut self.tool_data, tool_data.font_cache, |span| span.bold = self.options.bold.then_some(self.options.bold_size));
			}
			TextOptionsUpdate::Italic(value) => {
				self.options.italic = value;
				update_span(&mut self.tool_data, tool_data.font_cache, |span| span.italic = self.options.italic.then_some(self.options.italic_size));
			}
			TextOptionsUpdate::BoldSize(value) => {
				self.options.bold_size = value;
				update_span(&mut self.tool_data, tool_data.font_cache, |span| span.bold = self.options.bold.then_some(self.options.bold_size));
			}
			TextOptionsUpdate::ItalicSize(value) => {
				self.options.italic_size = value;
				update_span(&mut self.tool_data, tool_data.font_cache, |span| span.italic = self.options.italic.then_some(self.options.italic_size));
			}
			TextOptionsUpdate::LetterSpacing(value) => {
				self.options.letter_spacing = value;
				update_span(&mut self.tool_data, tool_data.font_cache, |span| span.letter_spacing = value);
			}
			TextOptionsUpdate::WordSpacing(value) => {
				self.options.word_spacing = value;
				update_span(&mut self.tool_data, tool_data.font_cache, |span| span.word_spacing = value);
			}
			TextOptionsUpdate::LineSpacing(value) => {
				self.options.line_spacing = value;
				update_span(&mut self.tool_data, tool_data.font_cache, |span| span.line_spacing = value);
			}
			TextOptionsUpdate::KerningX(value) => {
				self.options.kerning.x = value;
				update_span(&mut self.tool_data, tool_data.font_cache, |span| span.kerning.x = value);
			}
			TextOptionsUpdate::KerningY(value) => {
				self.options.kerning.y = value;
				update_span(&mut self.tool_data, tool_data.font_cache, |span| span.kerning.y = value);
			}
		}

		responses.add(OverlaysMessage::Draw);
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
				Select
			),
			TextToolFsmState::Selecting | TextToolFsmState::Wrap => actions!(TextToolMessageDiscriminant;
				Interact,
				Drag,
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
	Selecting,
	Wrap,
}
#[derive(Debug)]
pub struct EditingText {
	text: RichText,
	line_length: f64,
	path: VectorData,
	color: Option<Color>,
	transform: DAffine2,
	editor: graphene_core::text::cosmic_text::Editor,
	composition: Option<usize>,
}

#[derive(Debug, Default)]
struct TextToolData {
	layer: LayerNodeIdentifier,
	editing_text: Option<EditingText>,
	new_text: String,
}

impl TextToolData {
	/// Set the editing state of the currently modifying layer
	fn set_editing(&mut self, editable: bool, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let fill_node = graph_modification_utils::get_fill_id(self.layer, &document.network);
		if let Some(node_id) = fill_node {
			responses.add(NodeGraphMessage::SetVisibility { node_id, visible: !editable });
		}

		if let Some(editing_text) = self.editing_text.as_ref().filter(|_| !editable) {
			responses.add(GraphOperationMessage::FillSet {
				layer: self.layer,
				fill: editing_text.color.map_or(Fill::None, |color| Fill::Solid(color)),
			});
		}

		if let Some(editing_text) = self.editing_text.as_mut().filter(|_| editable) {
			responses.add(FrontendMessage::DisplayEditableTextbox {
				text: editing_text.text.text.clone(),
				transform: editing_text.transform.to_cols_array(),
			});
			use graphene_core::text::cosmic_text::{Affinity, Cursor};
			if let Some(last) = editing_text.editor.buffer().lines.last() {
				let last_index = last.text().len();
				let line = editing_text.editor.buffer().lines.len() - 1;
				editing_text.editor.set_select_opt(Some(Cursor::new_with_affinity(0, 0, Affinity::Before)));
				editing_text.editor.set_cursor(Cursor::new_with_affinity(line, last_index, Affinity::After));
				responses.add(OverlaysMessage::Draw);
			}
			responses.add(OverlaysMessage::Draw);
		} else {
			responses.add(FrontendMessage::DisplayRemoveEditableTextbox);
		}
		responses.add(ToolMessage::RefreshToolOptions);
	}

	fn load_layer_text_node(&mut self, document: &DocumentMessageHandler, font_cache: &FontCache) -> Option<()> {
		let transform = document.metadata().transform_to_viewport(self.layer);
		let color = Some(graph_modification_utils::get_fill_color(self.layer, &document.network).unwrap_or(Color::BLACK));
		let (text, line_length, path) = graph_modification_utils::get_text(self.layer, &document.network)?;
		let editor = graphene_core::text::create_cosmic_editor(&text, font_cache, line_length)?;
		self.editing_text = Some(EditingText {
			text: text.clone(),
			line_length,
			path: path.clone(),
			transform,
			color,
			editor,
			composition: None,
		});
		Some(())
	}

	fn start_editing_layer(&mut self, layer: LayerNodeIdentifier, tool_state: TextToolFsmState, document: &DocumentMessageHandler, font_cache: &FontCache, responses: &mut VecDeque<Message>) {
		if tool_state != TextToolFsmState::Ready {
			self.set_editing(false, document, responses);
		}

		self.layer = layer;
		self.load_layer_text_node(document, font_cache);

		responses.add(DocumentMessage::StartTransaction);

		self.set_editing(true, document, responses);

		responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![self.layer.to_node()] });
	}

	fn interact(&mut self, state: TextToolFsmState, mouse: DVec2, document: &DocumentMessageHandler, font_cache: &FontCache, responses: &mut VecDeque<Message>) -> TextToolFsmState {
		// Check if the user has selected an existing text layer
		if let Some(clicked_text_layer_path) = document
			.click(mouse, document.network())
			.filter(|&layer| is_layer_fed_by_node_of_name(layer, &document.network, "Text"))
		{
			self.start_editing_layer(clicked_text_layer_path, state, document, font_cache, responses);

			TextToolFsmState::Editing
		}
		// Create new text
		else if let Some(editing_text) = self.editing_text.as_ref().filter(|_| state == TextToolFsmState::Ready) {
			responses.add(DocumentMessage::StartTransaction);

			self.layer = LayerNodeIdentifier::new_unchecked(NodeId(generate_uuid()));

			responses.add(GraphOperationMessage::NewTextLayer {
				id: self.layer.to_node(),
				text: editing_text.text.clone(),
				parent: document.new_layer_parent(),
				insert_index: -1,
			});
			responses.add(GraphOperationMessage::TransformSetPivot {
				layer: self.layer,
				pivot: DVec2::ZERO,
			});
			responses.add(GraphOperationMessage::TransformSet {
				layer: self.layer,
				transform: editing_text.transform,
				transform_in: TransformIn::Viewport,
				skip_rerender: true,
			});

			self.set_editing(true, document, responses);

			responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![self.layer.to_node()] });

			TextToolFsmState::Editing
		} else {
			// Removing old text as editable
			self.set_editing(false, document, responses);

			TextToolFsmState::Ready
		}
	}
}

fn can_edit_selected(document: &DocumentMessageHandler) -> Option<LayerNodeIdentifier> {
	let mut selected_layers = document.selected_nodes.selected_layers(document.metadata());
	let layer = selected_layers.next()?;

	// Check that only one layer is selected
	if selected_layers.next().is_some() {
		return None;
	}

	if !is_layer_fed_by_node_of_name(layer, &document.network, "Text") {
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
			(TextToolFsmState::Editing | TextToolFsmState::Selecting | TextToolFsmState::Wrap, TextToolMessage::Overlays(mut overlay_context)) => {
				let transform = document.metadata().transform_to_viewport(tool_data.layer);

				let Some(editing_text) = tool_data.editing_text.as_ref() else { return self };
				if editing_text.composition.is_none() {
					responses.add(FrontendMessage::DisplayEditableTextbox {
						text: editing_text.editor.copy_selection().unwrap_or_default(),
						transform: (transform * DAffine2::from_translation(graphene_core::text::cursor_rectangle(&editing_text.editor, &editing_text.text).map_or(DVec2::ZERO, |cursor| cursor[0])))
							.to_cols_array(),
					});
				}
				let Some(mut font_system) = font_cache.get_system() else { return self };
				let subpaths = graphene_core::text::buffer_to_path(editing_text.editor.buffer(), &mut font_system, &editing_text.text.spans, &editing_text.path);
				overlay_context.outline(subpaths.iter(), transform);
				let handle = graphene_core::text::find_line_wrap_handle(editing_text.editor.buffer(), &editing_text.text.spans);
				overlay_context.manipulator_anchor(transform.transform_point2(handle), self == TextToolFsmState::Wrap, None);
				let subpaths = graphene_core::text::selection_shape(&editing_text.editor, &editing_text.text);
				overlay_context.outline(subpaths.iter(), transform);
				let subpaths = graphene_core::text::cursor_shape(&editing_text.editor, &editing_text.text);
				overlay_context.outline(subpaths.iter(), transform);

				self
			}
			(_, TextToolMessage::RefreshFonts) => {
				if let Some(editing) = &mut tool_data.editing_text {
					if let Some(mut font_system) = font_cache.get_system() {
						graphene_core::text::create_buffer(editing.editor.buffer_mut(), &mut font_system, &editing.text, &font_cache, editing.line_length);
						responses.add(OverlaysMessage::Draw);
					}
				}
				self
			}
			(_, TextToolMessage::Select) => {
				let Some(editing_text) = tool_data.editing_text.as_mut() else { return self };
				let handle = graphene_core::text::find_line_wrap_handle(editing_text.editor.buffer(), &editing_text.text.spans);
				let to_viewport = document.metadata().transform_to_viewport(tool_data.layer);
				if to_viewport.transform_point2(handle).distance_squared(input.mouse.position) < SELECTION_TOLERANCE * SELECTION_TOLERANCE {
					return TextToolFsmState::Wrap;
				}

				let pos = to_viewport.inverse().transform_point2(input.mouse.position);
				let Some(cursor) = graphene_core::text::compute_cursor_position(editing_text.editor.buffer(), &editing_text.text, pos)
					.filter(|_| graphene_core::text::has_hit_text_bounds(editing_text.editor.buffer(), &editing_text.text.spans, pos.as_vec2()))
				else {
					return self;
				};
				editing_text.editor.set_cursor(cursor);
				responses.add(ToolMessage::RefreshToolOptions);
				editing_text.editor.set_select_opt(None);
				responses.add(OverlaysMessage::Draw);

				return TextToolFsmState::Selecting;
			}
			(TextToolFsmState::Selecting, TextToolMessage::Drag) => {
				let Some(editing_text) = tool_data.editing_text.as_mut() else { return self };
				let pos = document.metadata().transform_to_viewport(tool_data.layer).inverse().transform_point2(input.mouse.position);
				if editing_text.editor.select_opt().is_none() {
					editing_text.editor.set_select_opt(Some(editing_text.editor.cursor()));
				}
				if let Some(cursor) = graphene_core::text::compute_cursor_position(editing_text.editor.buffer(), &editing_text.text, pos) {
					editing_text.editor.set_cursor(cursor);
					responses.add(ToolMessage::RefreshToolOptions);
				}

				responses.add(OverlaysMessage::Draw);
				self
			}
			(TextToolFsmState::Wrap, TextToolMessage::Drag) => {
				let Some(editing_text) = tool_data.editing_text.as_mut() else { return self };
				let pos = document.metadata().transform_to_viewport(tool_data.layer).inverse().transform_point2(input.mouse.position);
				editing_text.line_length = pos.x.max(0.);
				if let Some(mut font_system) = font_cache.get_system() {
					graphene_core::text::create_buffer(editing_text.editor.buffer_mut(), &mut font_system, &editing_text.text, font_cache, editing_text.line_length);
				}
				responses.add(OverlaysMessage::Draw);
				self
			}
			(TextToolFsmState::Selecting | TextToolFsmState::Wrap, TextToolMessage::Interact) => TextToolFsmState::Editing,
			(TextToolFsmState::Ready, TextToolMessage::Interact) => {
				let font = Arc::new(Font::new(tool_options.font_name.clone(), tool_options.font_style.clone()));
				let size = tool_options.font_size as f32;
				let text = {
					let normal = TextSpan::new(font.clone(), size);
					let italic = TextSpan::new(font.clone(), size).offset(5).italic(Some(5.));
					let bold = TextSpan::new(font.clone(), size).offset(7).bold(Some(2.));
					let letter = TextSpan::new(font.clone(), size).offset(5).letter_spacing(5.);
					let word = TextSpan::new(font, size).offset(15).word_spacing(10.);
					RichText::new("text italic bold letter spacing word spacing", [normal, italic, bold, letter, word])
				};
				let line_length = f64::MAX;
				let editor = graphene_core::text::create_cosmic_editor(&text, font_cache, line_length);
				tool_data.editing_text = editor.map(|editor| EditingText {
					text,
					editor,
					color: tool_options.fill.active_color(),
					transform: DAffine2::from_translation(input.mouse.position),
					path: VectorData::empty(),
					line_length,
					composition: None,
				});
				tool_data.new_text = String::new();

				tool_data.interact(self, input.mouse.position, document, font_cache, responses)
			}
			(state, TextToolMessage::EditSelected) => {
				if let Some(layer) = can_edit_selected(document) {
					tool_data.start_editing_layer(layer, state, document, font_cache, responses);
					return TextToolFsmState::Editing;
				}

				state
			}
			(state, TextToolMessage::Abort) => {
				if state != TextToolFsmState::Ready {
					tool_data.set_editing(false, document, responses);
				}

				TextToolFsmState::Ready
			}
			(TextToolFsmState::Editing | TextToolFsmState::Selecting | TextToolFsmState::Wrap, TextToolMessage::CommitText) | (TextToolFsmState::Editing, TextToolMessage::Interact) => {
				tool_data.set_editing(false, document, responses);
				let Some(editing_text) = tool_data.editing_text.take() else {
					responses.add(NodeGraphMessage::RunDocumentGraph);
					responses.add(OverlaysMessage::Draw);
					return TextToolFsmState::Ready;
				};
				responses.add(NodeGraphMessage::SetQualifiedInputValue {
					node_path: vec![graph_modification_utils::get_text_id(tool_data.layer, &document.network).unwrap()],
					input_index: 1,
					value: TaggedValue::RichText(editing_text.text),
				});
				responses.add(NodeGraphMessage::SetQualifiedInputValue {
					node_path: vec![graph_modification_utils::get_text_id(tool_data.layer, &document.network).unwrap()],
					input_index: 2,
					value: TaggedValue::F64(editing_text.line_length),
				});
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(OverlaysMessage::Draw);

				TextToolFsmState::Ready
			}
			(_, TextToolMessage::TextInput { input_type, data }) => {
				let Some(editing_text) = &mut tool_data.editing_text else { return self };
				let Some(mut font_system) = font_cache.get_system() else { return self };
				use graphene_core::text::cosmic_text::{Action, AttrsList, BufferLine, Cursor, Shaping};
				match input_type.as_str() {
					"insertText" | "insertFromPaste" => {
						if let Some(data) = data {
							editing_text.editor.insert_string(&data, None)
						}
					}
					"insertLineBreak" => editing_text.editor.action(&mut font_system, Action::Enter),
					"deleteContentForward" => editing_text.editor.action(&mut font_system, Action::Delete),
					"deleteContentBackward" => editing_text.editor.action(&mut font_system, Action::Backspace),
					"insertCompositionText" => {
						editing_text.editor.delete_selection();
						let cursor = editing_text.editor.cursor();
						let line = &mut editing_text.editor.buffer_mut().lines[cursor.line];

						let after = line.split_off(cursor.index);
						let after_len = after.text().len();

						line.split_off(line.text().len() - editing_text.composition.unwrap_or(0));
						editing_text.composition = data.as_ref().filter(|data| !data.is_empty()).map(|data| data.len());

						if let Some(data) = data {
							line.append(BufferLine::new(data, AttrsList::new(line.attrs_list().get_span(cursor.index.saturating_sub(1))), Shaping::Advanced));
						}
						line.append(after);
						let index = editing_text.editor.buffer().lines[cursor.line].text().len() - after_len;
						editing_text.editor.set_cursor(Cursor { index, ..cursor });
					}
					"compositionend" => editing_text.composition = None,
					input_type => warn!("Unhandled input type {input_type}"),
				}
				editing_text.text.text.clear();
				let mut used = vec![false; editing_text.text.spans.len()];
				let mut last_total_offset = 0;
				for (index, line) in editing_text.editor.buffer().lines.iter().enumerate() {
					if index != 0 {
						editing_text.text.text.push('\n');
					}
					let line_start = editing_text.text.text.len();
					let spans = line.attrs_list().spans();
					for (val, attrs) in spans {
						if used[attrs.metadata] {
							continue;
						}
						let offset = (line_start + val.start) - last_total_offset;
						editing_text.text.spans[attrs.metadata].offset = offset;
						last_total_offset += offset;
						used[attrs.metadata] = true;
					}
					editing_text.text.text.push_str(line.text());

					if !used[line.attrs_list().defaults().metadata] {
						let len = editing_text.text.text.len();
						let offset = len - last_total_offset;
						editing_text.text.spans[line.attrs_list().defaults().metadata].offset = offset;
						last_total_offset += offset;
						used[line.attrs_list().defaults().metadata] = true;
					}
				}

				used[0] = true;
				let mut used = used.iter();
				editing_text.text.spans.retain(|_| used.next() == Some(&true));
				editing_text.text.spans[0].offset = 0;
				graphene_core::text::create_buffer(editing_text.editor.buffer_mut(), &mut font_system, &editing_text.text, font_cache, editing_text.line_length);
				responses.add(OverlaysMessage::Draw);
				self
			}
			(_, TextToolMessage::TextNavigate { key, shift, ctrl }) => {
				let Some(editing_text) = &mut tool_data.editing_text else { return self };
				let Some(mut font_system) = font_cache.get_system() else { return self };

				use graphene_core::text::cosmic_text::Action;
				let action = match key.as_str() {
					"ArrowLeft" if ctrl => Action::LeftWord,
					"ArrowLeft" => Action::Left,
					"ArrowRight" if ctrl => Action::RightWord,
					"ArrowRight" => Action::Right,
					"ArrowUp" => Action::Up,
					"ArrowDown" => Action::Down,
					"Home" if ctrl => Action::BufferStart,
					"Home" => Action::Home,
					"End" if ctrl => Action::BufferEnd,
					"End" => Action::End,
					"a" if ctrl => {
						use graphene_core::text::cosmic_text::{Affinity, Cursor};
						if let Some(last) = editing_text.editor.buffer().lines.last() {
							let last_index = last.text().len();
							let line = editing_text.editor.buffer().lines.len() - 1;
							editing_text.editor.set_select_opt(Some(Cursor::new_with_affinity(0, 0, Affinity::Before)));
							editing_text.editor.set_cursor(Cursor::new_with_affinity(line, last_index, Affinity::After));
							responses.add(OverlaysMessage::Draw);
						}
						return self;
					}
					_ => return self,
				};

				if shift {
					editing_text
						.editor
						.set_select_opt(Some(editing_text.editor.select_opt().unwrap_or_else(|| editing_text.editor.cursor())));
				} else {
					editing_text.editor.set_select_opt(None);
				}
				editing_text.editor.action(&mut font_system, action);

				responses.add(OverlaysMessage::Draw);
				self
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
			TextToolFsmState::Editing | TextToolFsmState::Selecting | TextToolFsmState::Wrap => HintData(vec![
				HintGroup(vec![HintInfo::keys([Key::Escape], "Discard Changes")]),
				HintGroup(vec![HintInfo::keys([Key::Control, Key::Enter], "Commit Changes").add_mac_keys([Key::Command, Key::Enter])]),
			]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Text });
	}
}
