use super::utility_types::{OverlayProvider, OverlaysVisibilitySettings};
use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct OverlaysMessageContext<'a> {
	pub visibility_settings: OverlaysVisibilitySettings,
	pub viewport: &'a ViewportMessageHandler,
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct OverlaysMessageHandler {
	/// In the order they were added, which is the order they draw in.
	pub overlay_providers: Vec<OverlayProvider>,
}

#[message_handler_data]
impl MessageHandler<OverlaysMessage, OverlaysMessageContext<'_>> for OverlaysMessageHandler {
	fn process_message(&mut self, message: OverlaysMessage, responses: &mut VecDeque<Message>, context: OverlaysMessageContext) {
		let OverlaysMessageContext { visibility_settings, viewport, .. } = context;

		match message {
			#[cfg(not(test))]
			OverlaysMessage::Draw => {
				use super::utility_types::OverlayContext;

				let overlay_context = OverlayContext::new(*viewport, visibility_settings);

				if visibility_settings.all() {
					responses.add(DocumentMessage::GridOverlays { context: overlay_context.clone() });
					responses.add(DocumentMessage::LayersPanelHoverOverlays { context: overlay_context.clone() });

					for provider in &self.overlay_providers {
						responses.add(provider(overlay_context.clone()));
					}
				}

				// Queued after the providers so the scene is complete once it's rendered
				responses.add(PortfolioMessage::RenderOverlays { context: overlay_context });
			}
			#[cfg(test)]
			OverlaysMessage::Draw => {
				let _ = (responses, visibility_settings, viewport);
			}
			OverlaysMessage::AddProvider { provider: message } => {
				// A provider added again moves to the end, so it draws on top from then on
				self.overlay_providers.retain(|provider| !std::ptr::fn_addr_eq(*provider, message));
				self.overlay_providers.push(message);
			}
			OverlaysMessage::RemoveProvider { provider: message } => {
				self.overlay_providers.retain(|provider| !std::ptr::fn_addr_eq(*provider, message));
			}
		}
	}

	advertise_actions!(OverlaysMessage;);
}
