use super::utility_functions::overlay_canvas_element;
use super::utility_types::{OverlayContext, OverlayProvider};
use crate::messages::prelude::*;

use wasm_bindgen::JsCast;

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
				let canvas = self.canvas.get_or_insert_with(|| overlay_canvas_element().expect("Failed to get canvas element"));

				let context = self.context.get_or_insert_with(|| {
					let context = canvas.get_context("2d").ok().flatten().expect("Failed to get canvas context");
					context.dyn_into().expect("Context should be a canvas 2d context")
				});

				canvas.set_width(ipp.viewport_bounds.size().x as u32);
				canvas.set_height(ipp.viewport_bounds.size().y as u32);

				context.clear_rect(0., 0., ipp.viewport_bounds.size().x, ipp.viewport_bounds.size().y);

				if overlays_visible {
					for provider in &self.overlay_providers {
						responses.add(provider(OverlayContext { render_context: context.clone() }));
					}
				}
			}
			#[cfg(not(target_arch = "wasm32"))]
			OverlaysMessage::Draw => {
				warn!("Cannot render overlays on non-Wasm targets {overlays_visible} {ipp:?}.");
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
