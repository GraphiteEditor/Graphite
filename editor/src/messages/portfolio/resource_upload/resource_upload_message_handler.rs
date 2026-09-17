use super::utility_types::{ImageResource, UNSUPPORTED_IMAGE_FILE, UploadTarget, decoded_image_size};
use crate::messages::prelude::*;
use graph_craft::application_io::resource::ResourceId;
use graph_craft::document::value::TaggedValue;

#[derive(ExtractField)]
pub struct ResourceUploadMessageContext {
	/// Whether a document is open to receive an image layer, since otherwise the image opens its own
	pub document_open: bool,
}

/// Runs the file dialog for resource uploads and stores every upload in the active document before handing it to its target.
#[derive(Debug, Default, ExtractField)]
pub struct ResourceUploadMessageHandler {
	/// Where the file from the open dialog goes once it is picked
	pending_target: Option<UploadTarget>,
}

#[message_handler_data]
impl MessageHandler<ResourceUploadMessage, ResourceUploadMessageContext> for ResourceUploadMessageHandler {
	fn process_message(&mut self, message: ResourceUploadMessage, responses: &mut VecDeque<Message>, context: ResourceUploadMessageContext) {
		match message {
			ResourceUploadMessage::RequestUpload { target } => {
				self.pending_target = Some(target);
				responses.add(FrontendMessage::TriggerUploadResource { filters: target.filters() });
			}
			ResourceUploadMessage::ReceiveUpload { name, data } => {
				let Some(target) = self.pending_target.take() else {
					log::warn!("A file was picked without a pending upload request");
					return;
				};
				responses.add(ResourceUploadMessage::Upload { name, data, target });
			}
			ResourceUploadMessage::Upload { name, data, target } => {
				let reject = |responses: &mut VecDeque<Message>, description: &str| {
					responses.add(DialogMessage::DisplayDialogError {
						title: "Unsupported image format".into(),
						description: description.into(),
					});
				};

				match target {
					UploadTarget::NodeInput { node_id, input_index, kind } => {
						if let Some(description) = kind.rejection(&data) {
							return reject(responses, description);
						}

						let resource_id = ResourceId::new();
						responses.add(DocumentMessage::AddTransaction);
						responses.add(ResourceMessage::StoreEmbedded { resource_id, data });
						responses.add(NodeGraphMessage::SetInputValue {
							node_id,
							input_index,
							value: Box::new(TaggedValue::Resource(resource_id)),
						});
					}
					UploadTarget::Layer { .. } | UploadTarget::Document => {
						let Some((width, height)) = decoded_image_size(&data) else {
							return reject(responses, UNSUPPORTED_IMAGE_FILE);
						};

						// A layer needs a document to land in, so without one the image opens its own, wrapped in an artboard once rendered
						let (mouse, parent_and_insert_index, place_at_origin) = match target {
							UploadTarget::Layer { mouse, parent_and_insert_index } if context.document_open => (mouse, parent_and_insert_index, false),
							_ => {
								// An empty name becomes the next available "Untitled Document"
								responses.add(PortfolioMessage::NewDocumentWithName {
									name: name.clone().unwrap_or_default(),
								});
								(None, None, true)
							}
						};

						let resource_id = ResourceId::new();
						responses.add(ResourceMessage::StoreEmbedded { resource_id, data });
						responses.add(DocumentMessage::InsertImage {
							name,
							image: ImageResource { resource_id, width, height },
							mouse,
							parent_and_insert_index,
							place_at_origin,
						});

						if place_at_origin {
							responses.add(DeferMessage::AfterGraphRun {
								messages: vec![
									DocumentMessage::WrapContentInArtboard {
										place_artboard_at_origin: true,
										artboard_canvas: None,
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
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(ResourceUploadMessageDiscriminant;)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::messages::portfolio::resource_upload::utility_types::ResourceFileKind;
	use graph_craft::document::NodeId;
	use graphene_std::Color;
	use graphene_std::raster::Image;

	fn upload(data: &[u8], target: UploadTarget, document_open: bool) -> VecDeque<Message> {
		let mut responses = VecDeque::new();
		let message = ResourceUploadMessage::Upload {
			name: None,
			data: data.into(),
			target,
		};
		ResourceUploadMessageHandler::default().process_message(message, &mut responses, ResourceUploadMessageContext { document_open });
		responses
	}

	fn stores_a_resource(message: &Message) -> bool {
		matches!(
			message,
			Message::Portfolio(PortfolioMessage::Document(DocumentMessage::Resource(ResourceMessage::StoreEmbedded { .. })))
		)
	}

	#[test]
	fn a_file_that_is_not_an_image_is_rejected_before_it_is_stored() {
		let target = UploadTarget::NodeInput {
			node_id: NodeId(7),
			input_index: 1,
			kind: ResourceFileKind::RasterImage,
		};
		let responses = upload(b"not an image", target, true);

		assert!(!responses.iter().any(stores_a_resource), "a rejected file should not become a resource");
		assert!(
			responses.iter().any(|message| matches!(message, Message::Dialog(DialogMessage::DisplayDialogError { .. }))),
			"the user should be told why"
		);
	}

	#[test]
	fn a_picked_file_goes_to_the_target_of_the_pending_request() {
		let target = UploadTarget::NodeInput {
			node_id: NodeId(7),
			input_index: 1,
			kind: ResourceFileKind::Any,
		};
		let mut handler = ResourceUploadMessageHandler::default();
		let context = || ResourceUploadMessageContext { document_open: true };

		let mut responses = VecDeque::new();
		handler.process_message(ResourceUploadMessage::RequestUpload { target }, &mut responses, context());
		assert!(responses.contains(&FrontendMessage::TriggerUploadResource { filters: Vec::new() }.into()), "the dialog should open");

		let mut responses = VecDeque::new();
		let picked = ResourceUploadMessage::ReceiveUpload {
			name: Some("file.bin".into()),
			data: b"bytes".as_slice().into(),
		};
		handler.process_message(picked, &mut responses, context());
		let expected = ResourceUploadMessage::Upload {
			name: Some("file.bin".into()),
			data: b"bytes".as_slice().into(),
			target,
		};
		assert!(responses.contains(&expected.into()), "the picked file should be uploaded to the requested target");
	}

	#[test]
	fn an_image_layer_opens_a_document_when_none_is_open() {
		let png = Image::new(1, 1, Color::WHITE).to_png();
		let target = UploadTarget::Layer {
			mouse: None,
			parent_and_insert_index: None,
		};
		let opens_a_document = |message: &Message| matches!(message, Message::Portfolio(PortfolioMessage::NewDocumentWithName { .. }));
		let inserts_at_origin = |message: &Message| matches!(message, Message::Portfolio(PortfolioMessage::Document(DocumentMessage::InsertImage { place_at_origin, .. })) if *place_at_origin);

		let responses = upload(&png, target, true);
		assert!(responses.iter().any(stores_a_resource), "an image should be stored as a resource");
		assert!(
			!responses.iter().any(opens_a_document) && !responses.iter().any(inserts_at_origin),
			"an open document should receive the layer"
		);

		let responses = upload(&png, target, false);
		assert!(
			responses.iter().position(opens_a_document) < responses.iter().position(stores_a_resource),
			"the document should exist before the resource is stored in it"
		);
		assert!(responses.iter().any(inserts_at_origin), "the image should sit at the origin of its new document");
	}
}
