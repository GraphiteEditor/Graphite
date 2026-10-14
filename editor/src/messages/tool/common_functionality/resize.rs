use crate::messages::input_mapper::utility_types::keyboard::Key;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::snap_session::{BoxFeatures, Drawn, FeatureMap, Locus, Moving, PointOptions, SnapSession, SnapView};
use glam::{DAffine2, DVec2, Vec2Swizzles};

#[derive(Clone, Debug, Default)]
pub struct Resize {
	/// Stored as a document position so the start doesn't move if the canvas is panned.
	pub drag_start: DVec2,
	pub layer: Option<LayerNodeIdentifier>,
}

impl Resize {
	/// Starts drawing a layer from where the pointer is shown snapped to.
	pub fn start(&mut self, snap_session: &mut SnapSession, view: SnapView, drawn: Drawn) {
		self.snap_drag_start(snap_session, view);
		Self::begin_drawing(snap_session, view, drawn);
	}

	/// Puts the start of the drag where the pointer is shown snapped to, without beginning a gesture.
	pub fn snap_drag_start(&mut self, snap_session: &mut SnapSession, view: SnapView) {
		let pointer = view.document.metadata().document_to_viewport.inverse().transform_point2(view.input.mouse.position);
		self.drag_start = snap_session.shown_position().unwrap_or_else(|| snap_session.snap_point(view, pointer, PointOptions::default()));
	}

	/// Begins the gesture of drawing a layer that is not yet in the document.
	pub fn begin_drawing(snap_session: &mut SnapSession, view: SnapView, drawn: Drawn) {
		let moving = Moving {
			drawn: Some(drawn),
			..Default::default()
		};
		snap_session.begin_gesture(view, moving);
	}

	/// Calculate the drag start position in viewport space.
	pub fn viewport_drag_start(&self, document: &DocumentMessageHandler) -> DVec2 {
		let root_transform = document.metadata().document_to_viewport;
		root_transform.transform_point2(self.drag_start)
	}

	/// Compute the drag start and end based on the current mouse position. If the layer doesn't exist, returns [`None`].
	/// If you want to draw even without a layer, use [`Resize::calculate_points_ignore_layer`].
	/// A shape not filling its box gives its points to snap instead, as unmirrored offsets from the box's center in units of half its size.
	pub fn calculate_points(&mut self, snap_session: &mut SnapSession, view: SnapView, center: Key, lock_ratio: Key, shape_points: &[DVec2]) -> Option<[DVec2; 2]> {
		let layer = self.layer?;

		if layer == LayerNodeIdentifier::ROOT_PARENT {
			log::error!("Resize layer cannot be ROOT_PARENT");
			return None;
		}

		if !view.document.network_interface.document_network().nodes.contains_key(&layer.to_node()) {
			self.layer.take();
			return None;
		}

		let keyboard = &view.input.keyboard;
		Some(self.snap_box(snap_session, view, keyboard.key(center), keyboard.key(lock_ratio), false, shape_points))
	}

	/// Compute the drag start and end based on the current mouse position. Ignores the state of the layer.
	/// If you want to only draw whilst a layer exists, use [`Resize::calculate_points`].
	pub fn calculate_points_ignore_layer(&mut self, snap_session: &mut SnapSession, view: SnapView, center: Key, lock_ratio: Key, in_document: bool) -> [DVec2; 2] {
		let keyboard = &view.input.keyboard;
		self.snap_box(snap_session, view, keyboard.key(center), keyboard.key(lock_ratio), in_document, &[])
	}

	pub fn calculate_circle_points(&mut self, snap_session: &mut SnapSession, view: SnapView, center: Key) -> [DVec2; 2] {
		self.snap_box(snap_session, view, view.input.keyboard.key(center), true, false, &[])
	}

