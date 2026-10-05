//! The one place tools go to snap. It mirrors the document into the snapping library's scene and carries what a gesture needs from one pointer move to the next.

mod presenter;
mod scene_mirror;
mod settings;

use crate::messages::input_mapper::utility_types::keyboard::Key;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use glam::DVec2;
use graphene_std::vector::PointId;
use scene_mirror::layer_id;
use snapping::{
	AnchorId, Awake, Candidate, Context, CrossingCache, Exclusions, Frame, Incumbent, Outcome, Request, Scene, Settings, Tunables, ViewMapping, find_constructions, find_constructions_from,
};

pub use snapping::{FeatureMap, Locus};

/// What a tool hands over each time it asks to snap: the document being worked on and the state of the keyboard and pointer.
#[derive(Clone, Copy)]
pub struct SnapView<'a> {
	pub document: &'a DocumentMessageHandler,
	pub document_id: DocumentId,
	pub input: &'a InputPreprocessorMessageHandler,
}

impl<'a> SnapView<'a> {
	pub fn new(document: &'a DocumentMessageHandler, document_id: DocumentId, input: &'a InputPreprocessorMessageHandler) -> Self {
		Self { document, document_id, input }
	}
}

/// What a gesture is moving, which it must not snap to.
#[derive(Clone, Debug, Default)]
pub struct Moving {
	/// Layers moving as a whole, along with everything inside them.
	pub layers: Vec<LayerNodeIdentifier>,
	/// Layers with a path being edited, where what the layer draws follows the edit.
	pub edited: Vec<LayerNodeIdentifier>,
	/// The anchors being moved within those paths.
	pub anchors: Vec<(LayerNodeIdentifier, PointId)>,
}

/// How a point being placed relates to what was placed before it.
#[derive(Clone, Copy, Debug, Default)]
pub struct PointOptions {
	/// The point it joins back to with a straight segment.
	pub previous: Option<DVec2>,
	/// The length of the segment before that one.
	pub previous_length: Option<f64>,
	/// A track it must stay on.
	pub required: Option<Locus>,
}

/// What the latest attempt to snap came to, kept for drawing.
#[derive(Clone, Debug)]
pub struct Shown {
	pub outcome: Outcome,
	/// Where each feature ended up, in document space.
	pub features: Vec<DVec2>,
	/// The index of the feature the pointer carries.
	pub lead: usize,
}

/// What a kept set of constructions was found for, which it stays good for.
#[derive(Clone, Copy, Debug, PartialEq)]
struct FoundFor {
	previous: Option<DVec2>,
	previous_length: Option<f64>,
	settings: Settings,
}

#[derive(Debug, Default)]
pub struct SnapSession {
	/// The document the scene mirrors, and the generation of its metadata that the scene was mirrored at.
	mirrored: Option<(DocumentId, u64)>,
	scene: Scene,
	/// What is moving while a gesture is under way, during which the scene is left as it was when the gesture began.
	gesture: Option<Exclusions>,
	awake: Vec<Awake>,
	incumbent: Option<Incumbent>,
	crossings: CrossingCache,
	/// What the awake targets offer, kept from one pointer move to the next.
	constructions: Option<(FoundFor, Vec<Candidate>)>,
	/// What they offer the lead feature alone from the point it joins back to, kept along with `constructions`.
	constructions_from_previous: Vec<Candidate>,
	/// The features of the latest attempt, which are the only ones the engaged snap and the remembered crossings hold for.
	features: Vec<FeatureMap>,
	shown: Option<Shown>,
	cycle_key_was_held: bool,
}

impl SnapSession {
	/// Snaps a single point being placed or dragged, and returns where it lands in document space.
	pub fn snap_point(&mut self, view: SnapView, free: DVec2, options: PointOptions) -> DVec2 {
		self.snap_features(view, &[FeatureMap::IDENTITY], 0, free, options)
	}

