use crate::consts::{
	VIEWPORT_ROTATE_SNAP_INTERVAL, VIEWPORT_SCROLL_RATE, VIEWPORT_ZOOM_LEVELS, VIEWPORT_ZOOM_MIN_FRACTION_COVER, VIEWPORT_ZOOM_MOUSE_RATE, VIEWPORT_ZOOM_SCALE_MAX, VIEWPORT_ZOOM_SCALE_MIN,
	VIEWPORT_ZOOM_TO_FIT_PADDING_SCALE_FACTOR, VIEWPORT_ZOOM_WHEEL_RATE,
};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, KeysGroup};
use crate::messages::input_mapper::utility_types::input_mouse::{ViewportBounds, ViewportPosition};
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use document_legacy::document::Document;
use document_legacy::Operation as DocumentOperation;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NavigationMessageHandler {
	pub pan: DVec2,
	panning: bool,
	snap_tilt: bool,
	snap_tilt_released: bool,

	pub tilt: f64,
	tilting: bool,

	pub zoom: f64,
	zooming: bool,
	snap_zoom: bool,

	mouse_position: ViewportPosition,
}

impl Default for NavigationMessageHandler {
	fn default() -> Self {
		Self {
			pan: DVec2::ZERO,
			panning: false,
			snap_tilt: false,
			snap_tilt_released: false,

			tilt: 0.,
			tilting: false,

			zoom: 1.,
			zooming: false,
			snap_zoom: false,

			mouse_position: ViewportPosition::default(),
		}
	}
}

