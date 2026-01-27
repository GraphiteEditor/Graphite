use crate::application::Editor;
use crate::consts::{
	VIEWPORT_ROTATE_SNAP_INTERVAL, VIEWPORT_SCROLL_RATE, VIEWPORT_ZOOM_LEVELS, VIEWPORT_ZOOM_MIN_FRACTION_COVER, VIEWPORT_ZOOM_MOUSE_RATE, VIEWPORT_ZOOM_SCALE_MAX, VIEWPORT_ZOOM_SCALE_MIN,
	VIEWPORT_ZOOM_TO_FIT_PADDING_SCALE_FACTOR,
};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::portfolio::document::navigation::utility_types::NavigationOperation;
use crate::messages::portfolio::document::utility_types::misc::PTZ;
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};
use glam::{DAffine2, DVec2};
use graph_craft::document::NodeId;

#[derive(ExtractField)]
pub struct NavigationMessageContext<'a> {
	pub network_interface: &'a mut NodeNetworkInterface,
	pub breadcrumb_network_path: &'a [NodeId],
	pub ipp: &'a InputPreprocessorMessageHandler,
	pub document_ptz: &'a mut PTZ,
	pub graph_view_overlay_open: bool,
	pub preferences: &'a PreferencesMessageHandler,
	pub viewport: &'a ViewportMessageHandler,
}

#[derive(Debug, Clone, PartialEq, Default, ExtractField)]
pub struct NavigationMessageHandler {
	navigation_operation: NavigationOperation,
	mouse_position: ViewportPosition,
	finish_operation_with_click: bool,
	abortable_pan_start: Option<f64>,
}

