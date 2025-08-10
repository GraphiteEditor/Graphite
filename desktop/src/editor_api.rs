use graphene_std::{Color, raster::Image};
use graphite_editor::{
	application::Editor,
	messages::prelude::{DocumentId, DocumentMessage, FrontendMessage, InputPreprocessorMessage, Message, PortfolioMessage},
};
use std::{io::Cursor, path::PathBuf};

pub struct EditorApi {
	editor: Editor,
}

impl EditorApi {
	pub fn new() -> Self {
		Self { editor: Editor::new() }
	}

	pub fn dispatch(&mut self, message: EditorMessage) -> Vec<NativeMessage> {
		let mut responses = Vec::new();
		match message {
			EditorMessage::FromFrontend(data) => {
				let string = std::str::from_utf8(&data).unwrap();
				match ron::from_str::<Message>(string) {
					Ok(message) => {
						self.handle_message(message, &mut responses);
					}
					Err(e) => {
						tracing::error!("Failed to deserialize message {:?}", e)
					}
				}
			}
			EditorMessage::OpenFileDialogResult { path, content, context } => match context.0 {
				OpenFileDialogContextInner::Document => match String::from_utf8(content) {
					Ok(content) => {
						let message = PortfolioMessage::OpenDocumentFile {
							document_name: None,
							document_path: Some(path),
							document_serialized_content: content,
						};
						self.handle_message(message.into(), &mut responses);
					}
					Err(e) => {
						tracing::error!("Failed to deserialize document content: {:?}", e);
					}
				},
				OpenFileDialogContextInner::Import => {
					let extension = path.extension().and_then(|s| s.to_str());
					let name = path.file_stem().map(|s| s.to_string_lossy().to_string());
					match extension {
						Some("svg") => match String::from_utf8(content) {
							Ok(content) if !content.is_empty() => {
								let message = PortfolioMessage::PasteSvg {
									name,
									svg: content,
									mouse: None,
									parent_and_insert_index: None,
								};
								self.handle_message(message.into(), &mut responses);
							}
							Ok(_) => {
								tracing::warn!("Svg file is empty: {}", path.display());
							}
							Err(e) => {
								tracing::error!("Failed to deserialize document content: {:?}", e);
							}
						},
						Some(_) => {
							let reader = image::ImageReader::new(Cursor::new(content));
							match reader.decode() {
								Ok(image) => {
									let width = image.width();
									let height = image.height();
									let image_data = image.to_rgba8();
									let image = Image::<Color>::from_image_data(image_data.as_raw(), width, height);

									let message = PortfolioMessage::PasteImage {
										name,
										image,
										mouse: None,
										parent_and_insert_index: None,
									};
									self.handle_message(message.into(), &mut responses);
								}
								Err(e) => {
									tracing::error!("Failed to decode image: {}: {}", path.display(), e);
								}
							}
						}
						_ => {
							tracing::warn!("Unsupported file type: {}", path.display());
						}
					}
				}
			},
			EditorMessage::SaveFileDialogResult { path, context } => match context.0 {
				SaveFileDialogContextInner::Document { document_id } => {
					let message = Message::Portfolio(PortfolioMessage::DocumentPassMessage {
						document_id,
						message: DocumentMessage::SavedDocument { path: Some(path) },
					});
					self.handle_message(message, &mut responses);
				}
				SaveFileDialogContextInner::Export => {}
			},
		}
		responses
	}

	fn handle_message(&mut self, message: Message, responses: &mut Vec<NativeMessage>) {
		handle_frontend_messages(self.editor.handle_message(message), responses);
	}

	pub fn run_node_graph(&self) {}
}

fn handle_message(message: Message, responses: &mut Vec<NativeMessage>) -> Option<Message> {
	if let Message::InputPreprocessor(InputPreprocessorMessage::BoundsOfViewports { bounds_of_viewports }) = &message {
		let top_left = bounds_of_viewports[0].top_left;
		let bottom_right = bounds_of_viewports[0].bottom_right;
		responses.push(NativeMessage::UpdateViewportBounds {
			x: top_left.x as f32,
			y: top_left.y as f32,
			width: (bottom_right.x - top_left.x) as f32,
			height: (bottom_right.y - top_left.y) as f32,
		});
	}
	None
}

