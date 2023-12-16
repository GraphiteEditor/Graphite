use crate::consts::{COLOR_ACCENT, MANIPULATOR_GROUP_MARKER_SIZE, PIVOT_INNER, PIVOT_OUTER};
use crate::messages::portfolio::document::overlays::overlays_message::OverlayProvider;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;
use bezier_rs::Subpath;
use core::f64::consts::PI;
use document_legacy::document::Document as DocumentLegacy;
use document_legacy::layers::style::{RenderData, ViewMode};
use glam::{DAffine2, DVec2};
use graphene_core::renderer::Quad;
use graphene_core::uuid::ManipulatorGroupId;
use wasm_bindgen::JsCast;

#[derive(PartialEq, Eq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct OverlayContext {
	// I don't know why we need to serde messages - we never use this functionality
	#[serde(skip, default = "create_render_context")]
	render_context: web_sys::CanvasRenderingContext2d,
}
// I don't know why we need to hash messages - we never use this functionality
impl core::hash::Hash for OverlayContext {
	fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
}

impl OverlayContext {
	fn accent_hex() -> String {
		format!("#{}", COLOR_ACCENT.rgb_hex())
	}

	pub fn quad(&mut self, quad: Quad) {
		self.render_context.begin_path();
		self.render_context.move_to(quad.0[3].x.round(), quad.0[3].y.round());
		for i in 0..4 {
			self.render_context.line_to(quad.0[i].x.round(), quad.0[i].y.round());
		}
		self.render_context.set_stroke_style(&wasm_bindgen::JsValue::from_str(&Self::accent_hex()));
		self.render_context.stroke();
	}

	pub fn line(&mut self, start: DVec2, end: DVec2) {
		self.render_context.begin_path();
		self.render_context.move_to(start.x.round(), start.y.round());
		self.render_context.line_to(end.x.round(), end.y.round());
		self.render_context.set_stroke_style(&wasm_bindgen::JsValue::from_str(&Self::accent_hex()));
		self.render_context.stroke();
	}

	pub fn handle(&mut self, position: DVec2, selected: bool) {
		self.render_context.begin_path();
		let position = position.round();
		self.render_context.arc(position.x, position.y, MANIPULATOR_GROUP_MARKER_SIZE / 2., 0., PI * 2.).expect("draw circle");

		let fill = if selected { Self::accent_hex() } else { "white".to_string() };
		self.render_context.set_fill_style(&wasm_bindgen::JsValue::from_str(&fill));
		self.render_context.fill();
		self.render_context.set_stroke_style(&wasm_bindgen::JsValue::from_str(&Self::accent_hex()));
		self.render_context.stroke();
	}

	pub fn square(&mut self, position: DVec2) {
		self.render_context.begin_path();
		let corner = position - DVec2::splat(MANIPULATOR_GROUP_MARKER_SIZE) / 2.;
		self.render_context
			.rect(corner.x.round(), corner.y.round(), MANIPULATOR_GROUP_MARKER_SIZE, MANIPULATOR_GROUP_MARKER_SIZE);
		self.render_context.set_fill_style(&wasm_bindgen::JsValue::from_str("white"));
		self.render_context.fill();
		self.render_context.set_stroke_style(&wasm_bindgen::JsValue::from_str(&Self::accent_hex()));
		self.render_context.stroke();
	}

	pub fn pivot(&mut self, pivot: DVec2) {
		self.render_context.begin_path();
		self.render_context.arc(pivot.x, pivot.y, PIVOT_OUTER / 2., 0., PI * 2.).expect("draw circle");
		self.render_context.set_fill_style(&wasm_bindgen::JsValue::from_str(&"white"));
		self.render_context.fill();
		self.render_context.set_stroke_style(&wasm_bindgen::JsValue::from_str(&Self::accent_hex()));
		self.render_context.stroke();

		self.render_context.begin_path();
		self.render_context.arc(pivot.x - 0.5, pivot.y - 0.5, PIVOT_INNER / 2., 0., PI * 2.).expect("draw circle");
		self.render_context.set_fill_style(&wasm_bindgen::JsValue::from_str(&Self::accent_hex()));
		self.render_context.fill();
	}