#[message_handler_data]
impl MessageHandler<NavigationMessage, NavigationMessageContext<'_>> for NavigationMessageHandler {
	fn process_message(&mut self, message: NavigationMessage, responses: &mut VecDeque<Message>, context: NavigationMessageContext) {
		let NavigationMessageContext {
			network_interface,
			breadcrumb_network_path,
			ipp,
			document_ptz,
			graph_view_overlay_open,
			preferences,
			viewport,
		} = context;

		fn get_ptz<'a>(document_ptz: &'a PTZ, network_interface: &'a NodeNetworkInterface, graph_view_overlay_open: bool, breadcrumb_network_path: &[NodeId]) -> Option<&'a PTZ> {
			if !graph_view_overlay_open {
				Some(document_ptz)
			} else {
				let network_metadata = network_interface.network_metadata(breadcrumb_network_path)?;
				Some(&network_metadata.persistent_metadata.navigation_metadata.node_graph_ptz)
			}
		}

		fn get_ptz_mut<'a>(document_ptz: &'a mut PTZ, network_interface: &'a mut NodeNetworkInterface, graph_view_overlay_open: bool, breadcrumb_network_path: &[NodeId]) -> Option<&'a mut PTZ> {
			if !graph_view_overlay_open {
				Some(document_ptz)
			} else {
				let Some(node_graph_ptz) = network_interface.node_graph_ptz_mut(breadcrumb_network_path) else {
					log::error!("Could not get node graph PTZ in NavigationMessageHandler process_message");
					return None;
				};
				Some(node_graph_ptz)
			}
		}

		let Some(ptz) = get_ptz(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
			log::error!("Could not get PTZ in NavigationMessageHandler process_message");
			return;
		};
		let old_zoom = ptz.zoom();

		match message {
			NavigationMessage::BeginCanvasPan => {
				responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Grabbing });

				HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])]).send_layout(responses);

				self.mouse_position = ipp.mouse.position;
				let Some(ptz) = get_ptz(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
					return;
				};
				self.navigation_operation = NavigationOperation::Pan { pan_original_for_abort: ptz.pan };
			}
			NavigationMessage::BeginCanvasTilt { was_dispatched_from_menu } => {
				let Some(ptz) = get_ptz(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
					return;
				};
				// If the node graph is open, prevent tilt and instead start panning
				if graph_view_overlay_open {
					responses.add(NavigationMessage::BeginCanvasPan);
				} else {
					responses.add(EventMessage::ToolAbort);
					responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
					HintData(vec![
						HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
						HintGroup(vec![HintInfo::keys([Key::Shift], "15° Increments")]),
					])
					.send_layout(responses);

					self.navigation_operation = NavigationOperation::Tilt {
						tilt_original_for_abort: ptz.tilt(),
						tilt_raw_not_snapped: ptz.tilt(),
						snap: false,
					};

					self.mouse_position = ipp.mouse.position;
					self.finish_operation_with_click = was_dispatched_from_menu;
				}
			}
			NavigationMessage::BeginCanvasZoom => {
				let Some(ptz) = get_ptz(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
					return;
				};

				responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::ZoomIn });
				HintData(vec![
					HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
					HintGroup(vec![HintInfo::keys([Key::Shift], "Increments")]),
				])
				.send_layout(responses);

				self.navigation_operation = NavigationOperation::Zoom {
					zoom_raw_not_snapped: ptz.zoom(),
					zoom_original_for_abort: ptz.zoom(),
					snap: false,
				};
				self.mouse_position = ipp.mouse.position;
			}
			NavigationMessage::CanvasPan { delta } => {
				let Some(ptz) = get_ptz_mut(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
					log::error!("Could not get PTZ in CanvasPan");
					return;
				};
				let document_to_viewport = self.calculate_offset_transform(viewport.center_in_viewport_space().into_dvec2(), ptz);
				let transformed_delta = document_to_viewport.inverse().transform_vector2(delta);

				ptz.pan += transformed_delta;
				responses.add(EventMessage::CanvasTransformed);
				responses.add(DocumentMessage::PTZUpdate);
			}
			NavigationMessage::CanvasPanAbortPrepare { x_not_y_axis } => {
				let Some(ptz) = get_ptz_mut(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
					log::error!("Could not get PTZ in CanvasPanAbortPrepare");
					return;
				};
				self.abortable_pan_start = Some(if x_not_y_axis { ptz.pan.x } else { ptz.pan.y });
			}
			NavigationMessage::CanvasPanAbort { x_not_y_axis } => {
				let Some(ptz) = get_ptz_mut(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
					log::error!("Could not get PTZ in CanvasPanAbort");
					return;
				};
				if let Some(abortable_pan_start) = self.abortable_pan_start {
					if x_not_y_axis {
						ptz.pan.x = abortable_pan_start;
					} else {
						ptz.pan.y = abortable_pan_start;
					}
				}
				self.abortable_pan_start = None;
				responses.add(DocumentMessage::PTZUpdate);
			}
			NavigationMessage::CanvasPanByViewportFraction { delta } => {
				let Some(ptz) = get_ptz_mut(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
					log::error!("Could not get node graph PTZ in CanvasPanByViewportFraction");
					return;
				};
				let document_to_viewport = self.calculate_offset_transform(viewport.center_in_viewport_space().into_dvec2(), ptz);
				let transformed_delta = document_to_viewport.inverse().transform_vector2(delta * viewport.size().into_dvec2());

				ptz.pan += transformed_delta;
				responses.add(DocumentMessage::PTZUpdate);
			}
			NavigationMessage::CanvasPanMouseWheel { use_y_as_x } => {
				// On Mac, the OS already converts Shift+scroll into horizontal scrolling
				let delta = if use_y_as_x && !Editor::environment().is_mac() {
					(ipp.mouse.scroll_delta.y, 0.).into()
				} else {
					ipp.mouse.scroll_delta.as_dvec2()
				} * -VIEWPORT_SCROLL_RATE;
				responses.add(NavigationMessage::CanvasPan { delta });
			}
			NavigationMessage::CanvasTiltResetAndZoomTo100Percent => {
				let Some(ptz) = get_ptz_mut(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
					log::error!("Could not get mutable PTZ in CanvasTiltResetAndZoomTo100Percent");
					return;
				};
				ptz.set_tilt(0.);
				ptz.set_zoom(1.);
				if graph_view_overlay_open {
					responses.add(NodeGraphMessage::UpdateGraphBarRight);
				} else {
					responses.add(PortfolioMessage::UpdateDocumentWidgets);
				}
				responses.add(DocumentMessage::PTZUpdate);
			}
			NavigationMessage::CanvasTiltSet { angle_radians } => {
				let Some(ptz) = get_ptz_mut(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
					log::error!("Could not get mutable PTZ in CanvasTiltSet");
					return;
				};
				ptz.set_tilt(angle_radians);
				responses.add(DocumentMessage::PTZUpdate);
				if !graph_view_overlay_open {
					responses.add(PortfolioMessage::UpdateDocumentWidgets);
					responses.add(MenuBarMessage::SendLayout);
				}
			}
			NavigationMessage::CanvasZoomDecrease { center_on_mouse } => {
				let Some(ptz) = get_ptz(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
					return;
				};

				let new_scale = *VIEWPORT_ZOOM_LEVELS.iter().rev().find(|scale| **scale < ptz.zoom()).unwrap_or(&ptz.zoom());
				if center_on_mouse {
					responses.add(self.center_zoom(viewport.size().into(), new_scale / ptz.zoom(), ipp.mouse.position));
				}
				responses.add(NavigationMessage::CanvasZoomSet { zoom_factor: new_scale });
			}
			NavigationMessage::CanvasZoomIncrease { center_on_mouse } => {
				let Some(ptz) = get_ptz(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
					return;
				};

				let new_scale = *VIEWPORT_ZOOM_LEVELS.iter().find(|scale| **scale > ptz.zoom()).unwrap_or(&ptz.zoom());
				if center_on_mouse {
					responses.add(self.center_zoom(viewport.size().into(), new_scale / ptz.zoom(), ipp.mouse.position));
				}
				responses.add(NavigationMessage::CanvasZoomSet { zoom_factor: new_scale });
			}
			NavigationMessage::CanvasZoomMouseWheel => {
				let scroll = ipp.mouse.scroll_delta.scroll_delta();
				let mut zoom_factor = 1. + scroll.abs() * preferences.viewport_zoom_wheel_rate;
				if ipp.mouse.scroll_delta.y > 0. {
					zoom_factor = 1. / zoom_factor
				}
				let document_bounds = if !graph_view_overlay_open {
					// TODO: Cache this in node graph coordinates and apply the transform to the rectangle to get viewport coordinates
					network_interface.document_metadata().document_bounds_viewport_space()
				} else {
					network_interface.graph_bounds_viewport_space(breadcrumb_network_path)
				};
				let Some(ptz) = get_ptz(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
					return;
				};

				zoom_factor *= Self::clamp_zoom(ptz.zoom() * zoom_factor, document_bounds, old_zoom, viewport);

				responses.add(self.center_zoom(viewport.size().into(), zoom_factor, ipp.mouse.position));
				responses.add(NavigationMessage::CanvasZoomSet {
					zoom_factor: ptz.zoom() * zoom_factor,
				});
			}
			NavigationMessage::CanvasZoomSet { zoom_factor } => {
				let document_bounds = if !graph_view_overlay_open {
					// TODO: Cache this in node graph coordinates and apply the transform to the rectangle to get viewport coordinates
					network_interface.document_metadata().document_bounds_viewport_space()
				} else {
					network_interface.graph_bounds_viewport_space(breadcrumb_network_path)
				};
				let Some(ptz) = get_ptz_mut(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
					log::error!("Could not get mutable PTZ in CanvasZoomSet");
					return;
				};
				let zoom = zoom_factor.clamp(VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_SCALE_MAX);
				let zoom = zoom * Self::clamp_zoom(zoom, document_bounds, old_zoom, viewport);
				ptz.set_zoom(zoom);
				if graph_view_overlay_open {
					responses.add(NodeGraphMessage::UpdateGraphBarRight);
				} else {
					responses.add(PortfolioMessage::UpdateDocumentWidgets);
				}
				responses.add(DocumentMessage::PTZUpdate);
			}
			NavigationMessage::CanvasFlip => {
				if graph_view_overlay_open {
					return;
				}
				let Some(ptz) = get_ptz_mut(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
					log::error!("Could not get mutable PTZ in CanvasFlip");
					return;
				};

				ptz.flip = !ptz.flip;

				responses.add(DocumentMessage::PTZUpdate);
				responses.add(EventMessage::CanvasTransformed);
				responses.add(MenuBarMessage::SendLayout);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			NavigationMessage::EndCanvasPTZ { abort_transform } => {
				let Some(ptz) = get_ptz_mut(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
					log::error!("Could not get mutable PTZ in EndCanvasPTZ");
					return;
				};
				// If an abort was requested, reset the active PTZ value to its original state
				if abort_transform && self.navigation_operation != NavigationOperation::None {
					match self.navigation_operation {
						NavigationOperation::None => {}
						NavigationOperation::Tilt { tilt_original_for_abort, .. } => {
							ptz.set_tilt(tilt_original_for_abort);
						}
						NavigationOperation::Pan { pan_original_for_abort, .. } => {
							ptz.pan = pan_original_for_abort;
						}
						NavigationOperation::Zoom { zoom_original_for_abort, .. } => {
							ptz.set_zoom(zoom_original_for_abort);
						}
					}
				}

				// Final chance to apply snapping if the key was pressed during this final frame
				ptz.set_tilt(self.snapped_tilt(ptz.tilt()));
				ptz.set_zoom(self.snapped_zoom(ptz.zoom()));
				responses.add(DocumentMessage::PTZUpdate);
				if graph_view_overlay_open {
					responses.add(NodeGraphMessage::UpdateGraphBarRight);
				} else {
					responses.add(PortfolioMessage::UpdateDocumentWidgets);
				}
				// Reset the navigation operation now that it's done
				self.navigation_operation = NavigationOperation::None;

				// Send the final messages to close out the operation
				responses.add(EventMessage::CanvasTransformed);
				responses.add(ToolMessage::UpdateCursor);
				responses.add(ToolMessage::UpdateHints);
				responses.add(NavigateToolMessage::End);
			}
			NavigationMessage::EndCanvasPTZWithClick { commit_key } => {
				self.finish_operation_with_click = false;

				let abort_transform = commit_key == Key::MouseRight;
				responses.add(NavigationMessage::EndCanvasPTZ { abort_transform });
			}
			NavigationMessage::FitViewportToBounds {
				bounds: [pos1, pos2],
				prevent_zoom_past_100,
			} => {
				let (pos1, pos2) = (pos1.min(pos2), pos1.max(pos2));
				let diagonal = pos2 - pos1;

				if diagonal.length() < f64::EPSILON * 1000. || viewport.size().into_dvec2() == DVec2::ZERO {
					warn!("Cannot center since the viewport size is 0");
					return;
				}

				let Some(ptz) = get_ptz_mut(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
					log::error!("Could not get node graph PTZ in CanvasPanByViewportFraction");
					return;
				};
				let document_to_viewport = self.calculate_offset_transform(viewport.center_in_viewport_space().into_dvec2(), ptz);

				let v1 = document_to_viewport.inverse().transform_point2(DVec2::ZERO);
				let v2 = document_to_viewport.inverse().transform_point2(viewport.size().into_dvec2());

				let center = ((v2 + v1) - (pos2 + pos1)) / 2.;
				let size = (v2 - v1) / diagonal;
				let new_scale = size.min_element();

				let viewport_change = document_to_viewport.transform_vector2(center);

				// Only change the pan if the change will be visible in the viewport
				if viewport_change.x.abs() > 0.5 || viewport_change.y.abs() > 0.5 {
					ptz.pan += center;
				}

				ptz.set_zoom(ptz.zoom() * new_scale * VIEWPORT_ZOOM_TO_FIT_PADDING_SCALE_FACTOR);

				// Keep the canvas filling less than the full available viewport bounds if requested.
				// And if the zoom is close to the full viewport bounds, we ignore the padding because 100% is preferrable if it still fits.
				if prevent_zoom_past_100 && ptz.zoom() > VIEWPORT_ZOOM_TO_FIT_PADDING_SCALE_FACTOR {
					ptz.set_zoom(1.);
				}

				if graph_view_overlay_open {
					responses.add(NodeGraphMessage::UpdateGraphBarRight);
				} else {
					responses.add(PortfolioMessage::UpdateDocumentWidgets);
				}
				responses.add(DocumentMessage::PTZUpdate);
			}
			// Fully zooms in on the selected
			NavigationMessage::FitViewportToSelection => {
				let selection_bounds = if graph_view_overlay_open {
					network_interface.selected_nodes_bounding_box_viewport(breadcrumb_network_path)
				} else {
					network_interface.selected_layers_artwork_bounding_box_viewport()
				};

				if let Some(bounds) = selection_bounds {
					let Some(ptz) = get_ptz(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
						log::error!("Could not get node graph PTZ in FitViewportToSelection");
						return;
					};

					let document_to_viewport = self.calculate_offset_transform(viewport.center_in_viewport_space().into_dvec2(), ptz);
					responses.add(NavigationMessage::FitViewportToBounds {
						bounds: [document_to_viewport.inverse().transform_point2(bounds[0]), document_to_viewport.inverse().transform_point2(bounds[1])],
						prevent_zoom_past_100: false,
					})
				}
			}
			NavigationMessage::PointerMove { snap } => {
				match self.navigation_operation {
					NavigationOperation::None => {}
					NavigationOperation::Pan { .. } => {
						let delta = ipp.mouse.position - self.mouse_position;
						responses.add(NavigationMessage::CanvasPan { delta });
					}
					NavigationOperation::Tilt {
						tilt_raw_not_snapped,
						tilt_original_for_abort,
						..
					} => {
						let tilt_raw_not_snapped = {
							// Compute the angle in document space to counter for the canvas being flipped
							let viewport_to_document = network_interface.document_metadata().document_to_viewport.inverse();
							let half_viewport = viewport.center_in_viewport_space().into_dvec2();
							let start_offset = viewport_to_document.transform_vector2(self.mouse_position - half_viewport);
							let end_offset = viewport_to_document.transform_vector2(ipp.mouse.position - half_viewport);
							let angle = start_offset.angle_to(end_offset);

							tilt_raw_not_snapped + angle
						};
						let Some(ptz) = get_ptz_mut(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
							log::error!("Could not get mutable PTZ in Tilt");
							return;
						};
						ptz.set_tilt(self.snapped_tilt(tilt_raw_not_snapped));

						let snap = ipp.keyboard.get(snap as usize);

						self.navigation_operation = NavigationOperation::Tilt {
							tilt_original_for_abort,
							tilt_raw_not_snapped,
							snap,
						};

						responses.add(NavigationMessage::CanvasTiltSet { angle_radians: ptz.tilt() });
					}
					NavigationOperation::Zoom {
						zoom_raw_not_snapped,
						zoom_original_for_abort,
						..
					} => {
						let zoom_raw_not_snapped = {
							let vertical_delta = self.mouse_position.y - ipp.mouse.position.y;
							let amount = vertical_delta * VIEWPORT_ZOOM_MOUSE_RATE;
							let updated_zoom = zoom_raw_not_snapped * (1. + amount);

							let document_bounds = if !graph_view_overlay_open {
								// TODO: Cache this in node graph coordinates and apply the transform to the rectangle to get viewport coordinates
								network_interface.document_metadata().document_bounds_viewport_space()
							} else {
								network_interface.graph_bounds_viewport_space(breadcrumb_network_path)
							};

							updated_zoom * Self::clamp_zoom(updated_zoom, document_bounds, old_zoom, viewport)
						};
						let Some(ptz) = get_ptz_mut(document_ptz, network_interface, graph_view_overlay_open, breadcrumb_network_path) else {
							log::error!("Could not get mutable PTZ in Zoom");
							return;
						};
						ptz.set_zoom(self.snapped_zoom(zoom_raw_not_snapped));

						let snap = ipp.keyboard.get(snap as usize);

						self.navigation_operation = NavigationOperation::Zoom {
							zoom_raw_not_snapped,
							zoom_original_for_abort,
							snap,
						};

						responses.add(NavigationMessage::CanvasZoomSet { zoom_factor: ptz.zoom() });
					}
				}

				self.mouse_position = ipp.mouse.position;
			}
		}
	}

	fn actions(&self) -> ActionList {
		let mut common = actions!(NavigationMessageDiscriminant;
			BeginCanvasPan,
			BeginCanvasTilt,
			BeginCanvasZoom,
			CanvasPan,
			CanvasPanByViewportFraction,
			CanvasPanMouseWheel,
			CanvasTiltSet,
			CanvasZoomDecrease,
			CanvasZoomIncrease,
			CanvasZoomMouseWheel,
			CanvasFlip,
			FitViewportToSelection,
		);

		if self.navigation_operation != NavigationOperation::None {
			let transforming = actions!(NavigationMessageDiscriminant;
				EndCanvasPTZ,
				PointerMove,
			);
			common.extend(transforming);
		}

		if self.finish_operation_with_click {
			let transforming_from_menu = actions!(NavigationMessageDiscriminant;
				EndCanvasPTZWithClick,
			);

			common.extend(transforming_from_menu);
		}

		common
	}
}