	/// Snaps a gesture in which each feature follows the two parameters in its own way, and returns the parameters it lands on.
	///
	/// The feature at index `lead` is the one the pointer carries. Positions are in document space.
	pub fn snap_features(&mut self, view: SnapView, features: &[FeatureMap], lead: usize, free: DVec2, options: PointOptions) -> DVec2 {
		let SnapView { document, document_id, input } = view;

		// Held quiet, nothing is offered, so the scene can wait to be brought up to date
		let quiet = !document.snapping_state.snapping_enabled || input.keyboard.key(Key::KeyQ);
		if quiet {
			self.follow_document(document_id);
		} else {
			self.bring_up_to_date(document, document_id);
		}

		if self.features != features {
			self.features = features.to_vec();
			self.incumbent = None;
			self.crossings.clear();
		}

		let Some(view_mapping) = ViewMapping::from_affine(document.metadata().document_to_viewport) else {
			return free;
		};
		let settings = settings::settings(&document.snapping_state);
		let grid = settings::grid(document);
		let (frame, tunables, nothing_moving) = (Frame::default(), Tunables::default(), Exclusions::default());
		let context = Context {
			scene: &self.scene,
			view: &view_mapping,
			frame: &frame,
			tunables: &tunables,
			settings: &settings,
			exclusions: self.gesture.as_ref().unwrap_or(&nothing_moving),
			awake: &self.awake,
		};

		// What the awake targets offer is only found again when something it was found for has changed
		let found_for = FoundFor {
			previous: options.previous,
			previous_length: options.previous_length,
			settings,
		};
		if !quiet && self.constructions.as_ref().is_none_or(|(kept_for, _)| *kept_for != found_for) {
			let from_previous = options.previous.map(|previous| find_constructions_from(&context, previous, options.previous_length));
			self.constructions_from_previous = from_previous.unwrap_or_default();
			self.constructions = Some((found_for, find_constructions(&context)));
		}

		// A tap of the cycling key counts once, however long the key is held down
		let cycle_key_held = input.keyboard.key(Key::KeyW);
		let cycle = usize::from(cycle_key_held && !self.cycle_key_was_held);
		self.cycle_key_was_held = cycle_key_held;

		let request = Request {
			grid: grid.as_ref(),
			required: options.required,
			incumbent: self.incumbent,
			cycle,
			quiet,
			constructions: self.constructions.as_ref().map_or(&[], |(_, constructions)| constructions.as_slice()),
			lead_constructions: &self.constructions_from_previous,
			crossings: Some(&self.crossings),
		};
		let outcome = snapping::snap_features(&context, &request, features, lead, free, options.previous);

		self.incumbent = outcome.incumbent();
		let parameters = outcome.parameters;
		self.shown = Some(Shown {
			features: features.iter().map(|feature| feature.position(parameters)).collect(),
			lead,
			outcome,
		});

		parameters
	}

	/// Starts a gesture, which goes on snapping to the scene as it is now and to none of what it moves.
	///
	/// What the pointer was snapped to on the approach is let go, so a press should take what was shown before starting the gesture.
	pub fn begin_gesture(&mut self, view: SnapView, moving: Moving) {
		self.gesture = None;
		self.forget();
		self.bring_up_to_date(view.document, view.document_id);

		let metadata = view.document.metadata();
		let mut exclusions = Exclusions::default();
		for layer in moving.layers {
			let with_everything_inside = std::iter::once(layer).chain(layer.descendants(metadata));
			exclusions.layers.extend(with_everything_inside.map(layer_id));
		}
		exclusions.edited.extend(moving.edited.into_iter().map(layer_id));
		for (layer, anchor) in moving.anchors {
			exclusions.anchors.entry(layer_id(layer)).or_default().insert(AnchorId(anchor.inner()));
		}

		self.gesture = Some(exclusions);
		self.constructions = None;
	}

	/// Ends a gesture, however it ended, and forgets what the pointer was snapped to.
	pub fn end_gesture(&mut self) {
		self.gesture = None;
		self.constructions = None;
		self.crossings.clear();
		self.forget();
	}

	/// Forgets what the pointer was last snapped to, as when the pointer is no longer placing anything.
	pub fn forget(&mut self) {
		self.incumbent = None;
		self.shown = None;
	}

	/// What the latest attempt to snap came to, if the pointer is still placing what it snapped.
	pub fn shown(&self) -> Option<&Shown> {
		self.shown.as_ref()
	}