impl MessageHandler<NavigationMessage, (&Document, Option<[DVec2; 2]>, &InputPreprocessorMessageHandler, Option<[DVec2; 2]>)> for NavigationMessageHandler {
	#[remain::check]
	fn process_message(
		&mut self,
		message: NavigationMessage,
		responses: &mut VecDeque<Message>,
		(document, document_bounds, ipp, selection_bounds): (&Document, Option<[DVec2; 2]>, &InputPreprocessorMessageHandler, Option<[DVec2; 2]>),
	) {
		use NavigationMessage::*;

		let old_zoom = self.zoom;

		#[remain::sorted]
		match message {
			DecreaseCanvasZoom { center_on_mouse } => {
				let new_scale = *VIEWPORT_ZOOM_LEVELS.iter().rev().find(|scale| **scale < self.zoom).unwrap_or(&self.zoom);
				if center_on_mouse {
					responses.add(self.center_zoom(ipp.viewport_bounds.size(), new_scale / self.zoom, ipp.mouse.position));
				}
				responses.add(SetCanvasZoom { zoom_factor: new_scale });
			}
			FitViewportToBounds {
				bounds: [bounds_corner_a, bounds_corner_b],
				padding_scale_factor,
				prevent_zoom_past_100,
			} => {
				let pos1 = document.root.transform.inverse().transform_point2(bounds_corner_a);
				let pos2 = document.root.transform.inverse().transform_point2(bounds_corner_b);
				let v1 = document.root.transform.inverse().transform_point2(DVec2::ZERO);
				let v2 = document.root.transform.inverse().transform_point2(ipp.viewport_bounds.size());

				let center = v1.lerp(v2, 0.5) - pos1.lerp(pos2, 0.5);
				let size = (pos2 - pos1) / (v2 - v1);
				let size = 1. / size;
				let new_scale = size.min_element();

				self.pan += center;
				self.zoom *= new_scale;

				self.zoom /= padding_scale_factor.unwrap_or(1.) as f64;

				if self.zoom > 1. && prevent_zoom_past_100 {
					self.zoom = 1.
				}

				responses.add(BroadcastEvent::DocumentIsDirty);
				responses.add(DocumentMessage::DirtyRenderDocumentInOutlineView);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
				self.create_document_transform(&ipp.viewport_bounds, responses);
			}
			FitViewportToSelection => {
				if let Some(bounds) = selection_bounds {
					responses.add(FitViewportToBounds {
						bounds,
						padding_scale_factor: Some(VIEWPORT_ZOOM_TO_FIT_PADDING_SCALE_FACTOR),
						prevent_zoom_past_100: false,
					})
				}
			}
			IncreaseCanvasZoom { center_on_mouse } => {
				let new_scale = *VIEWPORT_ZOOM_LEVELS.iter().find(|scale| **scale > self.zoom).unwrap_or(&self.zoom);
				if center_on_mouse {
					responses.add(self.center_zoom(ipp.viewport_bounds.size(), new_scale / self.zoom, ipp.mouse.position));
				}
				responses.add(SetCanvasZoom { zoom_factor: new_scale });
			}
			PointerMove {
				snap_angle,
				wait_for_snap_angle_release,
				snap_zoom,
				zoom_from_viewport,
			} => {
				if self.panning {
					let delta = ipp.mouse.position - self.mouse_position;

					responses.add(TranslateCanvas { delta });
				}

				if self.tilting {
					let new_snap = ipp.keyboard.get(snap_angle as usize);
					if !(wait_for_snap_angle_release && new_snap && !self.snap_tilt_released) {
						// When disabling snap, keep the viewed rotation as it was previously.
						if !new_snap && self.snap_tilt {
							self.tilt = self.snapped_angle();
						}
						self.snap_tilt = new_snap;
						self.snap_tilt_released = true;
					}

					let half_viewport = ipp.viewport_bounds.size() / 2.;
					let rotation = {
						let start_offset = self.mouse_position - half_viewport;
						let end_offset = ipp.mouse.position - half_viewport;
						start_offset.angle_between(end_offset)
					};

					responses.add(SetCanvasRotation { angle_radians: self.tilt + rotation });
				}

				if self.zooming {
					let zoom_start = self.snapped_scale();

					let new_snap = ipp.keyboard.get(snap_zoom as usize);
					// When disabling snap, keep the viewed zoom as it was previously
					if !new_snap && self.snap_zoom {
						self.zoom = self.snapped_scale();
					}
					self.snap_zoom = new_snap;

					let difference = self.mouse_position.y - ipp.mouse.position.y;
					let amount = 1. + difference * VIEWPORT_ZOOM_MOUSE_RATE;

					self.zoom *= amount;
					self.zoom *= Self::clamp_zoom(self.zoom, document_bounds, old_zoom, ipp);

					if let Some(mouse) = zoom_from_viewport {
						let zoom_factor = self.snapped_scale() / zoom_start;

						responses.add(SetCanvasZoom { zoom_factor: self.zoom });
						responses.add(self.center_zoom(ipp.viewport_bounds.size(), zoom_factor, mouse));
					} else {
						responses.add(SetCanvasZoom { zoom_factor: self.zoom });
					}
				}

				self.mouse_position = ipp.mouse.position;
			}
			RotateCanvasBegin => {
				responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
				responses.add(FrontendMessage::UpdateInputHints {
					hint_data: HintData(vec![HintGroup(vec![HintInfo {
						key_groups: vec![KeysGroup(vec![Key::Control]).into()],
						key_groups_mac: None,
						mouse: None,
						label: String::from("Snap 15°"),
						plus: false,
					}])]),
				});

				self.tilting = true;
				self.mouse_position = ipp.mouse.position;
			}
			SetCanvasRotation { angle_radians } => {
				self.tilt = angle_radians;
				self.create_document_transform(&ipp.viewport_bounds, responses);
				responses.add(BroadcastEvent::DocumentIsDirty);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			SetCanvasZoom { zoom_factor } => {
				self.zoom = zoom_factor.clamp(VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_SCALE_MAX);
				self.zoom *= Self::clamp_zoom(self.zoom, document_bounds, old_zoom, ipp);
				responses.add(BroadcastEvent::DocumentIsDirty);
				responses.add(DocumentMessage::DirtyRenderDocumentInOutlineView);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
				self.create_document_transform(&ipp.viewport_bounds, responses);
			}
			TransformCanvasEnd => {
				self.tilt = self.snapped_angle();
				self.zoom = self.snapped_scale();
				responses.add(BroadcastEvent::DocumentIsDirty);
				responses.add(ToolMessage::UpdateCursor);
				responses.add(ToolMessage::UpdateHints);
				self.snap_tilt = false;
				self.snap_tilt_released = false;
				self.snap_zoom = false;
				self.panning = false;
				self.tilting = false;
				self.zooming = false;
			}
			TranslateCanvas { delta } => {
				let transformed_delta = document.root.transform.inverse().transform_vector2(delta);

				self.pan += transformed_delta;
				responses.add(BroadcastEvent::DocumentIsDirty);
				self.create_document_transform(&ipp.viewport_bounds, responses);
			}
			TranslateCanvasBegin => {
				responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Grabbing });
				responses.add(FrontendMessage::UpdateInputHints { hint_data: HintData(Vec::new()) });

				self.panning = true;
				self.mouse_position = ipp.mouse.position;
			}
			TranslateCanvasByViewportFraction { delta } => {
				let transformed_delta = document.root.transform.inverse().transform_vector2(delta * ipp.viewport_bounds.size());

				self.pan += transformed_delta;
				responses.add(BroadcastEvent::DocumentIsDirty);
				self.create_document_transform(&ipp.viewport_bounds, responses);
			}
			WheelCanvasTranslate { use_y_as_x } => {
				let delta = match use_y_as_x {
					false => -ipp.mouse.scroll_delta.as_dvec2(),
					true => (-ipp.mouse.scroll_delta.y as f64, 0.).into(),
				} * VIEWPORT_SCROLL_RATE;
				responses.add(TranslateCanvas { delta });
			}
			WheelCanvasZoom => {
				let scroll = ipp.mouse.scroll_delta.scroll_delta();
				let mut zoom_factor = 1. + scroll.abs() * VIEWPORT_ZOOM_WHEEL_RATE;
				if ipp.mouse.scroll_delta.y > 0 {
					zoom_factor = 1. / zoom_factor
				}
				zoom_factor *= Self::clamp_zoom(self.zoom * zoom_factor, document_bounds, old_zoom, ipp);

				responses.add(self.center_zoom(ipp.viewport_bounds.size(), zoom_factor, ipp.mouse.position));
				responses.add(SetCanvasZoom { zoom_factor: self.zoom * zoom_factor });
			}
			ZoomCanvasBegin => {
				responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::ZoomIn });
				responses.add(FrontendMessage::UpdateInputHints {
					hint_data: HintData(vec![HintGroup(vec![HintInfo {
						key_groups: vec![KeysGroup(vec![Key::Control]).into()],
						key_groups_mac: None,
						mouse: None,
						label: String::from("Snap Increments"),
						plus: false,
					}])]),
				});

