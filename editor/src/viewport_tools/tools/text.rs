use crate::consts::{COLOR_ACCENT, SELECTION_TOLERANCE};
use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::{Key, MouseMotion};
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::widgets::{LayoutRow, NumberInput, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::tool::{DocumentToolData, Fsm, ToolActionHandlerData};

use glam::{DAffine2, DVec2};
use graphene::color::Color;
use graphene::intersection::Quad;
use graphene::layers::style::{self, Fill, Stroke};
use graphene::Operation;
use kurbo::Shape;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Text {
	fsm_state: TextToolFsmState,
	data: TextToolData,
	options: TextOptions,
}

pub struct TextOptions {
	font_size: u32,
}

impl Default for TextOptions {
	fn default() -> Self {
		Self { font_size: 14 }
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Text)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum TextMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

	#[remain::unsorted]
	DocumentIsDirty,

	// Tool-specific messages
	CommitText,
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
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum TextOptionsUpdate {
	FontSize(u32),
}

impl PropertyHolder for Text {
	fn properties(&self) -> WidgetLayout {
		WidgetLayout::new(vec![LayoutRow::Row {
			name: "".into(),
			widgets: vec![WidgetHolder::new(Widget::NumberInput(NumberInput {
				unit: " px".into(),
				label: "Font Size".into(),
				value: self.options.font_size as f64,
				is_integer: true,
				min: Some(1.),
				on_update: WidgetCallback::new(|number_input| TextMessage::UpdateOptions(TextOptionsUpdate::FontSize(number_input.value as u32)).into()),
				..NumberInput::default()
			}))],
		}])
	}
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Text {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		if let ToolMessage::Text(TextMessage::UpdateOptions(action)) = action {
			match action {
				TextOptionsUpdate::FontSize(font_size) => self.options.font_size = font_size,
			}
			return;
		}

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, &self.options, data.2, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	fn actions(&self) -> ActionList {
		use TextToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(TextMessageDiscriminant; Interact),
			Editing => actions!(TextMessageDiscriminant; Interact, Abort, CommitText),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextToolFsmState {
	Ready,
	Editing,
}

impl Default for TextToolFsmState {
	fn default() -> Self {
		TextToolFsmState::Ready
	}
}

#[derive(Clone, Debug, Default)]
struct TextToolData {
	path: Vec<LayerId>,
	overlays: Vec<Vec<LayerId>>,
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

		let operation = Operation::AddOverlayRect {
			path,
			transform: DAffine2::ZERO.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), None),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());
	}
}

fn update_overlays(document: &DocumentMessageHandler, data: &mut TextToolData, responses: &mut VecDeque<Message>) {
	let visible_text_layers = document.selected_visible_text_layers().collect::<Vec<_>>();

	resize_overlays(&mut data.overlays, responses, visible_text_layers.len());

	for (layer_path, overlay_path) in visible_text_layers.into_iter().zip(&data.overlays) {
		let bounds = document
			.graphene_document
			.layer(layer_path)
			.unwrap()
			.current_bounding_box_with_transform(document.graphene_document.multiply_transforms(layer_path).unwrap())
			.unwrap();

		let operation = Operation::SetLayerTransformInViewport {
			path: overlay_path.to_vec(),
			transform: transform_from_box(bounds[0], bounds[1]),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());
	}
}

impl Fsm for TextToolFsmState {
	type ToolData = TextToolData;
	type ToolOptions = TextOptions;

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		tool_options: &Self::ToolOptions,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use TextMessage::*;
		use TextToolFsmState::*;

		if let ToolMessage::Text(event) = event {
			match (self, event) {
				(state, DocumentIsDirty) => {
					update_overlays(document, data, responses);

					state
				}
				(state, Interact) => {
					let mouse_pos = input.mouse.position;
					let tolerance = DVec2::splat(SELECTION_TOLERANCE);
					let quad = Quad::from_box([mouse_pos - tolerance, mouse_pos + tolerance]);

					let new_state = if let Some(l) = document
						.graphene_document
						.intersects_quad_root(quad)
						.last()
						.filter(|l| document.graphene_document.layer(l).map(|l| l.as_text().is_ok()).unwrap_or(false))
					// Editing existing text
					{
						if state == TextToolFsmState::Editing {
							responses.push_back(
								DocumentMessage::SetTexboxEditability {
									path: data.path.clone(),
									editable: false,
								}
								.into(),
							);
						}

						data.path = l.clone();

						responses.push_back(
							DocumentMessage::SetTexboxEditability {
								path: data.path.clone(),
								editable: true,
							}
							.into(),
						);
						responses.push_back(
							DocumentMessage::SetSelectedLayers {
								replacement_selected_layers: vec![data.path.clone()],
							}
							.into(),
						);

						Editing
					}
					// Creating new text
					else if state == TextToolFsmState::Ready {
						let transform = DAffine2::from_translation(input.mouse.position).to_cols_array();
						let font_size = tool_options.font_size;
						data.path = vec![generate_uuid()];

						responses.push_back(
							Operation::AddText {
								path: data.path.clone(),
								transform: DAffine2::ZERO.to_cols_array(),
								insert_index: -1,
								text: r#""#.to_string(),
								style: style::PathStyle::new(None, Some(Fill::new(tool_data.primary_color))),
								size: font_size as f64,
							}
							.into(),
						);
						responses.push_back(Operation::SetLayerTransformInViewport { path: data.path.clone(), transform }.into());

						responses.push_back(
							DocumentMessage::SetTexboxEditability {
								path: data.path.clone(),
								editable: true,
							}
							.into(),
						);

						responses.push_back(
							DocumentMessage::SetSelectedLayers {
								replacement_selected_layers: vec![data.path.clone()],
							}
							.into(),
						);

						Editing
					} else {
						// Removing old text as editable
						responses.push_back(
							DocumentMessage::SetTexboxEditability {
								path: data.path.clone(),
								editable: false,
							}
							.into(),
						);

						resize_overlays(&mut data.overlays, responses, 0);

						Ready
					};

					new_state
				}
				(state, Abort) => {
					if state == TextToolFsmState::Editing {
						responses.push_back(
							DocumentMessage::SetTexboxEditability {
								path: data.path.clone(),
								editable: false,
							}
							.into(),
						);
					}

					resize_overlays(&mut data.overlays, responses, 0);

					Ready
				}
				(Editing, CommitText) => {
					responses.push_back(FrontendMessage::TriggerTextCommit.into());

					Editing
				}
				(Editing, TextChange { new_text }) => {
					responses.push_back(Operation::SetTextContent { path: data.path.clone(), new_text }.into());

					responses.push_back(
						DocumentMessage::SetTexboxEditability {
							path: data.path.clone(),
							editable: false,
						}
						.into(),
					);

					resize_overlays(&mut data.overlays, responses, 0);

					Ready
				}
				(Editing, UpdateBounds { new_text }) => {
					resize_overlays(&mut data.overlays, responses, 1);
					let mut path = document.graphene_document.layer(&data.path).unwrap().as_text().unwrap().bounding_box(&new_text).to_path(0.1);

					fn glam_to_kurbo(transform: DAffine2) -> kurbo::Affine {
						kurbo::Affine::new(transform.to_cols_array())
					}

					path.apply_affine(glam_to_kurbo(document.graphene_document.multiply_transforms(&data.path).unwrap()));

					let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();

					let operation = Operation::SetLayerTransformInViewport {
						path: data.overlays[0].clone(),
						transform: transform_from_box(DVec2::new(x0, y0), DVec2::new(x1, y1)),
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
			TextToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::Lmb),
					label: String::from("Add Text"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::Lmb),
					label: String::from("Edit Text"),
					plus: false,
				},
			])]),
			TextToolFsmState::Editing => HintData(vec![HintGroup(vec![
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyControl, Key::KeyEnter])],
					mouse: None,
					label: String::from("Commit Edit"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyEscape])],
					mouse: None,
					label: String::from("Discard Edit"),
					plus: false,
				},
			])]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Text }.into());
	}
}