	/// Snaps the box dragged out from the start of the drag to the pointer, and returns two of its opposite corners in viewport space.
	fn snap_box(&self, snap_session: &mut SnapSession, view: SnapView, center: bool, lock_ratio: bool, in_document: bool, shape_points: &[DVec2]) -> [DVec2; 2] {
		let document_to_viewport = view.document.metadata().document_to_viewport;
		let viewport_to_document = document_to_viewport.inverse();
		let start = self.drag_start;
		let viewport_start = document_to_viewport.transform_point2(start);
		let pointer_from_start = view.input.mouse.position - viewport_start;

		// The box is upright in the window with extents in pixels, so a drag catching nothing ends exactly at the pointer, while an artboard is upright in the document
		let (axes, viewport_axes) = if in_document {
			([DVec2::X, DVec2::Y], [DVec2::X, DVec2::Y].map(|axis| document_to_viewport.transform_vector2(axis)))
		} else {
			([DVec2::X, DVec2::Y].map(|axis| viewport_to_document.transform_vector2(axis)), [DVec2::X, DVec2::Y])
		};
		let spans_the_plane = axes[0].perp_dot(axes[1]).is_finite() && axes[0].perp_dot(axes[1]) != 0.;
		if !spans_the_plane {
			return [viewport_start, view.input.mouse.position];
		}
		let along_axes = |extents: DVec2| axes[0] * extents.x + axes[1] * extents.y;
		let along_viewport_axes = |extents: DVec2| viewport_axes[0] * extents.x + viewport_axes[1] * extents.y;

		// The parameters are the box's extents along each axis from the start, which are half its size when drawn from the center
		let mut free = if in_document {
			viewport_to_document.transform_vector2(pointer_from_start)
		} else {
			pointer_from_start
		};
		let mut required = None;
		if lock_ratio {
			free = free.abs().max(free.abs().yx()) * free.signum();
			required = Locus::line(DVec2::ZERO, free.signum());
		}

		// Points of the box in units of its extents, with the corner the pointer carries third
		let unit_corners = if center {
			[DVec2::new(-1., -1.), DVec2::new(1., -1.), DVec2::ONE, DVec2::new(-1., 1.)]
		} else {
			[DVec2::ZERO, DVec2::X, DVec2::ONE, DVec2::Y]
		};
		let middle = unit_corners.iter().sum::<DVec2>() / 4.;
		let stretched = |unit: DVec2| FeatureMap::stretching(start + along_axes(unit), start, axes);

		let (features, lead, snapping, box_features): (Vec<FeatureMap>, usize, Option<usize>, Option<BoxFeatures>) = if shape_points.is_empty() {
			let middles = [0, 1, 2, 3].map(|i| (unit_corners[i] + unit_corners[(i + 1) % 4]) / 2.);
			let features = unit_corners.into_iter().chain(middles).chain([middle]).map(stretched).collect();

			// Gaps and sizes are matched only for a box upright in the document, and only along an axis where just the pointer's side moves
			let upright = axes.iter().all(|axis| axis.x.abs().min(axis.y.abs()) < 1e-9 * axis.length());
			let pointer_corner = start + along_axes(free);
			let box_features = upright.then(|| {
				let sides = [0, 1].map(|document_axis| {
					let mut sides = [None, None];
					if !lock_ratio && !center {
						sides[usize::from(pointer_corner[document_axis] >= start[document_axis])] = Some(2);
					}
					sides
				});
				BoxFeatures { corners: [0, 1, 2, 3], sides }
			});

			(features, 2, None, box_features)
		} else {
			// The shape is drawn upright whichever way the drag goes, so where its points fall in the box depends on the signs of the extents
			let signs = free.signum();
			let shape_units = shape_points.iter().map(|&point| if center { signs * point } else { (DVec2::ONE + signs * point) / 2. });

			// Only the shape's own points and its center snap, while the pointer's corner still leads and lands on the grid
			let features = shape_units.chain([middle]).chain(unit_corners).map(stretched).collect();
			(features, shape_points.len() + 3, Some(shape_points.len() + 1), None)
		};

		let options = PointOptions {
			required,
			snapping,
			box_features,
			..Default::default()
		};
		let snapped = snap_session.snap_features(view, &features, lead, free, options);

		// A drag catching nothing keeps the pointer's extents exactly, which projecting onto the required track would round
		let extents = along_viewport_axes(if snap_session.caught() { snapped } else { free });

		let near = if center { viewport_start - extents } else { viewport_start };
		[near, viewport_start + extents]
	}