impl NavigationMessageHandler {
	pub fn snapped_tilt(&self, tilt: f64) -> f64 {
		let increment_radians: f64 = VIEWPORT_ROTATE_SNAP_INTERVAL.to_radians();
		if matches!(self.navigation_operation, NavigationOperation::Tilt { snap: true, .. }) {
			(tilt / increment_radians).round() * increment_radians
		} else {
			tilt
		}
	}

	pub fn snapped_zoom(&self, zoom: f64) -> f64 {
		snapped_zoom(&self.navigation_operation, zoom)
	}

	pub fn calculate_offset_transform(&self, viewport_center: DVec2, ptz: &PTZ) -> DAffine2 {
		let pan = ptz.pan;
		let tilt = ptz.tilt();
		let zoom = ptz.zoom();

		let scale = self.snapped_zoom(zoom);
		let scale_vec = if ptz.flip { DVec2::new(-scale, scale) } else { DVec2::splat(scale) };
		let scaled_center = viewport_center / scale_vec;

		// Try to avoid fractional coordinates to reduce anti aliasing.
		let rounded_pan = ((pan + scaled_center) * scale).round() / scale - scaled_center;

		// TODO: replace with DAffine2::from_scale_angle_translation and fix the errors
		let offset_transform = DAffine2::from_translation(scaled_center);
		let scale_transform = DAffine2::from_scale(scale_vec);
		let angle_transform = DAffine2::from_angle(self.snapped_tilt(tilt));
		let translation_transform = DAffine2::from_translation(rounded_pan);
		scale_transform * offset_transform * angle_transform * translation_transform
	}

