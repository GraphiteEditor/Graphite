use super::utility_types::{OverlayProvider, OverlaysVisibilitySettings};
use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct OverlaysMessageContext<'a> {
	pub visibility_settings: OverlaysVisibilitySettings,
	pub viewport: &'a ViewportMessageHandler,
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct OverlaysMessageHandler {
	pub overlay_providers: HashSet<OverlayProvider>,
	#[cfg(target_family = "wasm")]
	canvas: Option<web_sys::HtmlCanvasElement>,
	#[cfg(target_family = "wasm")]
	context: Option<web_sys::CanvasRenderingContext2d>,
}

#[message_handler_data]
impl MessageHandler<OverlaysMessage, OverlaysMessageContext<'_>> for OverlaysMessageHandler {
	fn process_message(&mut self, message: OverlaysMessage, responses: &mut VecDeque<Message>, context: OverlaysMessageContext) {
		let OverlaysMessageContext { visibility_settings, viewport, .. } = context;

		match message {
			#[cfg(target_family = "wasm")]
			OverlaysMessage::Draw => {
				use super::utility_functions::overlay_canvas_element;
				use super::utility_types::OverlayContext;
				use wasm_bindgen::JsCast;

				let canvas = match &self.canvas {
					Some(canvas) => canvas,
					None => {
						let Some(new_canvas) = overlay_canvas_element() else { return };
						self.canvas.get_or_insert(new_canvas)
					}
				};

				let canvas_context = self.context.get_or_insert_with(|| {
					let canvas_context = canvas.get_context("2d").ok().flatten().expect("Failed to get canvas context");
					canvas_context.dyn_into().expect("Context should be a canvas 2d context")
				});

				let size = viewport.physical_size().into_dvec2();
				canvas_context.clear_rect(0., 0., size.x, size.y);

				if visibility_settings.all() {
					responses.add(DocumentMessage::GridOverlays {
						context: OverlayContext {
							render_context: canvas_context.clone(),
							visibility_settings: visibility_settings.clone(),
							viewport: *viewport,
						},
					});
					for provider in &self.overlay_providers {
						responses.add(provider(OverlayContext {
							render_context: canvas_context.clone(),
							visibility_settings: visibility_settings.clone(),
							viewport: *viewport,
						}));
					}
				}
			}
			#[cfg(all(not(target_family = "wasm"), not(test)))]
			OverlaysMessage::Draw => {
				use super::utility_types::OverlayContext;

				let overlay_context = OverlayContext::new(*viewport, visibility_settings);

				if visibility_settings.all() {
					responses.add(DocumentMessage::GridOverlays { context: overlay_context.clone() });

					for provider in &self.overlay_providers {
						responses.add(provider(overlay_context.clone()));
					}
				}
				responses.add(FrontendMessage::RenderOverlays { context: overlay_context });
			}
			#[cfg(all(not(target_family = "wasm"), test))]
			OverlaysMessage::Draw => {
				let _ = (responses, visibility_settings, viewport);
			}
			OverlaysMessage::AddProvider { provider: message } => {
				self.overlay_providers.insert(message);
			}
			OverlaysMessage::RemoveProvider { provider: message } => {
				self.overlay_providers.remove(&message);
			}
		}
	}

	advertise_actions!(OverlaysMessage;);
}
