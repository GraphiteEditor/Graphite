use super::utility_types::OverlayProvider;
use crate::messages::{debug, prelude::*};

pub struct OverlaysMessageData<'a> {
	pub overlays_visible: bool,
	pub overlay_artboard_tool_visible: bool,
	pub overlay_ellipse_tool_visible: bool,
	pub overlay_freehand_tool_visible: bool,
	pub overlay_gradient_tool_visible: bool,
	pub overlay_line_tool_visible: bool,
	pub overlay_path_tool_visible: bool,
	pub overlay_pen_tool_visible: bool,
	pub overlay_polygon_tool_visible: bool,
	pub overlay_rectangle_tool_visible: bool,
	pub overlay_select_tool_visible: bool,
	pub overlay_spline_tool_visible: bool,
	pub overlay_text_tool_visible: bool,
	pub overlay_transform_layer_visible: bool,
	pub ipp: &'a InputPreprocessorMessageHandler,
	pub device_pixel_ratio: f64,
}

#[derive(Debug, Clone, Default)]
pub struct OverlaysMessageHandler {
	pub overlay_providers: HashSet<OverlayProvider>,
	canvas: Option<web_sys::HtmlCanvasElement>,
	context: Option<web_sys::CanvasRenderingContext2d>,
}

impl MessageHandler<OverlaysMessage, OverlaysMessageData<'_>> for OverlaysMessageHandler {
	fn process_message(&mut self, message: OverlaysMessage, responses: &mut VecDeque<Message>, data: OverlaysMessageData) {
		let OverlaysMessageData {
			overlays_visible,
			overlay_artboard_tool_visible,
			overlay_ellipse_tool_visible,
			overlay_freehand_tool_visible,
			overlay_gradient_tool_visible,
			overlay_line_tool_visible,
			overlay_path_tool_visible,
			overlay_pen_tool_visible,
			overlay_polygon_tool_visible,
			overlay_rectangle_tool_visible,
			overlay_select_tool_visible,
			overlay_spline_tool_visible,
			overlay_text_tool_visible,
			overlay_transform_layer_visible,
			ipp,
			..
		} = data;

		match message {
			// #[cfg(target_arch = "wasm32")]
			OverlaysMessage::Draw => {
				use super::utility_functions::overlay_canvas_element;
				use super::utility_types::OverlayContext;
				use glam::{DAffine2, DVec2};
				use wasm_bindgen::JsCast;

				let device_pixel_ratio = data.device_pixel_ratio;

				let canvas = match &self.canvas {
					Some(canvas) => canvas,
					None => {
						let Some(new_canvas) = overlay_canvas_element() else { return };
						self.canvas.get_or_insert(new_canvas)
					}
				};

				let context = self.context.get_or_insert_with(|| {
					let context = canvas.get_context("2d").ok().flatten().expect("Failed to get canvas context");
					context.dyn_into().expect("Context should be a canvas 2d context")
				});

				let size = ipp.viewport_bounds.size().as_uvec2();

				let [a, b, c, d, e, f] = DAffine2::from_scale(DVec2::splat(device_pixel_ratio)).to_cols_array();
				let _ = context.set_transform(a, b, c, d, e, f);
				context.clear_rect(0., 0., ipp.viewport_bounds.size().x, ipp.viewport_bounds.size().y);
				let _ = context.reset_transform();

				if overlays_visible {
					responses.add(DocumentMessage::GridOverlays(OverlayContext {
						render_context: context.clone(),
						size: size.as_dvec2(),
						device_pixel_ratio,
					}));
					for provider in &self.overlay_providers {
						let message = provider(OverlayContext {
							render_context: context.clone(),
							size: size.as_dvec2(),
							device_pixel_ratio,
						});
						debug!("OverlaysMessageHandler: {:?}", message.clone());
						match message {
							Message::Tool(ToolMessage::Artboard(ArtboardToolMessage::Overlays(_))) => {
								if overlay_artboard_tool_visible {
									responses.add(message);
								}
							}
							Message::Tool(ToolMessage::Ellipse(EllipseToolMessage::Overlays(_))) => {
								if overlay_ellipse_tool_visible {
									responses.add(message);
								}
							}
							Message::Tool(ToolMessage::Freehand(FreehandToolMessage::Overlays(_))) => {
								if overlay_freehand_tool_visible {
									responses.add(message);
								}
							}
							Message::Tool(ToolMessage::Gradient(GradientToolMessage::Overlays(_))) => {
								if overlay_gradient_tool_visible {
									responses.add(message);
								}
							}
							Message::Tool(ToolMessage::Line(LineToolMessage::Overlays(_))) => {
								if overlay_line_tool_visible {
									responses.add(message);
								}
							}
							Message::Tool(ToolMessage::Path(PathToolMessage::Overlays(_))) => {
								if overlay_path_tool_visible {
									responses.add(message);
								}
							}
							Message::Tool(ToolMessage::Pen(PenToolMessage::Overlays(_))) => {
								if overlay_pen_tool_visible {
									responses.add(message);
								}
							}
							Message::Tool(ToolMessage::Polygon(PolygonToolMessage::Overlays(_))) => {
								if overlay_polygon_tool_visible {
									responses.add(message);
								}
							}
							Message::Tool(ToolMessage::Rectangle(RectangleToolMessage::Overlays(_))) => {
								if overlay_rectangle_tool_visible {
									responses.add(message);
								}
							}
							Message::Tool(ToolMessage::Spline(SplineToolMessage::Overlays(_))) => {
								if overlay_spline_tool_visible {
									responses.add(message);
								}
							}
							Message::Tool(ToolMessage::Select(SelectToolMessage::Overlays(_))) => {
								if overlay_select_tool_visible {
									responses.add(message);
								}
							}
							Message::Tool(ToolMessage::Text(TextToolMessage::Overlays(_))) => {
								if overlay_text_tool_visible {
									responses.add(message);
								}
							}
							Message::Tool(ToolMessage::TransformLayer(TransformLayerMessage::Overlays(_))) => {
								if overlay_transform_layer_visible {
									responses.add(message);
								}
							}
							_ => {
								responses.add(message);
							}
						}
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
