use crate::data_transfer;
use crate::helpers::render_image_data_to_canvases;
use editor::messages::prelude::FrontendMessage;

pub(crate) fn intercept_frontend_message(message: FrontendMessage) -> Option<FrontendMessage> {
	match message {
		FrontendMessage::UpdateImageData { image_data, canvas_ids_in_use } => {
			let images = image_data.iter().map(|image| (image.id, image.width, image.height, image.pixels.as_slice()));
			render_image_data_to_canvases(images, &canvas_ids_in_use);
		}
		FrontendMessage::TriggerClipboardRead => {
			data_transfer::read_clipboard();
		}
		FrontendMessage::TriggerClipboardWrite { content } => {
			data_transfer::write_clipboard(content);
		}
		m => return Some(m),
	}
	None
}
