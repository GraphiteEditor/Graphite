use super::broadcast_message_handler::BroadcastMessageHandler;
use crate::consts::{DEFAULT_FONT_FAMILY, DEFAULT_FONT_STYLE};
use crate::debug::debug_message::LoggingMessages;
use crate::debug::DebugMessageHandler;
use crate::document::PortfolioMessageHandler;
use crate::input::{InputMapperMessageHandler, InputPreprocessorMessageHandler};
use crate::layout::layout_message_handler::LayoutMessageHandler;
use crate::message_prelude::*;
use crate::viewport_tools::tool_message_handler::ToolMessageHandler;
use crate::workspace::WorkspaceMessageHandler;

use graphene::layers::text_layer::Font;

use std::collections::VecDeque;

#[derive(Debug, Default)]
pub struct Dispatcher {
	message_queues: Vec<VecDeque<Message>>,
	pub responses: Vec<FrontendMessage>,
	message_handlers: DispatcherMessageHandlers,
}

#[remain::sorted]
#[derive(Debug, Default)]
struct DispatcherMessageHandlers {
	broadcast_message_handler: BroadcastMessageHandler,
	debug_message_handler: DebugMessageHandler,
	dialog_message_handler: DialogMessageHandler,
	input_mapper_message_handler: InputMapperMessageHandler,
	input_preprocessor_message_handler: InputPreprocessorMessageHandler,
	layout_message_handler: LayoutMessageHandler,
	portfolio_message_handler: PortfolioMessageHandler,
	tool_message_handler: ToolMessageHandler,
	workspace_message_handler: WorkspaceMessageHandler,
}

/// For optimization, these are messages guaranteed to be redundant when repeated.
/// The last occurrence of the message in the message queue is sufficient to ensure correct behavior.
/// In addition, these messages do not change any state in the backend (aside from caches).
const SIDE_EFFECT_FREE_MESSAGES: &[MessageDiscriminant] = &[
	MessageDiscriminant::Portfolio(PortfolioMessageDiscriminant::Document(DocumentMessageDiscriminant::RenderDocument)),
	MessageDiscriminant::Portfolio(PortfolioMessageDiscriminant::Document(DocumentMessageDiscriminant::Overlays(OverlaysMessageDiscriminant::Rerender))),
	MessageDiscriminant::Portfolio(PortfolioMessageDiscriminant::Document(DocumentMessageDiscriminant::Artboard(
		ArtboardMessageDiscriminant::RenderArtboards,
	))),
	MessageDiscriminant::Portfolio(PortfolioMessageDiscriminant::Document(DocumentMessageDiscriminant::FolderChanged)),
	MessageDiscriminant::Portfolio(PortfolioMessageDiscriminant::Document(DocumentMessageDiscriminant::DocumentStructureChanged)),
	MessageDiscriminant::Frontend(FrontendMessageDiscriminant::UpdateDocumentLayerTreeStructure),
	MessageDiscriminant::Frontend(FrontendMessageDiscriminant::TriggerFontLoad),
	MessageDiscriminant::Broadcast(BroadcastMessageDiscriminant::TriggerSignal(BroadcastSignalDiscriminant::DocumentIsDirty)),
];

impl Dispatcher {
	pub fn new() -> Self {
		Self::default()
	}

	// If the deepest queues (higher index in queues list) are now empty (after being popped from) then remove them
	fn cleanup_queues(&mut self, leave_last: bool) {
		while self.message_queues.last().filter(|queue| queue.is_empty()).is_some() {
			if leave_last && self.message_queues.len() == 1 {
				break;
			}
			self.message_queues.pop();
		}
	}