	pub fn cleanup(&mut self) {
		self.layer = None;
	}
}

/// The viewport zoom factor, taken from the document-to-viewport transform so it accounts for zoom snapping and canvas flipping.
/// Divide a viewport-space measurement by this to convert it to document units.
pub fn viewport_zoom(document: &DocumentMessageHandler) -> f64 {
	document.metadata().document_to_viewport.matrix2.determinant().abs().sqrt()
}

/// The viewport-space transform for a freshly drawn layer placed at `viewport_position`: window-aligned, so it is just the zoom (no tilt or flip).
/// Routing it through [`TransformIn::Viewport`] yields a document-space layer transform that counter-rotates the tilt/flip with the zoom left out.
/// `aspect` is an optional unitless local-frame stretch (e.g. for polygons and stars); pass [`DVec2::ONE`] for none.
pub fn window_aligned_transform(document: &DocumentMessageHandler, viewport_position: DVec2, aspect: DVec2) -> DAffine2 {
	DAffine2::from_scale_angle_translation(viewport_zoom(document) * aspect, 0., viewport_position)
}

/// [`TransformSet`](GraphOperationMessage::TransformSet) applying [`window_aligned_transform`] to a freshly drawn layer, via
/// [`TransformIn::Viewport`] so the parent transform is resolved at execution time (also correct when the placement is deferred until after the graph runs).
pub fn window_aligned_transform_set(document: &DocumentMessageHandler, layer: LayerNodeIdentifier, viewport_position: DVec2, aspect: DVec2) -> Message {
	GraphOperationMessage::TransformSet {
		layer,
		transform: window_aligned_transform(document, viewport_position, aspect),
		transform_in: TransformIn::Viewport,
		skip_rerender: false,
	}
	.into()
}

/// [`TransformSet`](GraphOperationMessage::TransformSet) placing a freshly drawn path-like layer (Pen, Freehand, Spline, Line, Arrow) at
/// `viewport_position` with only a translation: the drawn geometry holds the shape (including tilt), so the Transform node stays a pure document-space offset.
pub fn translation_transform_set(document: &DocumentMessageHandler, layer: LayerNodeIdentifier, viewport_position: DVec2) -> Message {
	// Same orientation/scale as the document-to-viewport transform, but translated to land the layer's origin at the drawn viewport position.
	let mut transform = document.metadata().document_to_viewport;
	transform.translation = viewport_position;

	GraphOperationMessage::TransformSet {
		layer,
		transform,
		transform_in: TransformIn::Viewport,
		skip_rerender: false,
	}
	.into()
}

#[cfg(test)]
mod test_resize {
	use crate::messages::portfolio::document::utility_types::misc::{GridSnapping, GridType};
	use crate::test_utils::test_prelude::*;

	/// The least and greatest corners of a layer's bounding box in the viewport.
	fn bounds(editor: &EditorTestUtils, layer: LayerNodeIdentifier) -> [DVec2; 2] {
		editor.active_document().metadata().bounding_box_viewport(layer).expect("the layer should have a bounding box")
	}

	fn assert_bounds(editor: &EditorTestUtils, layer: LayerNodeIdentifier, expected: [DVec2; 2]) {
		let [min, max] = bounds(editor, layer);
		let [expected_min, expected_max] = expected;
		assert!(
			min.distance(expected_min) < 1e-6 && max.distance(expected_max) < 1e-6,
			"the layer spans {min} to {max} rather than {expected_min} to {expected_max}"
		);
	}

