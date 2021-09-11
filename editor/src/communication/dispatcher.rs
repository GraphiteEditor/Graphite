use crate::message_prelude::*;

pub use crate::document::DocumentsMessageHandler;
pub use crate::input::{InputMapper, InputPreprocessor};
pub use crate::tool::ToolMessageHandler;

use crate::global::GlobalMessageHandler;
use std::collections::VecDeque;

pub struct Dispatcher {
	input_preprocessor: InputPreprocessor,
	input_mapper: InputMapper,
	global_message_handler: GlobalMessageHandler,
	tool_message_handler: ToolMessageHandler,
	documents_message_handler: DocumentsMessageHandler,
	messages: VecDeque<Message>,
	pub responses: Vec<FrontendMessage>,
}

const GROUP_MESSAGES: &[MessageDiscriminant] = &[
	MessageDiscriminant::Documents(DocumentsMessageDiscriminant::Document(DocumentMessageDiscriminant::RenderDocument)),
	MessageDiscriminant::Documents(DocumentsMessageDiscriminant::Document(DocumentMessageDiscriminant::FolderChanged)),
	MessageDiscriminant::Frontend(FrontendMessageDiscriminant::UpdateLayer),
	MessageDiscriminant::Frontend(FrontendMessageDiscriminant::DisplayFolderTreeStructure),
	MessageDiscriminant::Tool(ToolMessageDiscriminant::SelectedLayersChanged),
];

impl Dispatcher {
	pub fn handle_message<T: Into<Message>>(&mut self, message: T) {
		self.messages.push_back(message.into());

		use Message::*;
		while let Some(message) = self.messages.pop_front() {
			if GROUP_MESSAGES.contains(&message.to_discriminant()) && self.messages.contains(&message) {
				continue;
			}
			log_message(&message);
			match message {
				NoOp => (),
				Documents(message) => self.documents_message_handler.process_action(message, &self.input_preprocessor, &mut self.messages),
				Global(message) => self.global_message_handler.process_action(message, (), &mut self.messages),
				Tool(message) => self
					.tool_message_handler
					.process_action(message, (self.documents_message_handler.active_document(), &self.input_preprocessor), &mut self.messages),
				Frontend(message) => self.responses.push(message),
				InputPreprocessor(message) => self.input_preprocessor.process_action(message, (), &mut self.messages),
				InputMapper(message) => {
					let actions = self.collect_actions();
					self.input_mapper.process_action(message, (&self.input_preprocessor, actions), &mut self.messages)
				}
			}
		}
	}

	pub fn collect_actions(&self) -> ActionList {
		// TODO: Reduce the number of heap allocations
		let mut list = Vec::new();
		list.extend(self.input_preprocessor.actions());
		list.extend(self.input_mapper.actions());
		list.extend(self.global_message_handler.actions());
		list.extend(self.tool_message_handler.actions());
		list.extend(self.documents_message_handler.actions());
		list
	}

	pub fn new() -> Dispatcher {
		Dispatcher {
			input_preprocessor: InputPreprocessor::default(),
			global_message_handler: GlobalMessageHandler::new(),
			input_mapper: InputMapper::default(),
			documents_message_handler: DocumentsMessageHandler::default(),
			tool_message_handler: ToolMessageHandler::default(),
			messages: VecDeque::new(),
			responses: vec![],
		}
	}
}

fn log_message(message: &Message) {
	use Message::*;
	if log::max_level() == log::LevelFilter::Trace
		&& !(matches!(
			message,
			InputPreprocessor(_) | Frontend(FrontendMessage::SetCanvasZoom { .. }) | Frontend(FrontendMessage::SetCanvasRotation { .. })
		) || MessageDiscriminant::from(message).local_name().ends_with("MouseMove"))
	{
		log::trace!("Message: {:?}", message);
		//log::trace!("Hints:{:?}", self.input_mapper.hints(self.collect_actions()));
	}
}

#[cfg(test)]
mod test {
	use crate::{document::DocumentMessageHandler, message_prelude::*, misc::test_utils::EditorTestUtils, Editor};
	use graphene::{color::Color, Operation};

	fn init_logger() {
		let _ = env_logger::builder().is_test(true).try_init();
	}

