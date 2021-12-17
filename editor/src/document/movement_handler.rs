pub use super::layer_panel::*;

use super::{DocumentMessage, LayerData};

use crate::message_prelude::*;
use crate::{
	consts::{VIEWPORT_SCROLL_RATE, VIEWPORT_ZOOM_LEVELS, VIEWPORT_ZOOM_MOUSE_RATE, VIEWPORT_ZOOM_SCALE_MAX, VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_WHEEL_RATE},
	input::{mouse::ViewportBounds, mouse::ViewportPosition, InputPreprocessor},
};
use glam::DVec2;
use graphene::document::Document;
use graphene::Operation as DocumentOperation;

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[impl_message(Message, DocumentMessage, Movement)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum MovementMessage {
	MouseMove,
	TranslateCanvasBegin,
	WheelCanvasTranslate { use_y_as_x: bool },
	RotateCanvasBegin { snap: bool },
	EnableSnapping,
	DisableSnapping,
	ZoomCanvasBegin,
	TransformCanvasEnd,
	SetCanvasRotation(f64),
	SetCanvasZoom(f64),
	IncreaseCanvasZoom,
	DecreaseCanvasZoom,
	WheelCanvasZoom,
	ZoomCanvasToFitAll,
	TranslateCanvas(DVec2),
	TranslateCanvasByViewportFraction(DVec2),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MovementMessageHandler {
	translating: bool,
	rotating: bool,
	zooming: bool,
	snapping: bool,
	mouse_pos: ViewportPosition,
}

impl MovementMessageHandler {
	fn create_document_transform_from_layerdata(&self, layerdata: &LayerData, viewport_bounds: &ViewportBounds, responses: &mut VecDeque<Message>) {
		let half_viewport = viewport_bounds.size() / 2.;
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
			TransformCanvasEnd => {
				layerdata.rotation = layerdata.snapped_angle();
				layerdata.snap_rotate = false;
				self.translating = false;
				self.rotating = false;
				self.zooming = false;
			}
			MouseMove => {
				if self.translating {
					let delta = ipp.mouse.position - self.mouse_pos;
					let transformed_delta = document.root.transform.inverse().transform_vector2(delta);

					layerdata.translation += transformed_delta;
					responses.push_back(ToolMessage::SelectedLayersChanged.into());
					self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_bounds, responses);
				}
				if self.rotating {
					let half_viewport = ipp.viewport_bounds.size() / 2.;
					let rotation = {
						let start_vec = self.mouse_pos - half_viewport;
						let end_vec = ipp.mouse.position - half_viewport;
						start_vec.angle_between(end_vec)
					};

					let snapping = self.snapping;

					layerdata.rotation += rotation;
					layerdata.snap_rotate = snapping;
					responses.push_back(ToolMessage::SelectedLayersChanged.into());
					responses.push_back(
						FrontendMessage::SetCanvasRotation {
							new_radians: layerdata.snapped_angle(),
						}
						.into(),
					);
					self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_bounds, responses);
				}
				if self.zooming {
					let difference = self.mouse_pos.y as f64 - ipp.mouse.position.y as f64;
					let amount = 1. + difference * VIEWPORT_ZOOM_MOUSE_RATE;

					let new = (layerdata.scale * amount).clamp(VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_SCALE_MAX);
					layerdata.scale = new;
					responses.push_back(FrontendMessage::SetCanvasZoom { new_zoom: layerdata.scale }.into());
					responses.push_back(ToolMessage::SelectedLayersChanged.into());
					self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_bounds, responses);
				}
				self.mouse_pos = ipp.mouse.position;
			}
			SetCanvasZoom(new) => {
				layerdata.scale = new.clamp(VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_SCALE_MAX);
				responses.push_back(FrontendMessage::SetCanvasZoom { new_zoom: layerdata.scale }.into());
				responses.push_back(ToolMessage::SelectedLayersChanged.into());
				responses.push_back(DocumentMessage::DispatchOperation(Box::from(DocumentOperation::SetViewMode{mode: data.1.view_mode()})).into());
				self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_bounds, responses);
			}
			IncreaseCanvasZoom => {
				layerdata.scale = *VIEWPORT_ZOOM_LEVELS.iter().find(|scale| **scale > layerdata.scale).unwrap_or(&layerdata.scale);
				responses.push_back(FrontendMessage::SetCanvasZoom { new_zoom: layerdata.scale }.into());
				responses.push_back(ToolMessage::SelectedLayersChanged.into());
				responses.push_back(DocumentMessage::DispatchOperation(Box::from(DocumentOperation::SetViewMode{mode: data.1.view_mode()})).into());
				self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_bounds, responses);
			}
			DecreaseCanvasZoom => {
				layerdata.scale = *VIEWPORT_ZOOM_LEVELS.iter().rev().find(|scale| **scale < layerdata.scale).unwrap_or(&layerdata.scale);
				responses.push_back(FrontendMessage::SetCanvasZoom { new_zoom: layerdata.scale }.into());
				responses.push_back(ToolMessage::SelectedLayersChanged.into());
				responses.push_back(DocumentMessage::DispatchOperation(Box::from(DocumentOperation::SetViewMode{mode: data.1.view_mode()})).into());
				self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_bounds, responses);
			}
			WheelCanvasZoom => {
				let scroll = ipp.mouse.scroll_delta.scroll_delta();
				let mouse = ipp.mouse.position;
				let viewport_bounds = ipp.viewport_bounds.size();
				let mut zoom_factor = 1. + scroll.abs() * VIEWPORT_ZOOM_WHEEL_RATE;
				if ipp.mouse.scroll_delta.y > 0 {
					zoom_factor = 1. / zoom_factor
				};
				let new_viewport_bounds = viewport_bounds * (1. / zoom_factor);
				let delta_size = viewport_bounds - new_viewport_bounds;
				let mouse_percent = mouse / viewport_bounds;
				let delta = (delta_size * -2.) * (mouse_percent - DVec2::splat(0.5));

				let transformed_delta = document.root.transform.inverse().transform_vector2(delta);
				let new = (layerdata.scale * zoom_factor).clamp(VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_SCALE_MAX);
				layerdata.scale = new;
				layerdata.translation += transformed_delta;
				responses.push_back(FrontendMessage::SetCanvasZoom { new_zoom: layerdata.scale }.into());
				responses.push_back(ToolMessage::SelectedLayersChanged.into());
				responses.push_back(DocumentMessage::DispatchOperation(Box::from(DocumentOperation::SetViewMode{mode: data.1.view_mode()})).into());
				self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_bounds, responses);
			}
			WheelCanvasTranslate { use_y_as_x } => {
				let delta = match use_y_as_x {
					false => -ipp.mouse.scroll_delta.as_dvec2(),
					true => (-ipp.mouse.scroll_delta.y as f64, 0.).into(),
				} * VIEWPORT_SCROLL_RATE;
				let transformed_delta = document.root.transform.inverse().transform_vector2(delta);
				layerdata.translation += transformed_delta;
				responses.push_back(ToolMessage::SelectedLayersChanged.into());
				self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_bounds, responses);
			}
			SetCanvasRotation(new) => {
				layerdata.rotation = new;
				self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_bounds, responses);
				responses.push_back(FrontendMessage::SetCanvasRotation { new_radians: new }.into());
				responses.push_back(ToolMessage::SelectedLayersChanged.into());
			}
			ZoomCanvasToFitAll => {
				if let Some([pos1, pos2]) = document.visible_layers_bounding_box() {
					let pos1 = document.root.transform.inverse().transform_point2(pos1);
					let pos2 = document.root.transform.inverse().transform_point2(pos2);
					let v1 = document.root.transform.inverse().transform_point2(DVec2::ZERO);
					let v2 = document.root.transform.inverse().transform_point2(ipp.viewport_bounds.size());

					let center = v1.lerp(v2, 0.5) - pos1.lerp(pos2, 0.5);
					let size = (pos2 - pos1) / (v2 - v1);
					let size = 1. / size;
					let new_scale = size.min_element();

					layerdata.translation += center;
					layerdata.scale *= new_scale;
					responses.push_back(FrontendMessage::SetCanvasZoom { new_zoom: layerdata.scale }.into());
					responses.push_back(ToolMessage::SelectedLayersChanged.into());
					responses.push_back(DocumentMessage::DispatchOperation(Box::from(DocumentOperation::SetViewMode{mode: data.1.view_mode()})).into());
					self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_bounds, responses);
				}
			}
			TranslateCanvas(delta) => {
				let transformed_delta = document.root.transform.inverse().transform_vector2(delta);

				layerdata.translation += transformed_delta;
				responses.push_back(ToolMessage::SelectedLayersChanged.into());
				self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_bounds, responses);
			}
			TranslateCanvasByViewportFraction(delta) => {
				let transformed_delta = document.root.transform.inverse().transform_vector2(delta * ipp.viewport_bounds.size());

				layerdata.translation += transformed_delta;
				responses.push_back(ToolMessage::SelectedLayersChanged.into());
				self.create_document_transform_from_layerdata(layerdata, &ipp.viewport_bounds, responses);
			}
		}
	}
	fn actions(&self) -> ActionList {
		let mut common = actions!(MovementMessageDiscriminant;
			MouseMove,
			TranslateCanvasBegin,
			RotateCanvasBegin,
			ZoomCanvasBegin,
			SetCanvasZoom,
			SetCanvasRotation,
			WheelCanvasZoom,
			IncreaseCanvasZoom,
			DecreaseCanvasZoom,
			WheelCanvasTranslate,
			ZoomCanvasToFitAll,
			TranslateCanvas,
			TranslateCanvasByViewportFraction,
		);

		if self.rotating {
			let snapping = actions!(MovementMessageDiscriminant;
				EnableSnapping,
				DisableSnapping,
			);
			common.extend(snapping);
		}
		if self.translating || self.rotating || self.zooming {
			let transforming = actions!(MovementMessageDiscriminant;
				TransformCanvasEnd,
			);
			common.extend(transforming);
		}
		common
	}
}
