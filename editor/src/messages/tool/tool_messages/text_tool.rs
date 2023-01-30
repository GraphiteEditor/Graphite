use crate::application::generate_uuid;
use crate::consts::{COLOR_ACCENT, SELECTION_TOLERANCE};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widgets::input_widgets::{FontInput, NumberInput};
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use document_legacy::intersection::Quad;
use document_legacy::layers::layer_info::LayerDataType;
use document_legacy::layers::style::{self, Fill, Stroke};
use document_legacy::layers::text_layer::FontCache;
use document_legacy::{LayerId, Operation};

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

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for TextTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: ToolActionHandlerData<'a>) {
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
	fn set_editing(&self, editable: bool, responses: &mut VecDeque<Message>) {
		let path = self.layer_path.clone();
		responses.push_back(DocumentMessage::SetTextboxEditability { path, editable }.into());
	}
}

fn transform_from_box(pos1: DVec2, pos2: DVec2) -> [f64; 6] {
	DAffine2::from_scale_angle_translation((pos2 - pos1).round(), 0., pos1.round() - DVec2::splat(0.5)).to_cols_array()
}

fn resize_overlays(overlays: &mut Vec<Vec<LayerId>>, responses: &mut VecDeque<Message>, newlen: usize) {
	while overlays.len() > newlen {
		let operation = Operation::DeleteLayer { path: overlays.pop().unwrap() };
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());
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
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());
	}
}

fn update_overlays(document: &DocumentMessageHandler, tool_data: &mut TextToolData, responses: &mut VecDeque<Message>, font_cache: &FontCache) {
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
				.aabb_for_transform(document.document_legacy.multiply_transforms(layer_path).unwrap(), font_cache)
				.map(|bounds| (bounds, overlay_path))
		})
		.collect::<Vec<_>>();

	let new_len = bounds.len();

	for (bounds, overlay_path) in bounds {
		let operation = Operation::SetLayerTransformInViewport {
			path: overlay_path.to_vec(),
			transform: transform_from_box(bounds[0], bounds[1]),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());
	}
	resize_overlays(&mut tool_data.overlays, responses, new_len);
}

impl Fsm for TextToolFsmState {
	type ToolData = TextToolData;
	type ToolOptions = TextOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		(document, _document_id, global_tool_data, input, font_cache): ToolActionHandlerData,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use TextToolFsmState::*;
		use TextToolMessage::*;

		if let ToolMessage::Text(event) = event {
			match (self, event) {
				(state, DocumentIsDirty) => {
					update_overlays(document, tool_data, responses, font_cache);

					state
				}
				(state, Interact) => {
					let mouse_pos = input.mouse.position;
					let tolerance = DVec2::splat(SELECTION_TOLERANCE);
					let quad = Quad::from_box([mouse_pos - tolerance, mouse_pos + tolerance]);

					// Check if the user has selected an existing text layer
					let new_state = if let Some(clicked_text_layer_path) = document
						.document_legacy
						.intersects_quad_root(quad, font_cache)
						.last()
						.filter(|l| document.document_legacy.layer(l).map(|l| l.as_text().is_ok()).unwrap_or(false))
					{
						if state == TextToolFsmState::Editing {
							tool_data.set_editing(false, responses);
						}

						tool_data.layer_path = clicked_text_layer_path.clone();

						responses.push_back(DocumentMessage::StartTransaction.into());

						tool_data.set_editing(true, responses);

						let replacement_selected_layers = vec![tool_data.layer_path.clone()];
						responses.push_back(DocumentMessage::SetSelectedLayers { replacement_selected_layers }.into());

						Editing
					}
					// Create new text
					else if state == TextToolFsmState::Ready {
						responses.push_back(DocumentMessage::StartTransaction.into());

						let transform = DAffine2::from_translation(input.mouse.position).to_cols_array();
						let font_size = tool_options.font_size;
						let font_name = tool_options.font_name.clone();
						let font_style = tool_options.font_style.clone();
						tool_data.layer_path = document.get_path_for_new_layer();

						responses.push_back(
							Operation::AddText {
								path: tool_data.layer_path.clone(),
								transform: DAffine2::ZERO.to_cols_array(),
								insert_index: -1,
								text: String::new(),
								style: style::PathStyle::new(None, Fill::solid(global_tool_data.primary_color)),
								size: font_size as f64,
								font_name,
								font_style,
							}
							.into(),
						);
						responses.push_back(
							Operation::SetLayerTransformInViewport {
								path: tool_data.layer_path.clone(),
								transform,
							}
							.into(),
						);

						tool_data.set_editing(true, responses);

						let replacement_selected_layers = vec![tool_data.layer_path.clone()];

						responses.push_back(DocumentMessage::SetSelectedLayers { replacement_selected_layers }.into());

						Editing
					} else {
						// Removing old text as editable
						tool_data.set_editing(false, responses);

						resize_overlays(&mut tool_data.overlays, responses, 0);

						Ready
					};

					new_state
				}
				(state, EditSelected) => {
					let mut selected_layers = document.selected_layers();

					if let Some(layer_path) = selected_layers.next() {
						// Check that only one layer is selected
						if selected_layers.next().is_none() {
							if let Ok(layer) = document.document_legacy.layer(layer_path) {
								if let LayerDataType::Text(_) = layer.data {
									if state == TextToolFsmState::Editing {
										tool_data.set_editing(false, responses);
									}

									tool_data.layer_path = layer_path.into();

									responses.push_back(DocumentMessage::StartTransaction.into());

									tool_data.set_editing(true, responses);

									let replacement_selected_layers = vec![tool_data.layer_path.clone()];
									responses.push_back(DocumentMessage::SetSelectedLayers { replacement_selected_layers }.into());

									return Editing;
								}
							}
						}
					}

					state
				}
				(state, Abort) => {
					if state == TextToolFsmState::Editing {
						tool_data.set_editing(false, responses);
					}

					resize_overlays(&mut tool_data.overlays, responses, 0);

					Ready
				}
				(Editing, CommitText) => {
					responses.push_back(FrontendMessage::TriggerTextCommit.into());

					Editing
				}
				(Editing, TextChange { new_text }) => {
					let path = tool_data.layer_path.clone();
					responses.push_back(Operation::SetTextContent { path, new_text }.into());

					tool_data.set_editing(false, responses);

					resize_overlays(&mut tool_data.overlays, responses, 0);

					Ready
				}
				(Editing, UpdateBounds { new_text }) => {
					resize_overlays(&mut tool_data.overlays, responses, 1);
					let text = document.document_legacy.layer(&tool_data.layer_path).unwrap().as_text().unwrap();
					let quad = text.bounding_box(&new_text, text.load_face(font_cache));

					let transformed_quad = document.document_legacy.multiply_transforms(&tool_data.layer_path).unwrap() * quad;
					let bounds = transformed_quad.bounding_box();

					let operation = Operation::SetLayerTransformInViewport {
						path: tool_data.overlays[0].clone(),
						transform: transform_from_box(bounds[0], bounds[1]),
					};
					responses.push_back(DocumentMessage::Overlays(operation.into()).into());

					Editing
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

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Text }.into());
	}
}