	pub fn outline<'a>(&mut self, subpaths: impl Iterator<Item = &'a Subpath<ManipulatorGroupId>>, transform: DAffine2) {
		let transform = |point| transform.transform_point2(point);
		self.render_context.begin_path();
		for subpath in subpaths {
			let mut curves = subpath.iter().peekable();
			let Some(first) = curves.peek() else {
				continue;
			};
			self.render_context.move_to(transform(first.start()).x, transform(first.start()).y);
			for curve in curves {
				match curve.handles {
					bezier_rs::BezierHandles::Linear => self.render_context.line_to(transform(curve.end()).x, transform(curve.end()).y),
					bezier_rs::BezierHandles::Quadratic { handle } => {
						self.render_context
							.quadratic_curve_to(transform(handle).x, transform(handle).y, transform(curve.end()).x, transform(curve.end()).y)
					}
					bezier_rs::BezierHandles::Cubic { handle_start, handle_end } => self.render_context.bezier_curve_to(
						transform(handle_start).x,
						transform(handle_start).y,
						transform(handle_end).x,
						transform(handle_end).y,
						transform(curve.end()).x,
						transform(curve.end()).y,
					),
				}
			}
			if subpath.closed() {
				self.render_context.close_path();
			}
		}

		self.render_context.set_stroke_style(&wasm_bindgen::JsValue::from_str(&Self::accent_hex()));
		self.render_context.stroke();
	}
}

#[derive(Debug, Clone, Default)]
pub struct OverlaysMessageHandler {
	pub overlays_document: DocumentLegacy,

	pub overlay_providers: HashSet<OverlayProvider>,
	pub canvas: Option<web_sys::HtmlCanvasElement>,
	pub render_context: Option<web_sys::CanvasRenderingContext2d>,
}

fn create_canvas() -> web_sys::HtmlCanvasElement {
	let window = web_sys::window().expect("global `window` should exist");
	let document = window.document().expect("document should exist");
	let parent = document.query_selector("div.viewport").ok().flatten().expect("viewport element should exist");
	let canvas: web_sys::HtmlCanvasElement = document.create_element("canvas").ok().expect("create canvas").dyn_into().ok().expect("is canvas");
	parent.append_child(canvas.dyn_ref().expect("canvas is node")).ok().expect("adding canvas");
	canvas
}

fn create_render_context() -> web_sys::CanvasRenderingContext2d {
	let context = create_canvas().get_context("2d").ok().flatten().expect("create 2d canvas context");
	context.dyn_into().expect("context should be a canvas rendering context")
}

impl MessageHandler<OverlaysMessage, (bool, &PersistentData, &InputPreprocessorMessageHandler)> for OverlaysMessageHandler {
	fn process_message(&mut self, message: OverlaysMessage, responses: &mut VecDeque<Message>, (overlays_visible, persistent_data, ipp): (bool, &PersistentData, &InputPreprocessorMessageHandler)) {
		match message {
			// Sub-messages
			OverlaysMessage::DispatchOperation(operation) => match self.overlays_document.handle_operation(*operation) {
				Ok(_) => responses.add(OverlaysMessage::Rerender),
				Err(e) => error!("OverlaysError: {e:?}"),
			},

			// Messages
			OverlaysMessage::ClearAllOverlays => {
				self.overlays_document = DocumentLegacy::default();
			}
			OverlaysMessage::Rerender =>
			// Render overlays
			{
				responses.add(FrontendMessage::UpdateDocumentOverlays {
					svg: if overlays_visible {
						let render_data = RenderData::new(&persistent_data.font_cache, ViewMode::Normal, Some(ipp.document_bounds()));
						self.overlays_document.render_root(&render_data)
					} else {
						String::from("")
					},
				})
			}

			#[cfg(target_arch = "wasm32")]
			OverlaysMessage::Render => {
				let canvas = self.canvas.get_or_insert_with(create_canvas);

				let render_context = self.render_context.get_or_insert_with(|| {
					let context = canvas.get_context("2d").ok().flatten().expect("create 2d canvas context");
					context.dyn_into().expect("context should be a canvas rendering context")
				});

				canvas.set_width(ipp.viewport_bounds.size().x as u32);
				canvas.set_height(ipp.viewport_bounds.size().y as u32);

				render_context.clear_rect(0., 0., ipp.viewport_bounds.size().x, ipp.viewport_bounds.size().y);

				if overlays_visible {
					for provider in &self.overlay_providers {
						responses.add(provider(OverlayContext {
							render_context: render_context.clone(),
						}));
					}
				}
			}
			#[cfg(not(target_arch = "wasm32"))]
			OverlaysMessage::Render => {
				warn!("Cannot render overlays on non wasm targets.");
			}
			OverlaysMessage::AddProvider(message) => {
				self.overlay_providers.insert(message);
			}
			OverlaysMessage::RemoveProvider(message) => {
				self.overlay_providers.remove(&message);
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(OverlaysMessageDiscriminant;
			ClearAllOverlays,
		)
	}
}