	/// Where the latest attempt to snap left the feature the pointer carries, in document space.
	pub fn shown_position(&self) -> Option<DVec2> {
		let shown = self.shown.as_ref()?;
		shown.features.get(shown.lead).copied()
	}

	/// Starts afresh on a document other than the one the scene mirrors.
	fn follow_document(&mut self, document_id: DocumentId) {
		let same_document = self.mirrored.is_some_and(|(mirrored_id, _)| mirrored_id == document_id);
		if !same_document {
			*self = Self::default();
		}
	}

	/// Mirrors the document into the scene if a render has reported anything new since the last time.
	fn bring_up_to_date(&mut self, document: &DocumentMessageHandler, document_id: DocumentId) {
		self.follow_document(document_id);

		let now = (document_id, document.metadata().generation);
		if self.gesture.is_some() || self.mirrored == Some(now) {
			return;
		}

		scene_mirror::mirror(&mut self.scene, document);
		self.mirrored = Some(now);
		self.constructions = None;
		self.crossings.clear();
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::messages::portfolio::document::utility_types::misc::GridType;
	use crate::test_utils::test_prelude::*;
	use snapping::{GeometryRole, Kind, LayerKind, Snap};

	const DOCUMENT: DocumentId = DocumentId(1);

	/// A document position given by where it appears in the viewport.
	fn in_document(editor: &EditorTestUtils, viewport_position: DVec2) -> DVec2 {
		editor.active_document().metadata().document_to_viewport.inverse().transform_point2(viewport_position)
	}

	fn snap(session: &mut SnapSession, editor: &EditorTestUtils, viewport_position: DVec2) -> DVec2 {
		let view = SnapView {
			document: editor.active_document(),
			document_id: DOCUMENT,
			input: &InputPreprocessorMessageHandler::default(),
		};
		session.snap_point(view, in_document(editor, viewport_position), PointOptions::default())
	}

	fn engaged_kinds(session: &SnapSession) -> Vec<Kind> {
		let shown = session.shown().expect("an attempt to snap should have been made");
		shown.outcome.engaged_candidates().map(|candidate| candidate.key.kind).collect()
	}

	#[tokio::test]
	async fn a_drawn_rectangle_is_mirrored_with_its_corners_edges_and_box() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 200., 300., 400.).await;

		let mut session = SnapSession::default();
		session.bring_up_to_date(editor.active_document(), DOCUMENT);

		let layers: Vec<_> = session.scene.layers().map(|(_, layer)| layer).collect();
		assert_eq!(layers.len(), 1);
		let layer = layers[0];
		assert_eq!(layer.kind, LayerKind::Artwork);
		assert_eq!((layer.anchors.len(), layer.curves.len()), (4, 4));
		assert!(layer.anchors.iter().all(|anchor| anchor.role == GeometryRole::Output));