	pub fn center_zoom(&self, viewport_bounds: DVec2, zoom_factor: f64, mouse: DVec2) -> Message {
		let new_viewport_bounds = viewport_bounds / zoom_factor;
		let delta_size = viewport_bounds - new_viewport_bounds;
		let mouse_fraction = mouse / viewport_bounds;
		let delta = delta_size * (DVec2::splat(0.5) - mouse_fraction);
		NavigationMessage::CanvasPan { delta }.into()
	}

	pub fn clamp_zoom(zoom: f64, document_bounds: Option<[DVec2; 2]>, old_zoom: f64, viewport: &ViewportMessageHandler) -> f64 {
		let document_size = (document_bounds.map(|[min, max]| max - min).unwrap_or_default() / old_zoom) * zoom;
		let scale_factor = (document_size / viewport.size().into_dvec2()).max_element();

		if scale_factor <= f64::EPSILON * 100. || !scale_factor.is_finite() || scale_factor >= VIEWPORT_ZOOM_MIN_FRACTION_COVER {
			return 1.;
		}

		VIEWPORT_ZOOM_MIN_FRACTION_COVER / scale_factor
	}
}

pub fn snapped_zoom(navigation_operation: &NavigationOperation, zoom: f64) -> f64 {
	if matches!(navigation_operation, NavigationOperation::Zoom { snap: true, .. }) {
		*VIEWPORT_ZOOM_LEVELS.iter().min_by(|a, b| (**a - zoom).abs().partial_cmp(&(**b - zoom).abs()).unwrap()).unwrap_or(&zoom)
	} else {
		zoom
	}
}
