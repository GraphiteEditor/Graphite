use super::utility_types::{DataType, IngestAction, TypeFilter};
use crate::messages::frontend::utility_types::{FileDialogOptions, FileFilter};
use crate::messages::prelude::*;
use glam::IVec2;
use graph_craft::application_io::resource::ResourceId;
use graph_craft::document::value::TaggedValue;
use graphene_std::raster::Image;
use graphene_std::raster_nodes::color_lookup_table::{Lut, LutParseError};

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
			IngestMessage::SetResourceInput {
				document_id,
				node_id,
				input_index,
				filters,
			} => {
				let action = IngestAction::resource_input(document_id, node_id, input_index, &filters);

				responses.add(IngestMessage::Browse { filters, multiple: false, action });
			}
			IngestMessage::Browse { filters, multiple, action } => responses.add(FrontendMessage::TriggerBrowse {
				options: FileDialogOptions {
					filters: filters.into_iter().map(FileFilter::from).collect(),
					multiple,
				},
				action,
			}),
			IngestMessage::Ingest { data, action, mime_type, path } => {
				let data_type = DataType::detect(&data, &mime_type, path.as_deref());

				let placement = match action {
					IngestAction::ResourceInput {
						document_id,
						node_id,
						input_index,
						accepted_types,
					} => {
						if let Some(description) = rejection(&data, data_type, &accepted_types) {
							responses.add(DialogMessage::DisplayDialogError {
								title: "Unsupported file".into(),
								description: description.into(),
							});
							return;
						}

						// The file goes to the document that asked for it, which may no longer be active or open once the dialog closes
						let resource_id = ResourceId::new();
						let messages = [
							DocumentMessage::AddTransaction,
							DocumentMessage::Resource(ResourceMessage::StoreEmbedded { resource_id, data: data.into() }),
							DocumentMessage::NodeGraph(NodeGraphMessage::SetInputValue {
								node_id,
								input_index: input_index as usize,
								value: TaggedValue::Resource(resource_id).into(),
							}),
						];
						for message in messages {
							responses.add(PortfolioMessage::DocumentPassMessage { document_id, message });
						}
						return;
					}
					IngestAction::Open => None,
					IngestAction::Import | IngestAction::Paste => Some((None, None)),
					IngestAction::DropOnCanvas { mouse } => Some((Some(mouse), None)),
					IngestAction::DropOnLayers { parent, insert_index } => Some((None, Some((parent, insert_index as usize)))),
				}
				.filter(|_| context.document_open);

				let name = path.as_ref().and_then(|path| path.file_stem()).map(|stem| stem.to_string_lossy().into_owned());
				let document_path = path.filter(|path| path.is_absolute());

				let (mouse, parent_and_insert_index) = placement.unwrap_or_default();
				let place_at_origin = placement.is_none();
				let (insert, artboard_canvas) = match data_type {
					DataType::GraphiteLegacy => {
						let Ok(document_serialized_content) = String::from_utf8(data) else {
							return unsupported(responses);
						};
						responses.add(PortfolioMessage::OpenLegacyDocumentFile {
							document_name: name,
							document_path,
							document_serialized_content,
						});
						return;
					}
					DataType::Gdd => {
						responses.add(PortfolioMessage::OpenDocumentFile {
							document_name: name,
							document_path,
							content: data,
						});
						return;
					}
					DataType::Svg => {
						let Ok(svg) = String::from_utf8(data) else { return unsupported(responses) };
						let artboard_canvas = place_at_origin.then(|| svg_canvas(&svg)).flatten();
						let insert = DocumentMessage::InsertSvg {
							name: name.clone(),
							svg,
							mouse,
							parent_and_insert_index,
							place_at_origin,
						};
						(insert, artboard_canvas)
					}
					DataType::Raster(_) => {
						let Some(size) = Image::encoded_size(&data) else { return unsupported(responses) };
						let insert = DocumentMessage::InsertImage {
							name: name.clone(),
							data: data.into(),
							size: size.into(),
							mouse,
							parent_and_insert_index,
							place_at_origin,
						};
						(insert, None)
					}
					// A LUT is only ever the file of a node input
					DataType::Lut | DataType::Unknown => return unsupported(responses),
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

/// The reason a node input refuses the file, or `None` if it accepts it. An input that lists its accepted types takes an image or LUT only if it fully parses.
fn rejection(data: &[u8], data_type: DataType, accepted_types: &[DataType]) -> Option<&'static str> {
	const WRONG_TYPE: &str = "This input does not accept the format of the chosen file.";

	if accepted_types.is_empty() {
		return None;
	}
	if !accepted_types.contains(&data_type) {
		return Some(WRONG_TYPE);
	}

	match data_type {
		DataType::Raster(_) => Image::encoded_size(data).is_none().then_some("This file could not be read as an image."),
		DataType::Lut => Lut::parse(data).err().map(|error| match error {
			LutParseError::IccProfileClass => {
				"This ICC profile describes the colors of a device (like a monitor or printer) instead\n\
				of remapping colors. Only \"abstract\" and \"device link\" profiles work as LUTs."
			}
			LutParseError::IccColorSpaces => {
				"This ICC profile remaps within color spaces that are currently unsupported, such as CMYK.\n\
				A \"device link\" profile must map RGB to RGB. An \"abstract\" profile must map Lab to Lab."
			}
			LutParseError::Unreadable => "This file could not be read as a LUT. It may be corrupted or an unsupported format variant.",
		}),
		_ => None,
	}
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
	use super::*;
	use graph_craft::document::NodeId;
	use graphene_std::Color;

	const REQUESTING_DOCUMENT: DocumentId = DocumentId(3);
	const IDENTITY_CUBE: &[u8] = b"LUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n";

	fn ingest(data: &[u8], action: IngestAction, document_open: bool) -> VecDeque<Message> {
		ingest_named(data, None, action, document_open)
	}

	fn ingest_named(data: &[u8], file_name: Option<&str>, action: IngestAction, document_open: bool) -> VecDeque<Message> {
		let mut responses = VecDeque::new();
		let message = IngestMessage::Ingest {
			data: data.into(),
			action,
			mime_type: String::new(),
			path: file_name.map(Into::into),
		};
		IngestMessageHandler::default().process_message(message, &mut responses, IngestMessageContext { document_open });
		responses
	}

	fn resource_input(accepted_types: Vec<DataType>) -> IngestAction {
		IngestAction::ResourceInput {
			document_id: REQUESTING_DOCUMENT,
			node_id: NodeId(7),
			input_index: 1,
			accepted_types,
		}
	}

	fn stored_resource(message: &Message) -> Option<ResourceId> {
		match message {
			Message::Portfolio(PortfolioMessage::DocumentPassMessage {
				document_id: REQUESTING_DOCUMENT,
				message: DocumentMessage::Resource(ResourceMessage::StoreEmbedded { resource_id, .. }),
			}) => Some(*resource_id),
			_ => None,
		}
	}

	#[test]
	fn resource_input_stores_the_file_and_assigns_it_in_the_requesting_document() {
		let responses = ingest(b"any bytes", resource_input(Vec::new()), true);
		let assigned = responses.iter().find_map(|message| match message {
			Message::Portfolio(PortfolioMessage::DocumentPassMessage {
				document_id: REQUESTING_DOCUMENT,
				message: DocumentMessage::NodeGraph(NodeGraphMessage::SetInputValue { node_id, input_index, value }),
			}) => Some((*node_id, *input_index, value.clone())),
			_ => None,
		});

		let stored = responses.iter().find_map(stored_resource).expect("the file should be stored as a resource");
		assert_eq!(assigned, Some((NodeId(7), 1, TaggedValue::Resource(stored).into())));
		assert!(
			responses.iter().all(|message| matches!(message, Message::Portfolio(PortfolioMessage::DocumentPassMessage { .. }))),
			"nothing should reach whichever document happens to be active"
		);
	}

	#[test]
	fn resource_input_rejects_a_file_outside_its_types_before_storing_it() {
		let png = Image::new(8, 8, Color::WHITE).to_png();
		let raster = TypeFilter::raster().types;
		assert!(ingest(&png, resource_input(raster.clone()), true).iter().any(|message| stored_resource(message).is_some()));

		// Text, an SVG, and an image cut off after its intact header
		for rejected in [b"not an image".as_slice(), b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>".as_slice(), &png[..40]] {
			let responses = ingest(rejected, resource_input(raster.clone()), true);
			assert_eq!(responses.len(), 1, "a rejected file should not become a resource");
			assert!(matches!(responses[0], Message::Dialog(DialogMessage::DisplayDialogError { .. })), "the user should be told why");
		}
	}

	#[test]
	fn browsing_for_a_resource_input_carries_the_action_a_dropped_file_uses() {
		let filters = vec![TypeFilter::documents(), TypeFilter::raster()];
		let accepted_types = [TypeFilter::documents().types, TypeFilter::raster().types].concat();
		let dropped = IngestAction::resource_input(REQUESTING_DOCUMENT, NodeId(7), 1, &filters);
		assert_eq!(dropped, resource_input(accepted_types));

		let mut responses = VecDeque::new();
		let message = IngestMessage::SetResourceInput {
			document_id: REQUESTING_DOCUMENT,
			node_id: NodeId(7),
			input_index: 1,
			filters,
		};
		IngestMessageHandler::default().process_message(message, &mut responses, IngestMessageContext { document_open: true });
		assert!(matches!(&responses[0], Message::Portfolio(PortfolioMessage::Ingest(IngestMessage::Browse { action, .. })) if *action == dropped));
	}

	#[test]
	fn resource_input_says_why_an_icc_profile_of_the_wrong_kind_is_refused() {
		let mut monitor_profile = vec![0; 128];
		monitor_profile[12..16].copy_from_slice(b"mntr");
		monitor_profile[36..40].copy_from_slice(b"acsp");

		let refusal = |data: &[u8], file_name: &str| {
			let responses = ingest_named(data, Some(file_name), resource_input(TypeFilter::lut().types), true);
			assert_eq!(responses.len(), 1, "a refused file should only show a dialog");
			match &responses[0] {
				Message::Dialog(DialogMessage::DisplayDialogError { description, .. }) => description.clone(),
				_ => panic!("the user should be told why"),
			}
		};

		assert!(refusal(&monitor_profile, "display.icc").contains("monitor"));
		assert!(refusal(b"not a table", "grade.cube").contains("could not be read"));
		assert!(refusal(IDENTITY_CUBE, "grade.png").contains("does not accept"));
	}

	#[test]
	fn resource_input_takes_an_image_format_that_has_no_signature() {
		// A TGA file is only told apart by its name
		let tga = crate::messages::frontend::utility_types::FileType::Tga.encode(2, 1, vec![255; 8]).unwrap();
		let stores = |file_name: &str| {
			let responses = ingest_named(&tga, Some(file_name), resource_input(TypeFilter::raster().types), true);
			responses.iter().any(|message| stored_resource(message).is_some())
		};

		assert!(stores("photo.tga"));
		assert!(!stores("photo.unknown"));
	}

	#[test]
	fn resource_input_tells_a_corrupt_image_apart_from_a_wrong_type() {
		let png = Image::new(8, 8, Color::WHITE).to_png();
		let refusal = |data: &[u8]| {
			let responses = ingest(data, resource_input(TypeFilter::raster().types), true);
			assert_eq!(responses.len(), 1, "a refused file should only show a dialog");
			match &responses[0] {
				Message::Dialog(DialogMessage::DisplayDialogError { description, .. }) => description.clone(),
				_ => panic!("the user should be told why"),
			}
		};

		assert!(refusal(&png[..40]).contains("could not be read as an image"));
		assert!(refusal(b"not an image").contains("does not accept"));
	}

	#[test]
	fn resource_input_takes_a_lookup_table_only_when_it_parses() {
		let png = Image::new(8, 8, Color::WHITE).to_png();
		let stores = |data: &[u8], filter: TypeFilter| {
			let responses = ingest_named(data, Some("grade.cube"), resource_input(filter.types), true);
			responses.iter().any(|message| stored_resource(message).is_some())
		};

		assert!(stores(IDENTITY_CUBE, TypeFilter::lut()));

		// A table cut off partway, an image named as a table, and a table offered to an image input
		assert!(!stores(&IDENTITY_CUBE[..30], TypeFilter::lut()));
		assert!(!stores(&png, TypeFilter::lut()));
		assert!(!stores(IDENTITY_CUBE, TypeFilter::raster()));
	}

	#[test]
	fn lookup_table_outside_a_node_input_only_shows_a_dialog() {
		let responses = ingest_named(IDENTITY_CUBE, Some("grade.cube"), IngestAction::Import, true);
		assert_eq!(responses.len(), 1);
		assert!(matches!(responses[0], Message::Dialog(DialogMessage::DisplayDialogError { .. })));
	}

	#[test]
	fn truncated_image_only_shows_a_dialog() {
		let png = Image::new(8, 8, Color::WHITE).to_png();
		let responses = ingest(&png[..40], IngestAction::Paste, true);
		assert_eq!(responses.len(), 1);
		assert!(matches!(responses[0], Message::Dialog(DialogMessage::DisplayDialogError { .. })));
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