				self.zooming = true;
				self.mouse_position = ipp.mouse.position;
			}
		}
	}

	fn actions(&self) -> ActionList {
		let mut common = actions!(NavigationMessageDiscriminant;
			TranslateCanvasBegin,
			RotateCanvasBegin,
			ZoomCanvasBegin,
			SetCanvasRotation,
			WheelCanvasZoom,
			IncreaseCanvasZoom,
			DecreaseCanvasZoom,
			WheelCanvasTranslate,
			TranslateCanvas,
			TranslateCanvasByViewportFraction,
			FitViewportToSelection,
		);

		if self.panning || self.tilting || self.zooming {
			let transforming = actions!(NavigationMessageDiscriminant;
				PointerMove,
				TransformCanvasEnd,
			);
			common.extend(transforming);
		}
		common
	}
}

impl NavigationMessageHandler {
	pub fn snapped_angle(&self) -> f64 {
		let increment_radians: f64 = VIEWPORT_ROTATE_SNAP_INTERVAL.to_radians();
		if self.snap_tilt {
			(self.tilt / increment_radians).round() * increment_radians
		} else {
			self.tilt
		}
	}

	pub fn snapped_scale(&self) -> f64 {
		if self.snap_zoom {
			*VIEWPORT_ZOOM_LEVELS
				.iter()
				.min_by(|a, b| (**a - self.zoom).abs().partial_cmp(&(**b - self.zoom).abs()).unwrap())
				.unwrap_or(&self.zoom)
		} else {
			self.zoom
		}
	}

	pub fn calculate_offset_transform(&self, offset: DVec2) -> DAffine2 {
		// Try to avoid fractional coordinates to reduce anti aliasing.
		let scale = self.snapped_scale();
		let rounded_pan = ((self.pan + offset) * scale).round() / scale - offset;

		// TODO: replace with DAffine2::from_scale_angle_translation and fix the errors
		let offset_transform = DAffine2::from_translation(offset);
		let scale_transform = DAffine2::from_scale(DVec2::splat(scale));
		let angle_transform = DAffine2::from_angle(self.snapped_angle());
		let translation_transform = DAffine2::from_translation(rounded_pan);
		scale_transform * offset_transform * angle_transform * translation_transform
	}

	fn create_document_transform(&self, viewport_bounds: &ViewportBounds, responses: &mut VecDeque<Message>) {
		let half_viewport = viewport_bounds.size() / 2.;
		let scaled_half_viewport = half_viewport / self.snapped_scale();
		responses.add(DocumentOperation::SetLayerTransform {
			path: vec![],
			transform: self.calculate_offset_transform(scaled_half_viewport).to_cols_array(),
		});

		responses.add(ArtboardMessage::DispatchOperation(
			DocumentOperation::SetLayerTransform {
				path: vec![],
				transform: self.calculate_offset_transform(scaled_half_viewport).to_cols_array(),
			}
			.into(),
		));
	}

	pub fn center_zoom(&self, viewport_bounds: DVec2, zoom_factor: f64, mouse: DVec2) -> Message {
		let new_viewport_bounds = viewport_bounds / zoom_factor;
		let delta_size = viewport_bounds - new_viewport_bounds;
		let mouse_fraction = mouse / viewport_bounds;
		let delta = delta_size * (DVec2::splat(0.5) - mouse_fraction);

		NavigationMessage::TranslateCanvas { delta }.into()
	}

	pub fn clamp_zoom(zoom: f64, document_bounds: Option<[DVec2; 2]>, old_zoom: f64, ipp: &InputPreprocessorMessageHandler) -> f64 {
		let document_size = (document_bounds.map(|[min, max]| max - min).unwrap_or_default() / old_zoom) * zoom;
		let scale_factor = (document_size / ipp.viewport_bounds.size()).max_element();

		if scale_factor > f64::EPSILON * 100. && scale_factor.is_finite() && scale_factor < VIEWPORT_ZOOM_MIN_FRACTION_COVER {
			VIEWPORT_ZOOM_MIN_FRACTION_COVER / scale_factor
		} else {
			1.
		}
	}
}
