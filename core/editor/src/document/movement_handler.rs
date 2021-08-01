pub use super::layer_panel::*;

use super::LayerData;

use crate::message_prelude::*;
use crate::{
	consts::{MOUSE_ZOOM_RATE, VIEWPORT_SCROLL_RATE, VIEWPORT_ZOOM_SCALE_MAX, VIEWPORT_ZOOM_SCALE_MIN, WHEEL_ZOOM_RATE},
	input::{mouse::ViewportPosition, InputPreprocessor},
};
use document_core::document::Document;
use document_core::Operation as DocumentOperation;

use std::collections::VecDeque;

#[impl_message(Message, DocumentMessage, Movement)]
#[derive(PartialEq, Clone, Debug)]
pub enum MovementMessage {
	MouseMove,
	TranslateCanvasBegin,
	WheelCanvasTranslate { use_y_as_x: bool },
	RotateCanvasBegin { snap: bool },
	EnableSnapping,
	DisableSnapping,
	ZoomCanvasBegin,
	TranslateCanvasEnd,
	SetCanvasZoom(f64),
	MultiplyCanvasZoom(f64),
	WheelCanvasZoom,
	SetCanvasRotation(f64),
}

#[derive(Debug, Clone, Hash, Default, PartialEq)]
pub struct MovementMessageHandler {
	translating: bool,
	rotating: bool,
	zooming: bool,
	snapping: bool,
	mouse_pos: ViewportPosition,
}

impl MovementMessageHandler {
	fn create_document_transform_from_layerdata(&self, layerdata: &LayerData, viewport_size: &ViewportPosition, responses: &mut VecDeque<Message>) {
		let half_viewport = viewport_size.as_dvec2() / 2.;
		let scaled_half_viewport = half_viewport / layerdata.scale;
		responses.push_back(
			DocumentOperation::SetLayerTransform {
				path: vec![],
				transform: layerdata.calculate_offset_transform(scaled_half_viewport).to_cols_array(),
			}
			.into(),
		);
	}
}

impl MessageHandler<MovementMessage, (&mut LayerData, &Document, &InputPreprocessor)> for MovementMessageHandler {
	fn process_action(&mut self, message: MovementMessage, data: (&mut LayerData, &Document, &InputPreprocessor), responses: &mut VecDeque<Message>) {
		let (layerdata, document, ipp) = data;
		use MovementMessage::*;
		match message {
			TranslateCanvasBegin => {
				self.translating = true;
				self.mouse_pos = ipp.mouse.position;
			}

			RotateCanvasBegin { snap } => {
				self.rotating = true;
				self.snapping = snap;
				layerdata.snap_rotate = snap;
				self.mouse_pos = ipp.mouse.position;
			}
			EnableSnapping => self.snapping = true,
			DisableSnapping => self.snapping = false,
			ZoomCanvasBegin => {
				self.zooming = true;
				self.mouse_pos = ipp.mouse.position;
			}
			TranslateCanvasEnd => {
				layerdata.rotation = layerdata.snapped_angle();
				layerdata.snap_rotate = false;
				self.translating = false;
				self.rotating = false;
				self.zooming = false;
			}
			MouseMove => {
				if self.translating {
					let delta = ipp.mouse.position.as_dvec2() - self.mouse_pos.as_dvec2();
					let transformed_delta = document.root.transform.inverse().transform_vector2(delta);

					layerdata.translation += transformed_delta;
					self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_size, responses);
				}
				if self.rotating {
					let half_viewport = ipp.viewport_size.as_dvec2() / 2.;
					let rotation = {
						let start_vec = self.mouse_pos.as_dvec2() - half_viewport;
						let end_vec = ipp.mouse.position.as_dvec2() - half_viewport;
						start_vec.angle_between(end_vec)
					};

					let snapping = self.snapping;

					layerdata.rotation += rotation;
					layerdata.snap_rotate = snapping;
					responses.push_back(
						FrontendMessage::SetCanvasRotation {
							new_radians: layerdata.snapped_angle(),
						}
						.into(),
					);
					self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_size, responses);
				}
				if self.zooming {
					let difference = self.mouse_pos.y as f64 - ipp.mouse.position.y as f64;
					let amount = 1. + difference * MOUSE_ZOOM_RATE;

					let new = (layerdata.scale * amount).clamp(VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_SCALE_MAX);
					layerdata.scale = new;
					responses.push_back(FrontendMessage::SetCanvasZoom { new_zoom: layerdata.scale }.into());
					self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_size, responses);
				}
				self.mouse_pos = ipp.mouse.position;
			}
			SetCanvasZoom(new) => {
				layerdata.scale = new.clamp(VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_SCALE_MAX);
				responses.push_back(FrontendMessage::SetCanvasZoom { new_zoom: layerdata.scale }.into());
				self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_size, responses);
			}
			MultiplyCanvasZoom(multiplier) => {
				let new = (layerdata.scale * multiplier).clamp(VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_SCALE_MAX);
				layerdata.scale = new;
				responses.push_back(FrontendMessage::SetCanvasZoom { new_zoom: layerdata.scale }.into());
				self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_size, responses);
			}
			WheelCanvasZoom => {
				let scroll = ipp.mouse.scroll_delta.scroll_delta();
				let mouse = ipp.mouse.position.as_dvec2();
				let viewport_size = ipp.viewport_size.as_dvec2();
				let mut zoom_factor = 1. + scroll.abs() * WHEEL_ZOOM_RATE;
				if ipp.mouse.scroll_delta.y > 0 {
					zoom_factor = 1. / zoom_factor
				};
				let new_viewport_size = viewport_size * (1. / zoom_factor);
				let delta_size = viewport_size - new_viewport_size;
				let mouse_percent = mouse / viewport_size;
				let delta = delta_size * -2. * (mouse_percent - (0.5, 0.5).into());

				let transformed_delta = document.root.transform.inverse().transform_vector2(delta);
				let new = (layerdata.scale * zoom_factor).clamp(VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_SCALE_MAX);
				layerdata.scale = new;
				layerdata.translation += transformed_delta;
				responses.push_back(FrontendMessage::SetCanvasZoom { new_zoom: layerdata.scale }.into());
				self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_size, responses);
			}
			WheelCanvasTranslate { use_y_as_x } => {
				let delta = match use_y_as_x {
					false => -ipp.mouse.scroll_delta.as_dvec2(),
					true => (-ipp.mouse.scroll_delta.y as f64, 0.).into(),
				} * VIEWPORT_SCROLL_RATE;
				let transformed_delta = document.root.transform.inverse().transform_vector2(delta);
				layerdata.translation += transformed_delta;
				self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_size, responses);
			}
			SetCanvasRotation(new) => {
				layerdata.rotation = new;
				self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_size, responses);
				responses.push_back(FrontendMessage::SetCanvasRotation { new_radians: new }.into());
			}
		}
	}
	fn actions(&self) -> ActionList {
		let mut common = actions!(MovementMessageDiscriminant;
			MouseMove,
			TranslateCanvasEnd,
			TranslateCanvasBegin,
			RotateCanvasBegin,
			ZoomCanvasBegin,
			SetCanvasZoom,
			MultiplyCanvasZoom,
			SetCanvasRotation,
			WheelCanvasZoom,
			WheelCanvasTranslate,
		);

		if self.rotating {
			let snapping = actions!(MovementMessageDiscriminant;
				EnableSnapping,
				DisableSnapping,
			);
			common.extend(snapping);
		}
		common
	}
}
