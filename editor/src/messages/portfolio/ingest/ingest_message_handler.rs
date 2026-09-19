use super::utility_types::{DataType, IngestAction, TypeFilter};
use crate::messages::frontend::utility_types::{FileDialogOptions, FileFilter};
use crate::messages::prelude::*;
use glam::IVec2;
use graph_craft::application_io::resource::ResourceId;
use graph_craft::document::value::TaggedValue;
use std::io::Cursor;

#[derive(ExtractField)]
pub struct IngestMessageContext {
	pub document_open: bool,
}

#[derive(Debug, Default, ExtractField)]
pub struct IngestMessageHandler {}

#[message_handler_data]
impl MessageHandler<IngestMessage, IngestMessageContext> for IngestMessageHandler {
	fn process_message(&mut self, message: IngestMessage, responses: &mut VecDeque<Message>, context: IngestMessageContext) {
		match message {
			IngestMessage::Open => responses.add(IngestMessage::Browse {
				filters: vec![TypeFilter::documents(), TypeFilter::image()],
				multiple: true,
				action: IngestAction::Open,
			}),
			IngestMessage::Import => responses.add(IngestMessage::Browse {
				filters: vec![TypeFilter::image()],
				multiple: false,
				action: IngestAction::Import,
			}),
			IngestMessage::SetResourceInput { node_id, input_index, filters } => responses.add(IngestMessage::Browse {
				filters,
				multiple: false,
				action: IngestAction::ResourceInput { node_id, input_index },
			}),
			IngestMessage::Browse { filters, multiple, action } => responses.add(FrontendMessage::TriggerBrowse {
				options: FileDialogOptions {
					filters: filters.into_iter().map(FileFilter::from).collect(),
					multiple,
				},
				action,
			}),
			IngestMessage::Ingest { data, action, hint, path } => {
				let placement = match action {
					IngestAction::ResourceInput { node_id, input_index } => {
						let resource_id = ResourceId::new();
						responses.add(DocumentMessage::AddTransaction);
						responses.add(ResourceMessage::StoreEmbedded { resource_id, data });
						responses.add(NodeGraphMessage::SetInputValue {
							node_id,
							input_index,
							value: TaggedValue::Resource(resource_id).into(),
						});
						return;
					}
					IngestAction::Open => None,
					IngestAction::Import | IngestAction::Paste => Some((None, None)),
					IngestAction::DropOnCanvas { mouse } => Some((Some(mouse), None)),
					IngestAction::DropOnLayers { parent, insert_index } => Some((None, Some((parent, insert_index)))),
				}
				.filter(|_| context.document_open);

				let name = path.as_ref().and_then(|path| path.file_stem()).map(|stem| stem.to_string_lossy().into_owned());
				let document_path = path.filter(|path| path.is_absolute());
				let data_type = match DataType::from_content(&data) {
					DataType::Unknown => DataType::from(hint),
					data_type => data_type,
				};

				let (mouse, parent_and_insert_index) = placement.unwrap_or_default();
				let place_at_origin = placement.is_none();
				let (insert, artboard_canvas) = match data_type {
					DataType::GraphiteLegacy => {
						let Ok(document_serialized_content) = String::from_utf8(data.to_vec()) else {
							return unsupported(responses);
						};
						responses.add(PortfolioMessage::OpenDocumentFile {
							document_name: name,
							document_path,
							document_serialized_content,
						});
						return;
					}
					DataType::Gdd => {
						responses.add(PortfolioMessage::OpenGddDocument {
							document_name: name,
							document_path,
							content: data.to_vec(),
						});
						return;
					}
					DataType::Svg => {
						let Ok(svg) = String::from_utf8(data.to_vec()) else { return unsupported(responses) };
						let artboard_canvas = svg_canvas(&svg);
						let insert = DocumentMessage::InsertSvg {
							name: name.clone(),
							svg,
							mouse,
							parent_and_insert_index,
							place_at_origin,
						};
						(insert, artboard_canvas)
					}
					DataType::Raster(format) if format.reading_enabled() => {
						let dimensions = image::ImageReader::new(Cursor::new(&data[..]))
							.with_guessed_format()
							.ok()
							.and_then(|reader| reader.into_dimensions().ok());
						let Some(size) = dimensions else { return unsupported(responses) };
						let insert = DocumentMessage::InsertImage {
							name: name.clone(),
							data,
							size: size.into(),
							mouse,
							parent_and_insert_index,
							place_at_origin,
						};
						(insert, None)
					}
					DataType::Raster(_) | DataType::Unknown => return unsupported(responses),
				};

				if !place_at_origin {
					responses.add(insert);
					return;
				}
				responses.add(PortfolioMessage::NewDocumentWithName { name: name.unwrap_or_default() });
				responses.add(insert);
				responses.add(DeferMessage::AfterGraphRun {
					messages: vec![
						DocumentMessage::WrapContentInArtboard {
							place_artboard_at_origin: true,
							artboard_canvas,
						}
						.into(),
					],
				});
				responses.add(DeferMessage::AfterNavigationReady {
					messages: vec![DocumentMessage::ZoomCanvasToFitAll.into()],
				});
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(IngestMessageDiscriminant;)
	}
}

fn unsupported(responses: &mut VecDeque<Message>) {
	responses.add(DialogMessage::DisplayDialogError {
		title: "Unsupported format".into(),
		description: "This file is not a supported document or image format.".into(),
	});
}

// The viewBox preserves the full canvas rather than the tighter bounding box of the rendered content
fn svg_canvas(svg: &str) -> Option<(IVec2, IVec2)> {
	usvg::roxmltree::Document::parse(svg)
		.ok()
		.and_then(|document| {
			let numbers: Vec<f64> = document
				.root_element()
				.attribute("viewBox")?
				.split(|character: char| character.is_ascii_whitespace() || character == ',')
				.filter_map(|number| number.parse().ok())
				.collect();
			let [x, y, width, height, ..] = numbers[..] else { return None };
			Some((IVec2::new(x.round() as i32, y.round() as i32), IVec2::new(width.round() as i32, height.round() as i32)))
		})
		.or_else(|| {
			let size = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?.size();
			Some((IVec2::ZERO, IVec2::new(size.width().round() as i32, size.height().round() as i32)))
		})
}

#[cfg(test)]
mod tests {
	use super::super::utility_types::TypeHint;
	use super::*;
	use graph_craft::document::NodeId;
	use graphene_std::Color;
	use graphene_std::raster::Image;

	fn ingest(data: &[u8], action: IngestAction, document_open: bool) -> VecDeque<Message> {
		let mut responses = VecDeque::new();
		let message = IngestMessage::Ingest {
			data: data.into(),
			action,
			hint: TypeHint::default(),
			path: None,
		};
		IngestMessageHandler::default().process_message(message, &mut responses, IngestMessageContext { document_open });
		responses
	}

	#[test]
	fn resource_input_stores_the_file_and_assigns_it() {
		let responses = ingest(b"any bytes", IngestAction::ResourceInput { node_id: NodeId(7), input_index: 1 }, true);
		let stored = responses.iter().find_map(|message| match message {
			Message::Portfolio(PortfolioMessage::Document(DocumentMessage::Resource(ResourceMessage::StoreEmbedded { resource_id, .. }))) => Some(*resource_id),
			_ => None,
		});
		let assigned = responses.iter().find_map(|message| match message {
			Message::Portfolio(PortfolioMessage::Document(DocumentMessage::NodeGraph(NodeGraphMessage::SetInputValue { node_id, input_index, value }))) => {
				Some((*node_id, *input_index, value.clone()))
			}
			_ => None,
		});
		let stored = stored.expect("the file should be stored as a resource");
		assert_eq!(assigned, Some((NodeId(7), 1, TaggedValue::Resource(stored).into())));
	}

	#[test]
	fn unsupported_file_only_shows_a_dialog() {
		let responses = ingest(b"just text", IngestAction::Open, true);
		assert_eq!(responses.len(), 1);
		assert!(matches!(responses[0], Message::Dialog(DialogMessage::DisplayDialogError { .. })));
	}

	#[test]
	fn image_without_a_document_opens_one() {
		let png = Image::new(1, 1, Color::WHITE).to_png();
		let responses = ingest(&png, IngestAction::Paste, false);
		assert!(matches!(responses[0], Message::Portfolio(PortfolioMessage::NewDocumentWithName { .. })));
		assert!(
			responses
				.iter()
				.any(|message| matches!(message, Message::Portfolio(PortfolioMessage::Document(DocumentMessage::InsertImage { place_at_origin: true, .. }))))
		);

		let responses = ingest(&png, IngestAction::Paste, true);
		assert!(matches!(
			responses[0],
			Message::Portfolio(PortfolioMessage::Document(DocumentMessage::InsertImage { place_at_origin: false, .. }))
		));
	}
}