	/// Draws a rectangle by dragging from one viewport position to another, and returns its layer.
	async fn draw_rectangle(editor: &mut EditorTestUtils, from: DVec2, to: DVec2, modifier_keys: ModifierKeys) -> LayerNodeIdentifier {
		editor.drag_tool(ToolType::Rectangle, from.x, from.y, to.x, to.y, modifier_keys).await;
		editor.get_selected_layer().await.expect("the drawn rectangle should be selected")
	}

	fn shows_an_attempt(editor: &EditorTestUtils) -> bool {
		editor.editor.dispatcher.message_handlers.tool_message_handler.snap_session.shown().is_some()
	}

	#[tokio::test]
	async fn a_rectangle_drawn_with_its_far_corner_near_a_corner_of_another_ends_on_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let other = draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.), ModifierKeys::empty()).await;
		assert_bounds(&editor, other, [DVec2::new(100., 100.), DVec2::new(200., 180.)]);

		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(203., 178.), ModifierKeys::empty()).await;

		assert_bounds(&editor, layer, [DVec2::new(200., 180.), DVec2::new(300., 300.)]);
	}

	#[tokio::test]
	async fn a_rectangle_pressed_a_few_pixels_from_a_corner_of_another_starts_on_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.), ModifierKeys::empty()).await;

		// The press lands where the pointer was shown snapped to as it came near
		let layer = draw_rectangle(&mut editor, DVec2::new(203., 183.), DVec2::new(300., 300.), ModifierKeys::empty()).await;

		assert_bounds(&editor, layer, [DVec2::new(200., 180.), DVec2::new(300., 300.)]);
	}

	#[tokio::test]
	async fn a_rectangle_drawn_beside_another_a_few_pixels_short_of_level_with_its_bottom_ends_level_with_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(160., 160.), ModifierKeys::empty()).await;

		// Its bottom comes four pixels above the other's bottom, with hundreds of pixels between them
		let layer = draw_rectangle(&mut editor, DVec2::new(400., 128.), DVec2::new(440., 156.), ModifierKeys::empty()).await;

		assert_bounds(&editor, layer, [DVec2::new(400., 128.), DVec2::new(440., 160.)]);
	}

	#[tokio::test]
	async fn a_rectangle_drawn_nearly_as_wide_as_the_one_beside_it_gets_exactly_that_width() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(170., 170.), ModifierKeys::empty()).await;

		// Seventy wide, as the other is, puts its right edge at 370
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(366., 340.), ModifierKeys::empty()).await;

		assert_bounds(&editor, layer, [DVec2::new(300., 300.), DVec2::new(370., 340.)]);
	}

	#[tokio::test]
	async fn a_rectangle_drawn_with_shift_is_square_with_its_larger_extent_and_snaps_along_its_diagonal() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(480., 480.), DVec2::new(540., 540.), ModifierKeys::empty()).await;

		let square = draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(180., 130.), ModifierKeys::SHIFT).await;
		assert_bounds(&editor, square, [DVec2::new(100., 100.), DVec2::new(180., 180.)]);

		// Squared up, its far corner comes a few pixels short of the other's top left corner, which lies along its diagonal
		let square = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(476., 470.), ModifierKeys::SHIFT).await;
		assert_bounds(&editor, square, [DVec2::new(300., 300.), DVec2::new(480., 480.)]);
	}

	#[tokio::test]
	async fn a_rectangle_drawn_from_its_center_with_a_corner_near_a_corner_of_another_lands_on_it_and_stays_centered() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(160., 160.), ModifierKeys::empty()).await;

		// The corner opposite the pointer comes a few pixels from the other's bottom right corner
		let layer = draw_rectangle(&mut editor, DVec2::new(230., 230.), DVec2::new(297., 302.), ModifierKeys::ALT).await;

		assert_bounds(&editor, layer, [DVec2::new(160., 160.), DVec2::new(300., 300.)]);
	}

	#[tokio::test]
	async fn on_the_grid_both_corners_of_a_rectangle_being_drawn_land_on_grid_points() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.active_document_mut().snapping_state.grid_snapping = true;

		let layer = draw_rectangle(&mut editor, DVec2::new(300.3, 300.7), DVec2::new(357.3, 343.7), ModifierKeys::empty()).await;

		let document = editor.active_document();
		let state = &document.snapping_state;
		let GridType::Rectangular { spacing } = state.grid.grid_type else {
			panic!("the default grid should be rectangular")
		};
		let spacing = GridSnapping::compute_rectangle_spacing(spacing, &document.document_ptz).expect("the default grid should be usable");
		for corner in document.metadata().bounding_box_document(layer).expect("the rectangle should have a bounding box") {
			let steps = (corner - state.grid.origin) / spacing;
			assert!((steps - steps.round()).abs().max_element() < 1e-6, "the corner at {corner} is {steps} grid steps");
		}
	}

	#[tokio::test]
	async fn escape_while_drawing_a_rectangle_leaves_no_layer_and_ends_the_snapping_gesture() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		editor.select_tool(ToolType::Rectangle).await;
		editor.move_mouse(300., 300., ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(300., 300., ModifierKeys::empty()).await;
		editor.move_mouse(360., 350., ModifierKeys::empty(), MouseKeys::LEFT).await;
		assert_eq!(editor.active_document().metadata().all_layers().count(), 1);
		assert!(shows_an_attempt(&editor));

		editor.press(Key::Escape, ModifierKeys::empty()).await;
		editor.left_mouseup(360., 350., ModifierKeys::empty()).await;

		assert_eq!(editor.active_document().metadata().all_layers().count(), 0);
		assert!(!shows_an_attempt(&editor));
	}

	#[tokio::test]
	async fn on_a_tilted_canvas_a_rectangle_being_drawn_follows_the_pointer_and_its_far_corner_lands_on_a_corner_of_another() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.), ModifierKeys::empty()).await;

		editor.handle_message(NavigationMessage::CanvasTiltSet { angle_radians: 30_f64.to_radians() }).await;
		let corner = editor.active_document().metadata().document_to_viewport.transform_point2(DVec2::new(200., 180.));

		// Upright in the window, a rectangle far from the other spans just where it was dragged
		let free = draw_rectangle(&mut editor, corner + DVec2::new(200., 40.), corner + DVec2::new(300., 120.), ModifierKeys::empty()).await;
		assert_bounds(&editor, free, [corner + DVec2::new(200., 40.), corner + DVec2::new(300., 120.)]);

		// Its far corner is brought a few pixels on screen from the other's bottom right corner
		let snapped = draw_rectangle(&mut editor, corner + DVec2::new(100., 80.), corner + DVec2::new(3., -2.), ModifierKeys::empty()).await;
		assert_bounds(&editor, snapped, [corner, corner + DVec2::new(100., 80.)]);
	}

	#[tokio::test]
	async fn on_a_zoomed_canvas_a_rectangle_being_drawn_follows_the_pointer_and_its_far_corner_lands_on_a_corner_of_another() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.), ModifierKeys::empty()).await;

		editor.handle_message(NavigationMessage::CanvasZoomSet { zoom_factor: 2.5 }).await;
		let corner = editor.active_document().metadata().document_to_viewport.transform_point2(DVec2::new(200., 180.));

		let free = draw_rectangle(&mut editor, corner + DVec2::new(200., 40.), corner + DVec2::new(300., 120.), ModifierKeys::empty()).await;
		assert_bounds(&editor, free, [corner + DVec2::new(200., 40.), corner + DVec2::new(300., 120.)]);

		// Its far corner is brought a few pixels on screen from the other's bottom right corner
		let snapped = draw_rectangle(&mut editor, corner + DVec2::new(100., 80.), corner + DVec2::new(3., -2.), ModifierKeys::empty()).await;
		assert_bounds(&editor, snapped, [corner, corner + DVec2::new(100., 80.)]);
	}
}