	/// Create an editor instance with three layers
	/// 1. A red rectangle
	/// 2. A blue shape
	/// 3. A green ellipse
	fn create_editor_with_three_layers() -> Editor {
		let mut editor = Editor::new();

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

		let document_before_copy = editor.dispatcher.documents_message_handler.active_document().graphene_document.clone();
		editor.handle_message(DocumentsMessage::Copy);
		editor.handle_message(DocumentsMessage::PasteIntoFolder { path: vec![], insert_index: -1 });
		let document_after_copy = editor.dispatcher.documents_message_handler.active_document().graphene_document.clone();

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

		let document_before_copy = editor.dispatcher.documents_message_handler.active_document().graphene_document.clone();
		let shape_id = document_before_copy.root.as_folder().unwrap().layer_ids[1];

		editor.handle_message(DocumentMessage::SetSelectedLayers(vec![vec![shape_id]]));
		editor.handle_message(DocumentsMessage::Copy);
		editor.handle_message(DocumentsMessage::PasteIntoFolder { path: vec![], insert_index: -1 });

		let document_after_copy = editor.dispatcher.documents_message_handler.active_document().graphene_document.clone();

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

		editor.handle_message(DocumentMessage::CreateFolder(vec![]));

		let document_before_added_shapes = editor.dispatcher.documents_message_handler.active_document().graphene_document.clone();
		let folder_id = document_before_added_shapes.root.as_folder().unwrap().layer_ids[FOLDER_INDEX];

		// TODO: This adding of a Line and Pen should be rewritten using the corresponding functions in EditorTestUtils.
		// This has not been done yet as the line and pen tool are not yet able to add layers to the currently selected folder
		editor.handle_message(Operation::AddLine {
			path: vec![folder_id, LINE_INDEX as u64],
			insert_index: 0,
			transform: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
			style: Default::default(),
		});

		editor.handle_message(Operation::AddPen {
			path: vec![folder_id, PEN_INDEX as u64],
			insert_index: 0,
			transform: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
			style: Default::default(),
			points: vec![(10.0, 20.0), (30.0, 40.0)],
		});

		editor.handle_message(DocumentMessage::SetSelectedLayers(vec![vec![folder_id]]));

		let document_before_copy = editor.dispatcher.documents_message_handler.active_document().graphene_document.clone();

		editor.handle_message(DocumentsMessage::Copy);
		editor.handle_message(DocumentMessage::DeleteSelectedLayers);
		editor.handle_message(DocumentsMessage::PasteIntoFolder { path: vec![], insert_index: -1 });
		editor.handle_message(DocumentsMessage::PasteIntoFolder { path: vec![], insert_index: -1 });

		let document_after_copy = editor.dispatcher.documents_message_handler.active_document().graphene_document.clone();

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

		let document_before_copy = editor.dispatcher.documents_message_handler.active_document().graphene_document.clone();
		let rect_id = document_before_copy.root.as_folder().unwrap().layer_ids[RECT_INDEX];
		let ellipse_id = document_before_copy.root.as_folder().unwrap().layer_ids[ELLIPSE_INDEX];

		editor.handle_message(DocumentMessage::SetSelectedLayers(vec![vec![rect_id], vec![ellipse_id]]));
		editor.handle_message(DocumentsMessage::Copy);
		editor.handle_message(DocumentMessage::DeleteSelectedLayers);
		editor.draw_rect(0., 800., 12., 200.);
		editor.handle_message(DocumentsMessage::PasteIntoFolder { path: vec![], insert_index: -1 });
		editor.handle_message(DocumentsMessage::PasteIntoFolder { path: vec![], insert_index: -1 });

		let document_after_copy = editor.dispatcher.documents_message_handler.active_document().graphene_document.clone();

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
	/// - create rect, shape and ellipse
	/// - select ellipse and rect
	/// - move them down and back up again
	fn move_selection() {
		init_logger();
		let mut editor = create_editor_with_three_layers();

		let verify_order = |handler: &mut DocumentMessageHandler| (handler.all_layers_sorted(), handler.non_selected_layers_sorted(), handler.selected_layers_sorted());

		editor.handle_message(DocumentMessage::SetSelectedLayers(vec![vec![0], vec![2]]));

		editor.handle_message(DocumentMessage::ReorderSelectedLayers(1));
		let (all, non_selected, selected) = verify_order(&mut editor.dispatcher.documents_message_handler.active_document_mut());
		assert_eq!(all, non_selected.into_iter().chain(selected.into_iter()).collect::<Vec<_>>());

		editor.handle_message(DocumentMessage::ReorderSelectedLayers(-1));
		let (all, non_selected, selected) = verify_order(&mut editor.dispatcher.documents_message_handler.active_document_mut());
		assert_eq!(all, selected.into_iter().chain(non_selected.into_iter()).collect::<Vec<_>>());

		editor.handle_message(DocumentMessage::ReorderSelectedLayers(i32::MAX));
		let (all, non_selected, selected) = verify_order(&mut editor.dispatcher.documents_message_handler.active_document_mut());
		assert_eq!(all, non_selected.into_iter().chain(selected.into_iter()).collect::<Vec<_>>());
	}
}