		// Both the path's anchors and the corners of its box are where the rectangle was dragged out
		let quad = layer.quad.expect("a rectangle has a bounding box");
		for corner in [(100., 200.), (300., 200.), (300., 400.), (100., 400.)] {
			let corner = in_document(&editor, corner.into());
			assert!(layer.anchors.iter().any(|anchor| anchor.position.abs_diff_eq(corner, 1e-6)), "no anchor at {corner}");
			assert!(quad.iter().any(|position| position.abs_diff_eq(corner, 1e-6)), "no box corner at {corner}");
		}
	}

	#[tokio::test]
	async fn the_straight_lines_between_the_points_of_a_spline_are_snapped_to_only_while_it_is_edited() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let points = [DVec2::new(100., 300.), DVec2::new(250., 150.), DVec2::new(400., 300.)];
		editor.draw_spline(&points).await;
		let spline = editor.get_selected_layer().await.expect("the spline should be selected");

		let mut session = SnapSession::default();
		session.bring_up_to_date(editor.active_document(), DOCUMENT);

		// The path through its points is two straight lines, which the layer draws a smooth curve in place of
		let layer = session.scene.layers().next().map(|(_, layer)| layer).expect("the spline should be mirrored");
		let with_role = |role: GeometryRole| layer.curves.iter().filter(|curve| curve.role == role).collect::<Vec<_>>();
		let undrawn = with_role(GeometryRole::Undrawn);
		assert_eq!(undrawn.len(), 2);
		assert!(undrawn.iter().all(|curve| curve.curve.is_straight()));
		assert!(with_role(GeometryRole::Source).is_empty());
		assert!(with_role(GeometryRole::Output).iter().any(|curve| !curve.curve.is_straight()));
		assert!(layer.anchors.iter().filter(|anchor| anchor.id.is_some()).all(|anchor| anchor.role == GeometryRole::Source));

		// So the middle of one of those lines catches nothing as things stand
		let middle_of_a_line = (points[0] + points[1]) / 2.;
		snap(&mut session, &editor, middle_of_a_line + DVec2::new(1., -1.));
		assert!(!engaged_kinds(&session).contains(&Kind::Midpoint));

		// With the spline's path being edited, the lines are there to see, and to snap to
		let input = InputPreprocessorMessageHandler::default();
		let moving = Moving {
			edited: vec![spline],
			..Default::default()
		};
		session.begin_gesture(SnapView::new(editor.active_document(), DOCUMENT, &input), moving);

		let snapped = snap(&mut session, &editor, middle_of_a_line + DVec2::new(1., -1.));
		assert!(snapped.abs_diff_eq(in_document(&editor, middle_of_a_line), 1e-6));
		assert_eq!(engaged_kinds(&session), vec![Kind::Midpoint]);
	}

	#[tokio::test]
	async fn a_point_brought_near_a_corner_lands_on_it_and_one_far_from_everything_stays_put() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 200., 300., 400.).await;
		let mut session = SnapSession::default();

		let near_the_corner = snap(&mut session, &editor, DVec2::new(304., 397.));
		assert!(near_the_corner.abs_diff_eq(in_document(&editor, DVec2::new(300., 400.)), 1e-6));
		assert_eq!(engaged_kinds(&session), vec![Kind::Anchor]);

		session.forget();
		let far_away = DVec2::new(600., 90.);
		assert_eq!(snap(&mut session, &editor, far_away), in_document(&editor, far_away));
		assert_eq!(session.shown().map(|shown| &shown.outcome.snap), Some(&Snap::Free));
	}

	#[tokio::test]
	async fn a_gesture_snaps_to_nothing_it_moves_and_to_the_scene_as_it_began() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 200., 300., 400.).await;
		let moving_layer = editor.get_selected_layer().await.expect("the rectangle should be selected");

		let mut session = SnapSession::default();
		let view = SnapView {
			document: editor.active_document(),
			document_id: DOCUMENT,
			input: &InputPreprocessorMessageHandler::default(),
		};
		let moving = Moving {
			layers: vec![moving_layer],
			..Default::default()
		};
		session.begin_gesture(view, moving);

		// The corner of the moving rectangle no longer catches the point
		let beside_its_corner = DVec2::new(304., 397.);
		assert_eq!(snap(&mut session, &editor, beside_its_corner), in_document(&editor, beside_its_corner));

		// A rectangle that appears while the gesture is under way is not snapped to until the gesture is over
		editor.draw_rect(500., 100., 560., 160.).await;
		let beside_the_new_corner = DVec2::new(503., 102.);
		assert_eq!(snap(&mut session, &editor, beside_the_new_corner), in_document(&editor, beside_the_new_corner));

		session.end_gesture();
		assert!(snap(&mut session, &editor, beside_the_new_corner).abs_diff_eq(in_document(&editor, DVec2::new(500., 100.)), 1e-6));
	}

	#[tokio::test]
	async fn artboards_are_mirrored_as_boxes_and_hidden_layers_are_left_out() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Artboard, 0., 0., 400., 300., ModifierKeys::default()).await;
		editor.draw_rect(100., 100., 200., 200.).await;
		let rectangle = editor.get_selected_layer().await.expect("the rectangle should be selected");

		let mut session = SnapSession::default();
		session.bring_up_to_date(editor.active_document(), DOCUMENT);
		let kinds = |session: &SnapSession| {
			let mut kinds: Vec<LayerKind> = session.scene.layers().map(|(_, layer)| layer.kind).collect();
			kinds.sort_by_key(|kind| *kind == LayerKind::Artwork);
			kinds
		};
		assert_eq!(kinds(&session), vec![LayerKind::Artboard, LayerKind::Artwork]);

		// The rectangle is inside the artboard, which the scene records
		let artwork = session.scene.layers().find(|(_, layer)| layer.kind == LayerKind::Artwork);
		assert!(artwork.is_some_and(|(_, layer)| layer.parent.is_some()));

		let hide = NodeGraphMessage::SetVisibility {
			node_id: rectangle.to_node(),
			network_path: Vec::new(),
			visible: false,
		};
		editor.handle_message(hide).await;
		editor.handle_message(NodeGraphMessage::RunDocumentGraph).await;

		session.bring_up_to_date(editor.active_document(), DOCUMENT);
		assert_eq!(kinds(&session), vec![LayerKind::Artboard]);
	}

	#[tokio::test]
	async fn the_documents_options_turn_snapping_and_the_grid_on_and_off() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 200., 300., 400.).await;
		let mut session = SnapSession::default();
		let beside_the_corner = DVec2::new(304., 397.);

		// With the grid on, a point away from everything lands where the lines of the grid as drawn meet
		editor.active_document_mut().snapping_state.grid_snapping = true;
		let spacing = settings::grid(editor.active_document()).expect("the default grid should be usable").families()[0].spacing;
		let on_the_grid = snap(&mut session, &editor, DVec2::new(600., 90.));
		assert!(((on_the_grid / spacing) - (on_the_grid / spacing).round()).abs().max_element() < 1e-9);
		assert_eq!(session.shown().map(|shown| &shown.outcome.snap), Some(&Snap::Grid));

		// The master switch turns off the grid along with everything else
		editor.active_document_mut().snapping_state.snapping_enabled = false;
		for position in [beside_the_corner, DVec2::new(600., 90.)] {
			session.forget();
			assert_eq!(snap(&mut session, &editor, position), in_document(&editor, position));
		}

		// A kind of target that is turned off is not snapped to, while the others still are
		let state = &mut editor.active_document_mut().snapping_state;
		(state.snapping_enabled, state.grid_snapping, state.path.anchor_point) = (true, false, false);
		session.forget();
		assert!(snap(&mut session, &editor, beside_the_corner).abs_diff_eq(in_document(&editor, DVec2::new(300., 400.)), 1e-6));
		assert_eq!(engaged_kinds(&session), vec![Kind::BoxCorner]);
	}

	#[tokio::test]
	async fn an_isometric_grid_puts_its_points_where_all_three_sets_of_lines_meet() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let (y_axis_spacing, angle_a, angle_b) = (40., 30_f64, 20_f64);
		let state = &mut editor.active_document_mut().snapping_state;
		state.grid_snapping = true;
		state.grid.origin = DVec2::new(7., -3.);
		state.grid.grid_type = GridType::Isometric { y_axis_spacing, angle_a, angle_b };
		let origin = state.grid.origin;

		let grid = settings::grid(editor.active_document()).expect("the grid should be usable");
		let (tan_a, tan_b) = (angle_a.to_radians().tan(), angle_b.to_radians().tan());

		// The grid may be drawn coarser than its options say, by a whole number of doublings, to keep its lines apart on screen
		let across = grid.families()[0].spacing;
		let along_y = across * (tan_a + tan_b);
		let doublings = (along_y / y_axis_spacing).log2();
		assert!((doublings - doublings.round()).abs() < 1e-9 && doublings > -1e-9);

		for step in 0..50 {
			let position = DVec2::new((step as f64 * 0.731).sin() * 400., (step as f64 * 1.37).cos() * 400.);
			let point = grid.nearest_point(position) - origin;

			// Counting lines along X for the upright ones, and along Y for each slanted set, always comes to a whole number
			for lines in [point.x / across, (point.y + tan_a * point.x) / along_y, (point.y - tan_b * point.x) / along_y] {
				assert!((lines - lines.round()).abs() < 1e-6, "{point:?} is off a line by {lines}");
			}
		}
	}
}
