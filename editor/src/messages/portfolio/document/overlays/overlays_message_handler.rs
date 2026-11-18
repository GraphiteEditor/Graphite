use super::utility_types::{OverlayProvider, OverlaysVisibilitySettings};
use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct OverlaysMessageContext<'a> {
	pub visibility_settings: OverlaysVisibilitySettings,
	pub viewport: &'a ViewportMessageHandler,
	pub graph_open: bool,
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct OverlaysMessageHandler {
	/// In the order they were added, which is the order they draw in.
	pub overlay_providers: Vec<OverlayProvider>,
}

#[message_handler_data]
impl MessageHandler<OverlaysMessage, OverlaysMessageContext<'_>> for OverlaysMessageHandler {
	fn process_message(&mut self, message: OverlaysMessage, responses: &mut VecDeque<Message>, context: OverlaysMessageContext) {
		let OverlaysMessageContext {
			visibility_settings,
			viewport,
			graph_open,
		} = context;

		match message {
			#[cfg(not(test))]
			OverlaysMessage::Draw => self.draw(visibility_settings, viewport, graph_open, responses),
			#[cfg(test)]
			OverlaysMessage::Draw => {
				let _ = (responses, visibility_settings, viewport, graph_open);
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

impl OverlaysMessageHandler {
	fn draw(&self, visibility_settings: OverlaysVisibilitySettings, viewport: &ViewportMessageHandler, graph_open: bool, responses: &mut VecDeque<Message>) {
		use super::utility_types::OverlayContext;

		let overlay_context = OverlayContext::new(*viewport, visibility_settings);

		if visibility_settings.all() && !graph_open {
			responses.add(DocumentMessage::GridOverlays { context: overlay_context.clone() });
			responses.add(DocumentMessage::LayersPanelHoverOverlays { context: overlay_context.clone() });

			for provider in &self.overlay_providers {
				responses.add(provider(overlay_context.clone()));
			}
		}

		// Queued after the providers so the scene is complete once it's rendered
		responses.add(PortfolioMessage::RenderOverlays { context: overlay_context });
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;

	fn provider(_: OverlayContext) -> Message {
		Message::NoOp
	}

	#[test]
	fn the_document_overlays_are_cleared_and_not_drawn_while_the_graph_is_drawn_over_the_viewport() {
		let mut handler = OverlaysMessageHandler::default();
		handler.overlay_providers.push(provider);
		let drawn = |graph_open| {
			let mut responses = VecDeque::new();
			handler.draw(OverlaysVisibilitySettings::default(), &ViewportMessageHandler::default(), graph_open, &mut responses);
			responses
				.into_iter()
				.map(|message| match message {
					Message::Portfolio(PortfolioMessage::Document(DocumentMessage::GridOverlays { .. })) => "grid",
					Message::Portfolio(PortfolioMessage::Document(DocumentMessage::LayersPanelHoverOverlays { .. })) => "Layers panel hover",
					Message::NoOp => "provider",
					Message::Portfolio(PortfolioMessage::RenderOverlays { .. }) => "render",
					message => panic!("{message:?}"),
				})
				.collect::<Vec<_>>()
		};

		assert_eq!(drawn(false), ["grid", "Layers panel hover", "provider", "render"]);
		assert_eq!(drawn(true), ["render"], "the empty overlays replace those drawn before the graph opened");
	}
}
