use super::utility_types::OverlayProvider;
use crate::messages::prelude::*;

#[derive(Debug, Clone, Default)]
pub struct OverlaysMessageHandler {
	pub overlay_providers: HashSet<OverlayProvider>,
	canvas: Option<web_sys::HtmlCanvasElement>,
	context: Option<web_sys::CanvasRenderingContext2d>,
}

impl MessageHandler<OverlaysMessage, (bool, &InputPreprocessorMessageHandler)> for OverlaysMessageHandler {
	fn process_message(&mut self, message: OverlaysMessage, responses: &mut VecDeque<Message>, (overlays_visible, ipp): (bool, &InputPreprocessorMessageHandler)) {
		match message {
			#[cfg(target_arch = "wasm32")]
			OverlaysMessage::Draw => {
				use super::utility_functions::overlay_canvas_element;
				use super::utility_types::OverlayContext;
				use wasm_bindgen::JsCast;

				let canvas = self.canvas.get_or_insert_with(|| overlay_canvas_element().expect("Failed to get canvas element"));

				let context = self.context.get_or_insert_with(|| {
					let context = canvas.get_context("2d").ok().flatten().expect("Failed to get canvas context");
					context.dyn_into().expect("Context should be a canvas 2d context")
				});

				let size = ipp.viewport_bounds.size().as_uvec2();

				context.clear_rect(0., 0., ipp.viewport_bounds.size().x, ipp.viewport_bounds.size().y);

				if overlays_visible {
					responses.add(DocumentMessage::GridOverlays(OverlayContext {
						render_context: context.clone(),
						size: size.as_dvec2(),
					}));
					for provider in &self.overlay_providers {
						responses.add(provider(OverlayContext {
							render_context: context.clone(),
							size: size.as_dvec2(),
						}));
					}
				}
			}
			#[cfg(not(target_arch = "wasm32"))]
			OverlaysMessage::Draw => {
				warn!(
					"Cannot render overlays on non-Wasm targets.\n{responses:?} {overlays_visible} {ipp:?} {:?} {:?}",
					self.canvas, self.context
				);
			}
			OverlaysMessage::AddProvider(message) => {
				self.overlay_providers.insert(message);
			}
			OverlaysMessage::RemoveProvider(message) => {
				self.overlay_providers.remove(&message);
			}
		}
	}

	advertise_actions!(OverlaysMessage;);
}
