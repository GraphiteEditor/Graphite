use crate::messages::clipboard::utility_types::{ClipboardContent, ClipboardContentRaw};
use crate::messages::prelude::*;
use graphene_std::raster::Image;
use graphite_proc_macros::{ExtractField, message_handler_data};

const CLIPBOARD_PREFIX_LAYER: &str = "graphite/layer: ";
const CLIPBOARD_PREFIX_NODES: &str = "graphite/nodes: ";
const CLIPBOARD_PREFIX_VECTOR: &str = "graphite/vector: ";

#[derive(Debug, Clone, Default, ExtractField)]
pub struct ClipboardMessageHandler {}

#[message_handler_data]
impl MessageHandler<ClipboardMessage, ()> for ClipboardMessageHandler {
	fn process_message(&mut self, message: ClipboardMessage, responses: &mut std::collections::VecDeque<Message>, _: ()) {
		match message {
			ClipboardMessage::Cut => responses.add(FrontendMessage::TriggerSelectionRead { cut: true }),
			ClipboardMessage::Copy => responses.add(FrontendMessage::TriggerSelectionRead { cut: false }),
			ClipboardMessage::Paste => responses.add(FrontendMessage::TriggerClipboardRead),
			ClipboardMessage::ReadClipboard { content } => match content {
				ClipboardContentRaw::Text(text) => {
					if let Some(layer) = text.strip_prefix(CLIPBOARD_PREFIX_LAYER) {
						responses.add(PortfolioMessage::PasteSerializedData { data: layer.to_string() });
					} else if let Some(nodes) = text.strip_prefix(CLIPBOARD_PREFIX_NODES) {
						responses.add(NodeGraphMessage::PasteNodes { serialized_nodes: nodes.to_string() });
					} else if let Some(vector) = text.strip_prefix(CLIPBOARD_PREFIX_VECTOR) {
						responses.add(PortfolioMessage::PasteSerializedVector { data: vector.to_string() });
					} else {
						responses.add(FrontendMessage::TriggerSelectionWrite { content: text });
					}
				}
				ClipboardContentRaw::Svg(svg) => {
					responses.add(PortfolioMessage::PasteSvg {
						svg,
						name: None,
						mouse: None,
						parent_and_insert_index: None,
					});
				}
				ClipboardContentRaw::Image { data, width, height } => {
					responses.add(PortfolioMessage::PasteImage {
						image: Image::from_image_data(&data, width, height),
						name: None,
						mouse: None,
						parent_and_insert_index: None,
					});
				}
			},
			ClipboardMessage::ReadSelection { content, cut } => {
				use crate::messages::portfolio::document::utility_types::clipboards::Clipboard;
				if let Some(text) = content {
					responses.add(ClipboardMessage::Write {
						content: ClipboardContent::Text(text),
					});
				} else if cut {
					responses.add(PortfolioMessage::Cut { clipboard: Clipboard::Device });
				} else {
					responses.add(PortfolioMessage::Copy { clipboard: Clipboard::Device });
				}
			}
			ClipboardMessage::Write { content } => {
				let text = match content {
					ClipboardContent::Svg(_) => {
						log::error!("SVG copying is not yet supported");
						return;
					}
					ClipboardContent::Image { .. } => {
						log::error!("Image copying is not yet supported");
						return;
					}
					ClipboardContent::Layer(layer) => format!("{CLIPBOARD_PREFIX_LAYER}{layer}"),
					ClipboardContent::Nodes(nodes) => format!("{CLIPBOARD_PREFIX_NODES}{nodes}"),
					ClipboardContent::Vector(vector) => format!("{CLIPBOARD_PREFIX_VECTOR}{vector}"),
					ClipboardContent::Text(text) => text,
				};
				responses.add(FrontendMessage::TriggerClipboardWrite { content: text });
			}
		}
	}
	advertise_actions!(ClipboardMessageDiscriminant;
		Cut,
		Copy,
		Paste,
	);
}