	#[remain::check]
	pub fn handle_message<T: Into<Message>>(&mut self, message: T) {
		use Message::*;

		if let Some(first) = self.message_queues.first_mut() {
			first.push_back(message.into());
		} else {
			self.message_queues.push(VecDeque::from_iter([message.into()]));
		}

		while let Some(message) = self.message_queues.last_mut().and_then(VecDeque::pop_front) {
			// Skip processing of this message if it will be processed later (at the end of the shallowest level queue)
			if SIDE_EFFECT_FREE_MESSAGES.contains(&message.to_discriminant()) {
				let already_in_queue = self.message_queues.first().filter(|queue| queue.contains(&message)).is_some();
				if already_in_queue {
					self.log_deferred_message(&message, &self.message_queues, self.message_handlers.debug_message_handler.logging_messages_mode);
					self.cleanup_queues(false);
					continue;
				} else if self.message_queues.len() > 1 {
					self.log_deferred_message(&message, &self.message_queues, self.message_handlers.debug_message_handler.logging_messages_mode);
					self.cleanup_queues(true);
					self.message_queues[0].push_back(message);
					continue;
				}
			}

			// Print the message at a verbosity level of `log`
			self.log_message(&message, &self.message_queues, self.message_handlers.debug_message_handler.logging_messages_mode);

			// Create a new queue for the child messages
			let mut queue = VecDeque::new();

			// Process the action by forwarding it to the relevant message handler, or saving the FrontendMessage to be sent to the frontend
			#[remain::sorted]
			match message {
				#[remain::unsorted]
				NoOp => {}
				#[remain::unsorted]
				Init => {
					// Display the menu bar at the top of the window
					let message = MenuBarMessage::SendLayout.into();
					queue.push_back(message);

					// Load the default font
					let font = Font::new(DEFAULT_FONT_FAMILY.into(), DEFAULT_FONT_STYLE.into());
					let message = FrontendMessage::TriggerFontLoad { font, is_default: true }.into();
					queue.push_back(message);
				}

				Broadcast(message) => self.message_handlers.broadcast_message_handler.process_action(message, (), &mut queue),
				Debug(message) => {
					self.message_handlers.debug_message_handler.process_action(message, (), &mut queue);
				}
				Dialog(message) => {
					self.message_handlers
						.dialog_message_handler
						.process_action(message, &self.message_handlers.portfolio_message_handler, &mut queue);
				}
				Frontend(message) => {
					// Handle these messages immediately by returning early
					if let FrontendMessage::UpdateImageData { .. } | FrontendMessage::TriggerFontLoad { .. } | FrontendMessage::TriggerRefreshBoundsOfViewports = message {
						self.responses.push(message);
						self.cleanup_queues(false);

						// Return early to avoid running the code after the match block
						return;
					} else {
						// `FrontendMessage`s are saved and will be sent to the frontend after the message queue is done being processed
						self.responses.push(message);
					}
				}
				InputMapper(message) => {
					let actions = self.collect_actions();
					let keyboard_platform = self.message_handlers.portfolio_message_handler.platform.as_keyboard_platform_layout();

					self.message_handlers
						.input_mapper_message_handler
						.process_action(message, (&self.message_handlers.input_preprocessor_message_handler, keyboard_platform, actions), &mut queue);
				}
				InputPreprocessor(message) => {
					let keyboard_platform = self.message_handlers.portfolio_message_handler.platform.as_keyboard_platform_layout();

					self.message_handlers.input_preprocessor_message_handler.process_action(message, keyboard_platform, &mut queue);
				}
				Layout(message) => {
					let action_input_mapping = &|action_to_find: &MessageDiscriminant| {
						let keyboard_platform = self.message_handlers.portfolio_message_handler.platform.as_keyboard_platform_layout();
						self.message_handlers.input_mapper_message_handler.action_input_mapping(action_to_find, keyboard_platform)
					};

					self.message_handlers.layout_message_handler.process_action(message, action_input_mapping, &mut queue);
				}
				Portfolio(message) => {
					self.message_handlers
						.portfolio_message_handler
						.process_action(message, &self.message_handlers.input_preprocessor_message_handler, &mut queue);
				}
				Tool(message) => {
					if let Some(document) = self.message_handlers.portfolio_message_handler.active_document() {
						self.message_handlers.tool_message_handler.process_action(
							message,
							(
								document,
								&self.message_handlers.input_preprocessor_message_handler,
								self.message_handlers.portfolio_message_handler.font_cache(),
							),
							&mut queue,
						);
					} else {
						log::warn!("Called ToolMessage without an active document.\nGot {:?}", message);
					}
				}
				Workspace(message) => {
					self.message_handlers
						.workspace_message_handler
						.process_action(message, &self.message_handlers.input_preprocessor_message_handler, &mut queue);
				}
			}

			// If there are child messages, append the queue to the list of queues
			if !queue.is_empty() {
				self.message_queues.push(queue);
			}

			self.cleanup_queues(false);
		}
	}