fn handle_frontend_messages(messages: Vec<FrontendMessage>, responses: &mut Vec<NativeMessage>) {
	let frontend_messages = messages.into_iter().filter_map(|m| handle_frontend_message(m, responses)).collect::<Vec<_>>();
	responses.push(NativeMessage::ToFrontend(ron::to_string(&frontend_messages).unwrap().into_bytes()));
}

fn handle_frontend_message(message: FrontendMessage, responses: &mut Vec<NativeMessage>) -> Option<FrontendMessage> {
	match message {
		FrontendMessage::RenderOverlays(overlay_context) => {
			responses.push(NativeMessage::UpdateOverlays(overlay_context.take_scene()));
		}
		FrontendMessage::TriggerOpenDocument => {
			responses.push(NativeMessage::OpenFileDialog {
				title: "Open Document".to_string(),
				filters: vec![FileFilter {
					name: "Graphite".to_string(),
					extensions: vec!["graphite".to_string()],
				}],
				context: OpenFileDialogContext(OpenFileDialogContextInner::Document),
			});
		}
		FrontendMessage::TriggerImport => {
			responses.push(NativeMessage::OpenFileDialog {
				title: "Import File".to_string(),
				filters: vec![
					FileFilter {
						name: "Svg".to_string(),
						extensions: vec!["svg".to_string()],
					},
					FileFilter {
						name: "Image".to_string(),
						extensions: vec!["png".to_string(), "jpg".to_string(), "jpeg".to_string(), "bmp".to_string()],
					},
				],
				context: OpenFileDialogContext(OpenFileDialogContextInner::Import),
			});
		}
		FrontendMessage::TriggerSaveDocument { document_id, name, path, content } => {
			responses.push(NativeMessage::SaveFileDialog {
				title: "Save Document".to_string(),
				default_filename: name,
				default_folder: path.and_then(|p| p.parent().map(PathBuf::from)),
				content,
				context: SaveFileDialogContext(SaveFileDialogContextInner::Document { document_id }),
			});
		}
		FrontendMessage::TriggerSaveFile { name, content } => {
			responses.push(NativeMessage::SaveFileDialog {
				title: "Save File".to_string(),
				default_filename: name,
				default_folder: None,
				content,
				context: SaveFileDialogContext(SaveFileDialogContextInner::Export),
			});
		}
		FrontendMessage::TriggerVisitLink { url } => {
			responses.push(NativeMessage::OpenUrl(url));
		}
		m => return Some(m),
	}
	None
}

pub enum NativeMessage {
	ToFrontend(Vec<u8>),
	OpenFileDialog {
		title: String,
		filters: Vec<FileFilter>,
		context: OpenFileDialogContext,
	},
	SaveFileDialog {
		title: String,
		default_filename: String,
		default_folder: Option<PathBuf>,
		content: Vec<u8>,
		context: SaveFileDialogContext,
	},
	OpenUrl(String),
	UpdateViewport(wgpu::Texture),
	UpdateViewportBounds {
		x: f32,
		y: f32,
		width: f32,
		height: f32,
	},
	UpdateOverlays(vello::Scene),
}

pub enum EditorMessage {
	FromFrontend(Vec<u8>),
	OpenFileDialogResult { path: PathBuf, content: Vec<u8>, context: OpenFileDialogContext },
	SaveFileDialogResult { path: PathBuf, context: SaveFileDialogContext },
}

pub struct FileFilter {
	pub name: String,
	pub extensions: Vec<String>,
}

pub struct OpenFileDialogContext(OpenFileDialogContextInner);
enum OpenFileDialogContextInner {
	Document,
	Import,
}

pub struct SaveFileDialogContext(SaveFileDialogContextInner);
enum SaveFileDialogContextInner {
	Document { document_id: DocumentId },
	Export,
}