	pub fn collect_actions(&self) -> ActionList {
		// TODO: Reduce the number of heap allocations
		let mut list = Vec::new();
		list.extend(self.message_handlers.dialog_message_handler.actions());
		list.extend(self.message_handlers.input_preprocessor_message_handler.actions());
		list.extend(self.message_handlers.input_mapper_message_handler.actions());
		list.extend(self.message_handlers.debug_message_handler.actions());
		if self.message_handlers.portfolio_message_handler.active_document().is_some() {
			list.extend(self.message_handlers.tool_message_handler.actions());
		}
		list.extend(self.message_handlers.portfolio_message_handler.actions());
		list
	}

	/// Create the tree structure for logging the messages as a tree
	fn create_indents(queues: &[VecDeque<Message>]) -> String {
		String::from_iter(queues.iter().enumerate().skip(1).map(|(index, queue)| {
			if index == queues.len() - 1 {
				if queue.is_empty() {
					"└── "
				} else {
					"├── "
				}
			} else if queue.is_empty() {
				"   "
			} else {
				"│    "
			}
		}))
	}

	/// Logs a message that is about to be executed,
	/// either as a tree with a discriminant or the entire payload (depending on settings)
	fn log_message(&self, message: &Message, queues: &[VecDeque<Message>], log_tree_contents: LoggingMessages) {
		if !MessageDiscriminant::from(message).local_name().ends_with("PointerMove") {
			match log_tree_contents {
				LoggingMessages::Off => {}
				LoggingMessages::Names => {
					log::info!("{}{:?}", Self::create_indents(queues), message.to_discriminant());
				}
				LoggingMessages::Contents => {
					if !(matches!(message, Message::InputPreprocessor(_))) {
						log::info!("Message: {}{:?}", Self::create_indents(queues), message);
					}
				}
			}
		}
	}

	/// Logs into the tree that the message is in the side effect free messages and its execution will be deferred
	fn log_deferred_message(&self, message: &Message, queues: &[VecDeque<Message>], log_tree_contents: LoggingMessages) {
		if let LoggingMessages::Names = log_tree_contents {
			log::info!("{}Deferred \"{:?}\" because it's a SIDE_EFFECT_FREE_MESSAGE", Self::create_indents(queues), message.to_discriminant());
		}
	}
}

#[cfg(test)]
mod test {
	use crate::communication::set_uuid_seed;
	use crate::document::clipboards::Clipboard;
	use crate::document::DocumentMessageHandler;
	use crate::message_prelude::*;
	use crate::misc::test_utils::EditorTestUtils;
	use crate::Editor;

	use graphene::color::Color;
	use graphene::Operation;

	fn init_logger() {
		let _ = env_logger::builder().is_test(true).try_init();
	}

	/// Create an editor instance with three layers
	/// 1. A red rectangle
	/// 2. A blue shape
	/// 3. A green ellipse
	fn create_editor_with_three_layers() -> Editor {
		set_uuid_seed(0);
		let mut editor = Editor::new();

		editor.new_document();
		editor.select_primary_color(Color::RED);
		editor.draw_rect(100., 200., 300., 400.);
		editor.select_primary_color(Color::BLUE);
		editor.draw_shape(10., 1200., 1300., 400.);
		editor.select_primary_color(Color::GREEN);
		editor.draw_ellipse(104., 1200., 1300., 400.);

		editor
	}

	#[test]
	/// - create rect, shape and ellipse
	/// - copy
	/// - paste
	/// - assert that ellipse was copied
	fn copy_paste_single_layer() {
		init_logger();
		let mut editor = create_editor_with_three_layers();

		let document_before_copy = editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().graphene_document.clone();
		editor.handle_message(PortfolioMessage::Copy { clipboard: Clipboard::Internal });
		editor.handle_message(PortfolioMessage::PasteIntoFolder {
			clipboard: Clipboard::Internal,
			folder_path: vec![],
			insert_index: -1,
		});
		let document_after_copy = editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().graphene_document.clone();

		let layers_before_copy = document_before_copy.root.as_folder().unwrap().layers();
		let layers_after_copy = document_after_copy.root.as_folder().unwrap().layers();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 4);

		// Existing layers are unaffected
		for i in 0..=2 {
			assert_eq!(layers_before_copy[i], layers_after_copy[i]);
		}

		// The ellipse was copied
		assert_eq!(layers_before_copy[2], layers_after_copy[3]);
	}

	#[test]
	/// - create rect, shape and ellipse
	/// - select shape
	/// - copy
	/// - paste
	/// - assert that shape was copied
	fn copy_paste_single_layer_from_middle() {
		init_logger();
		let mut editor = create_editor_with_three_layers();

		let document_before_copy = editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().graphene_document.clone();
		let shape_id = document_before_copy.root.as_folder().unwrap().layer_ids[1];

		editor.handle_message(DocumentMessage::SetSelectedLayers {
			replacement_selected_layers: vec![vec![shape_id]],
		});
		editor.handle_message(PortfolioMessage::Copy { clipboard: Clipboard::Internal });
		editor.handle_message(PortfolioMessage::PasteIntoFolder {
			clipboard: Clipboard::Internal,
			folder_path: vec![],
			insert_index: -1,
		});

		let document_after_copy = editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().graphene_document.clone();

		let layers_before_copy = document_before_copy.root.as_folder().unwrap().layers();
		let layers_after_copy = document_after_copy.root.as_folder().unwrap().layers();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 4);

		// Existing layers are unaffected
		for i in 0..=2 {
			assert_eq!(layers_before_copy[i], layers_after_copy[i]);
		}

		// The shape was copied
		assert_eq!(layers_before_copy[1], layers_after_copy[3]);
	}

	#[test]
	fn copy_paste_folder() {
		init_logger();
		let mut editor = create_editor_with_three_layers();

		const FOLDER_INDEX: usize = 3;
		const ELLIPSE_INDEX: usize = 2;
		const SHAPE_INDEX: usize = 1;
		const RECT_INDEX: usize = 0;

		const LINE_INDEX: usize = 0;
		const PEN_INDEX: usize = 1;

		editor.handle_message(DocumentMessage::CreateEmptyFolder { container_path: vec![] });

		let document_before_added_shapes = editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().graphene_document.clone();
		let folder_id = document_before_added_shapes.root.as_folder().unwrap().layer_ids[FOLDER_INDEX];

		// TODO: This adding of a Line and Pen should be rewritten using the corresponding functions in EditorTestUtils.
		// This has not been done yet as the line and pen tool are not yet able to add layers to the currently selected folder
		editor.handle_message(Operation::AddLine {
			path: vec![folder_id, LINE_INDEX as u64],
			insert_index: 0,
			transform: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
			style: Default::default(),
		});

		editor.handle_message(Operation::AddPolyline {
			path: vec![folder_id, PEN_INDEX as u64],
			insert_index: 0,
			transform: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
			style: Default::default(),
			points: vec![(10.0, 20.0), (30.0, 40.0)],
		});

		editor.handle_message(DocumentMessage::SetSelectedLayers {
			replacement_selected_layers: vec![vec![folder_id]],
		});

		let document_before_copy = editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().graphene_document.clone();

		editor.handle_message(PortfolioMessage::Copy { clipboard: Clipboard::Internal });
		editor.handle_message(DocumentMessage::DeleteSelectedLayers);
		editor.handle_message(PortfolioMessage::PasteIntoFolder {
			clipboard: Clipboard::Internal,
			folder_path: vec![],
			insert_index: -1,
		});
		editor.handle_message(PortfolioMessage::PasteIntoFolder {
			clipboard: Clipboard::Internal,
			folder_path: vec![],
			insert_index: -1,
		});

		let document_after_copy = editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().graphene_document.clone();

		let layers_before_copy = document_before_copy.root.as_folder().unwrap().layers();
		let layers_after_copy = document_after_copy.root.as_folder().unwrap().layers();

		assert_eq!(layers_before_copy.len(), 4);
		assert_eq!(layers_after_copy.len(), 5);

		let rect_before_copy = &layers_before_copy[RECT_INDEX];
		let ellipse_before_copy = &layers_before_copy[ELLIPSE_INDEX];
		let shape_before_copy = &layers_before_copy[SHAPE_INDEX];
		let folder_before_copy = &layers_before_copy[FOLDER_INDEX];
		let line_before_copy = folder_before_copy.as_folder().unwrap().layers()[LINE_INDEX].clone();
		let pen_before_copy = folder_before_copy.as_folder().unwrap().layers()[PEN_INDEX].clone();

		assert_eq!(&layers_after_copy[0], rect_before_copy);
		assert_eq!(&layers_after_copy[1], shape_before_copy);
		assert_eq!(&layers_after_copy[2], ellipse_before_copy);
		assert_eq!(&layers_after_copy[3], folder_before_copy);
		assert_eq!(&layers_after_copy[4], folder_before_copy);

		// Check the layers inside the two folders
		let first_folder_layers_after_copy = layers_after_copy[3].as_folder().unwrap().layers();
		let second_folder_layers_after_copy = layers_after_copy[4].as_folder().unwrap().layers();

		assert_eq!(first_folder_layers_after_copy.len(), 2);
		assert_eq!(second_folder_layers_after_copy.len(), 2);

		assert_eq!(first_folder_layers_after_copy[0], line_before_copy);
		assert_eq!(first_folder_layers_after_copy[1], pen_before_copy);

		assert_eq!(second_folder_layers_after_copy[0], line_before_copy);
		assert_eq!(second_folder_layers_after_copy[1], pen_before_copy);
	}

	#[test]
	/// - create rect, shape and ellipse
	/// - select ellipse and rect
	/// - copy
	/// - delete
	/// - create another rect
	/// - paste
	/// - paste
	fn copy_paste_deleted_layers() {
		init_logger();
		let mut editor = create_editor_with_three_layers();

		const ELLIPSE_INDEX: usize = 2;
		const SHAPE_INDEX: usize = 1;
		const RECT_INDEX: usize = 0;

		let document_before_copy = editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().graphene_document.clone();
		let rect_id = document_before_copy.root.as_folder().unwrap().layer_ids[RECT_INDEX];
		let ellipse_id = document_before_copy.root.as_folder().unwrap().layer_ids[ELLIPSE_INDEX];

		editor.handle_message(DocumentMessage::SetSelectedLayers {
			replacement_selected_layers: vec![vec![rect_id], vec![ellipse_id]],
		});
		editor.handle_message(PortfolioMessage::Copy { clipboard: Clipboard::Internal });
		editor.handle_message(DocumentMessage::DeleteSelectedLayers);
		editor.draw_rect(0., 800., 12., 200.);
		editor.handle_message(PortfolioMessage::PasteIntoFolder {
			clipboard: Clipboard::Internal,
			folder_path: vec![],
			insert_index: -1,
		});
		editor.handle_message(PortfolioMessage::PasteIntoFolder {
			clipboard: Clipboard::Internal,
			folder_path: vec![],
			insert_index: -1,
		});

		let document_after_copy = editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().graphene_document.clone();

		let layers_before_copy = document_before_copy.root.as_folder().unwrap().layers();
		let layers_after_copy = document_after_copy.root.as_folder().unwrap().layers();

		assert_eq!(layers_before_copy.len(), 3);
		assert_eq!(layers_after_copy.len(), 6);

		let rect_before_copy = &layers_before_copy[RECT_INDEX];
		let ellipse_before_copy = &layers_before_copy[ELLIPSE_INDEX];

		assert_eq!(layers_after_copy[0], layers_before_copy[SHAPE_INDEX]);
		assert_eq!(&layers_after_copy[2], rect_before_copy);
		assert_eq!(&layers_after_copy[3], ellipse_before_copy);
		assert_eq!(&layers_after_copy[4], rect_before_copy);
		assert_eq!(&layers_after_copy[5], ellipse_before_copy);
	}

	#[test]
	#[ignore] // TODO: Re-enable test, see issue #444 (https://github.com/GraphiteEditor/Graphite/pull/444)
	/// - create rect, shape and ellipse
	/// - select ellipse and rect
	/// - move them down and back up again
	fn move_selection() {
		init_logger();
		let mut editor = create_editor_with_three_layers();

		fn map_to_vec(paths: Vec<&[LayerId]>) -> Vec<Vec<LayerId>> {
			paths.iter().map(|layer| layer.to_vec()).collect::<Vec<_>>()
		}
		let sorted_layers = map_to_vec(editor.dispatcher.message_handlers.portfolio_message_handler.active_document().unwrap().all_layers_sorted());
		println!("Sorted layers: {:?}", sorted_layers);

		let verify_order = |handler: &mut DocumentMessageHandler| {
			(
				map_to_vec(handler.all_layers_sorted()),
				map_to_vec(handler.non_selected_layers_sorted()),
				map_to_vec(handler.selected_layers_sorted()),
			)
		};

		editor.handle_message(DocumentMessage::SetSelectedLayers {
			replacement_selected_layers: sorted_layers[..2].to_vec(),
		});

		editor.handle_message(DocumentMessage::SelectedLayersRaise);
		let (all, non_selected, selected) = verify_order(editor.dispatcher.message_handlers.portfolio_message_handler.active_document_mut().unwrap());
		assert_eq!(all, non_selected.into_iter().chain(selected.into_iter()).collect::<Vec<_>>());

		editor.handle_message(DocumentMessage::SelectedLayersLower);
		let (all, non_selected, selected) = verify_order(editor.dispatcher.message_handlers.portfolio_message_handler.active_document_mut().unwrap());
		assert_eq!(all, selected.into_iter().chain(non_selected.into_iter()).collect::<Vec<_>>());

		editor.handle_message(DocumentMessage::SelectedLayersRaiseToFront);
		let (all, non_selected, selected) = verify_order(editor.dispatcher.message_handlers.portfolio_message_handler.active_document_mut().unwrap());
		assert_eq!(all, non_selected.into_iter().chain(selected.into_iter()).collect::<Vec<_>>());
	}

	#[test]
	/// If this test is failing take a look at `GRAPHITE_DOCUMENT_VERSION` in `editor/src/consts.rs`, it may need to be updated.
	/// This test will fail when you make changes to the underlying serialization format for a document.
	fn check_if_graphite_file_version_upgrade_is_needed() {
		use crate::layout::widgets::{LayoutGroup, TextLabel, Widget};
		let print_problem_to_terminal_on_failure = |value: &String| {
			println!();
			println!("-------------------------------------------------");
			println!("Failed test due to receiving a DisplayDialogError while loading the Graphite sample file!");
			println!("This is most likely caused by forgetting to bump the `GRAPHITE_DOCUMENT_VERSION` in `editor/src/consts.rs`");
			println!("After bumping this version number, please replace the `graphite-test-document.graphite` with a valid file [saved from the editor].");
			println!("DisplayDialogError details:");
			println!();
			println!("Description: {}", value);
			println!("-------------------------------------------------");
			println!();
			panic!()
		};

		init_logger();
		set_uuid_seed(0);
		let mut editor = Editor::new();
		let test_file = include_str!("./graphite-test-document.graphite");
		let responses = editor.handle_message(PortfolioMessage::OpenDocumentFile {
			document_name: "Graphite Version Test".into(),
			document_serialized_content: test_file.into(),
		});

		for response in responses {
			// Check for the existence of the file format incompatibility warning dialog after opening the test file
			if let FrontendMessage::UpdateDialogDetails { layout_target: _, layout } = response {
				if let LayoutGroup::Row { widgets } = &layout[0] {
					if let Widget::TextLabel(TextLabel { value, .. }) = &widgets[0].widget {
						print_problem_to_terminal_on_failure(value);
					}
				}
			}
		}
	}
}
