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
	AnchorId, Awake, Candidate, Context, CrossingCache, Exclusions, Frame, Highlight, Incumbent, LayerId, Outcome, Request, Scene, Settings, Tunables, ViewMapping, find_constructions,
	find_constructions_from,
};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

pub use snapping::{FeatureMap, Locus};

/// How long the pointer must rest on a target to wake it while a gesture is under way, in milliseconds.
const WAKE_DWELL_IN_GESTURE: u64 = 250;

/// How long the pointer must rest on a target to wake it before any gesture, in milliseconds, which is longer so that passing over things wakes nothing.
pub(crate) const WAKE_DWELL_ON_APPROACH: u64 = 500;

/// How far the pointer may drift and still count as resting, in viewport pixels.
const REST_DRIFT: f64 = 4.;

/// How many woken targets are kept, past which the one woken longest ago goes back to sleep.
const AWAKE_LIMIT: usize = 5;

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

/// Where the pointer has stayed put, and since when, for telling when it has rested on something long enough to wake it.
#[derive(Clone, Copy, Debug)]
struct Rest {
	/// In the viewport.
	position: DVec2,
	/// In milliseconds.
	since: u64,
	/// Whether this rest has already woken what the pointer is on.
	spent: bool,
}

#[derive(Debug, Default)]
pub struct SnapSession {
	/// The document the scene mirrors, and the generation of its metadata that the scene was mirrored at.
	mirrored: Option<(DocumentId, u64)>,
	scene: Scene,
	/// What is moving while a gesture is under way, during which the scene is left as it was when the gesture began.
	gesture: Option<Exclusions>,
	/// The targets woken by resting the pointer on them, from the one woken longest ago to the latest.
	awake: Vec<Awake>,
	/// The layers each woken point of its own was found on, by its ID, which it goes back to sleep with.
	position_sources: HashMap<u64, Vec<LayerId>>,
	rest: Option<Rest>,
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

		// The pointer starts a new rest whenever it has drifted from where the last one began
		let pointer = input.mouse.position;
		if self.rest.is_none_or(|rest| rest.position.distance(pointer) > REST_DRIFT) {
			self.rest = Some(Rest {
				position: pointer,
				since: input.time,
				spent: false,
			});
		}

		let Some(view_mapping) = ViewMapping::from_affine(document.metadata().document_to_viewport) else {
			return free;
		};
		let settings = settings::settings(&document.snapping_state);
		let grid = settings::grid(document);
		let (frame, tunables, nothing_moving) = (Frame::default(), Tunables::default(), Exclusions::default());

		// A woken point found on what the gesture moves or edits goes along with it, so it is no target
		let awake: Vec<Awake> = match &self.gesture {
			Some(exclusions) => {
				let moving = |layer: &LayerId| exclusions.skips_layer(*layer) || exclusions.edited.contains(layer) || exclusions.anchors.contains_key(layer);
				let still = |target: &Awake| match target {
					Awake::Position { id, .. } => self.position_sources.get(id).is_none_or(|sources| !sources.iter().any(moving)),
					_ => true,
				};
				self.awake.iter().copied().filter(still).collect()
			}
			None => self.awake.clone(),
		};

		let context = Context {
			scene: &self.scene,
			view: &view_mapping,
			frame: &frame,
			tunables: &tunables,
			settings: &settings,
			exclusions: self.gesture.as_ref().unwrap_or(&nothing_moving),
			awake: &awake,
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
		self.rest = None;
	}

	/// Ends a gesture, however it ended, and forgets what the pointer was snapped to.
	pub fn end_gesture(&mut self) {
		self.gesture = None;
		self.constructions = None;
		self.rest = None;
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

	/// Wakes what the pointer has rested on for long enough, given the present time in milliseconds. Returns whether anything woke.
	pub fn tick(&mut self, now: u64) -> bool {
		let dwell = if self.gesture.is_some() { WAKE_DWELL_IN_GESTURE } else { WAKE_DWELL_ON_APPROACH };
		let resting_on = self.resting_on();
		let Some(rest) = self.rest.as_mut().filter(|rest| !rest.spent && now.saturating_sub(rest.since) >= dwell) else {
			return false;
		};
		if resting_on.is_empty() {
			return false;
		}

		rest.spent = true;
		let sources = self.engaged_layers();
		for target in resting_on {
			if let Awake::Position { id, .. } = target {
				self.position_sources.insert(id, sources.clone());
			}
			self.awake.push(target);
		}
		let over_the_limit = self.awake.len().saturating_sub(AWAKE_LIMIT);
		self.awake.drain(..over_the_limit);
		self.forget_sources_of_sleeping_positions();
		self.constructions = None;

		true
	}

	/// Whether the pointer is resting on something that staying there longer would wake, which calls for `tick` to be kept up.
	pub fn wants_ticks(&self) -> bool {
		self.rest.is_some_and(|rest| !rest.spent) && !self.resting_on().is_empty()
	}

	/// Puts every woken target back to sleep, and keeps the pointer from waking them again until it moves.
	pub fn sleep_all(&mut self) {
		self.awake.clear();
		self.position_sources.clear();
		self.constructions = None;

		if let Some(rest) = &mut self.rest {
			rest.spent = true;
		}
	}

	/// The layers that the snap the pointer holds was found on.
	fn engaged_layers(&self) -> Vec<LayerId> {
		let Some(shown) = &self.shown else { return Vec::new() };

		let highlights = shown.outcome.engaged_candidates().filter_map(|candidate| candidate.highlight);
		highlights
			.map(|highlight| match highlight {
				Highlight::Anchor { layer, .. } | Highlight::Curve { layer, .. } | Highlight::Box { layer } => layer,
			})
			.collect()
	}

	fn forget_sources_of_sleeping_positions(&mut self) {
		let awake = &self.awake;
		self.position_sources
			.retain(|id, _| awake.iter().any(|target| matches!(target, Awake::Position { id: awake_id, .. } if awake_id == id)));
	}

	/// The sleeping targets that the pointer is snapped to.
	fn resting_on(&self) -> Vec<Awake> {
		let Some(shown) = &self.shown else { return Vec::new() };

		let found_on = shown.outcome.engaged_candidates().filter_map(|candidate| candidate.highlight).map(|highlight| match highlight {
			Highlight::Anchor { layer, index } => Awake::Anchor { layer, index },
			Highlight::Curve { layer, index } => Awake::Curve { layer, index },
			Highlight::Box { layer } => Awake::Box { layer },
		});

		// A midpoint, or a place where paths cross, is a point of its own rather than the curves it was found on
		let engaged: Vec<&Candidate> = shown.outcome.engaged_candidates().collect();
		let on_a_path = |candidate: &&Candidate| matches!(candidate.key.kind, snapping::Kind::OnCurve | snapping::Kind::OnBoxEdge | snapping::Kind::OnArtboardEdge);
		let point_of_its_own = match engaged.as_slice() {
			[midpoint] => midpoint.key.kind == snapping::Kind::Midpoint,
			[first, second] => on_a_path(first) && on_a_path(second),
			_ => false,
		};
		let found_on: Vec<Awake> = match (point_of_its_own, engaged.first()) {
			(true, Some(candidate)) => {
				// The same point is known as one whichever feature found it
				let mut relationships: Vec<_> = engaged.iter().map(|candidate| (candidate.key.kind, candidate.key.identity)).collect();
				relationships.sort();
				let mut hasher = DefaultHasher::new();
				relationships.hash(&mut hasher);

				let position = shown.features.get(candidate.key.feature as usize).copied();
				position.map(|position| Awake::Position { id: hasher.finish(), position }).into_iter().collect()
			}
			_ => found_on.collect(),
		};

		let mut resting_on: Vec<Awake> = Vec::new();
		for target in found_on {
			if !self.awake.contains(&target) && !resting_on.contains(&target) {
				resting_on.push(target);
			}
		}
		resting_on
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

		let changed = scene_mirror::mirror(&mut self.scene, document);
		self.mirrored = Some(now);
		self.constructions = None;
		self.crossings.clear();

		// A woken target goes back to sleep when its layer changes, which may have given its place to something else
		let position_sources = &self.position_sources;
		self.awake.retain(|target| match target {
			Awake::Anchor { layer, .. } | Awake::Curve { layer, .. } | Awake::Box { layer } => !changed.contains(layer),
			Awake::Position { id, .. } => position_sources.get(id).is_none_or(|sources| !sources.iter().any(|layer| changed.contains(layer))),
		});
		self.forget_sources_of_sleeping_positions();
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
	use crate::messages::portfolio::document::utility_types::misc::GridType;
	use crate::test_utils::test_prelude::*;
	use glam::DAffine2;
	use snapping::{GeometryRole, Kind, LayerKind, Snap};

	const DOCUMENT: DocumentId = DocumentId(1);

	/// A document position given by where it appears in the viewport.
	fn in_document(editor: &EditorTestUtils, viewport_position: DVec2) -> DVec2 {
		editor.active_document().metadata().document_to_viewport.inverse().transform_point2(viewport_position)
	}

	fn snap(session: &mut SnapSession, editor: &EditorTestUtils, viewport_position: DVec2) -> DVec2 {
		snap_at_time(session, editor, viewport_position, 0)
	}

	/// Snaps with the pointer at a position in the viewport, at a time in milliseconds.
	fn snap_at_time(session: &mut SnapSession, editor: &EditorTestUtils, viewport_position: DVec2, time: u64) -> DVec2 {
		let mut input = InputPreprocessorMessageHandler::default();
		input.mouse.position = viewport_position;
		input.time = time;

		let view = SnapView::new(editor.active_document(), DOCUMENT, &input);
		session.snap_point(view, in_document(editor, viewport_position), PointOptions::default())
	}

	/// Brings the pointer to a position at a time in milliseconds, and leaves it there for as long as waking takes before any gesture.
	fn rest(session: &mut SnapSession, editor: &EditorTestUtils, viewport_position: DVec2, time: u64) -> bool {
		snap_at_time(session, editor, viewport_position, time);
		session.tick(time + WAKE_DWELL_ON_APPROACH)
	}

	/// A position on the rim of an ellipse drawn with its bounding box from `corner` to `corner + size`, at an angle around its center.
	fn on_rim(corner: DVec2, size: DVec2, degrees: f64) -> DVec2 {
		corner + size / 2. + DVec2::from_angle(degrees.to_radians()) * size / 2.
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
	async fn resting_the_pointer_on_a_curve_wakes_it_once_it_has_stayed_long_enough() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let (corner, size) = (DVec2::new(300., 200.), DVec2::new(200., 120.));
		editor.draw_ellipse(corner.x, corner.y, corner.x + size.x, corner.y + size.y).await;
		let mut session = SnapSession::default();

		// The pointer arrives on the rim, away from the anchors at its four sides, and nothing wakes until it has stayed
		let on_the_rim = on_rim(corner, size, -50.);
		snap_at_time(&mut session, &editor, on_the_rim, 1000);
		assert_eq!(engaged_kinds(&session), vec![Kind::OnCurve]);
		assert!(session.wants_ticks());
		assert!(!session.tick(1000 + WAKE_DWELL_ON_APPROACH - 1) && session.awake.is_empty());

		// Drifting a pixel or two is still resting, and moving further starts the wait again
		snap_at_time(&mut session, &editor, on_the_rim + DVec2::new(2., 1.), 1400);
		let moved_along = on_rim(corner, size, -40.);
		snap_at_time(&mut session, &editor, moved_along, 1500);
		assert!(!session.tick(1000 + WAKE_DWELL_ON_APPROACH));

		assert!(session.tick(1500 + WAKE_DWELL_ON_APPROACH));
		assert!(matches!(session.awake.as_slice(), [Awake::Curve { .. }]));

		// Staying on has nothing more to wake
		assert!(!session.wants_ticks() && !session.tick(9000));
	}

	#[tokio::test]
	async fn a_gesture_wakes_what_the_pointer_rests_on_sooner() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let (corner, size) = (DVec2::new(300., 200.), DVec2::new(200., 120.));
		editor.draw_ellipse(corner.x, corner.y, corner.x + size.x, corner.y + size.y).await;

		let mut session = SnapSession::default();
		let input = InputPreprocessorMessageHandler::default();
		session.begin_gesture(SnapView::new(editor.active_document(), DOCUMENT, &input), Moving::default());

		snap_at_time(&mut session, &editor, on_rim(corner, size, -50.), 1000);
		assert!(!session.tick(1000 + WAKE_DWELL_IN_GESTURE - 1));
		assert!(session.tick(1000 + WAKE_DWELL_IN_GESTURE));
	}

	#[tokio::test]
	async fn only_the_latest_few_woken_targets_stay_awake() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 100., 300., 240.).await;
		editor.draw_rect(500., 100., 700., 240.).await;
		let mut session = SnapSession::default();

		// Six edges of the two rectangles are rested on in turn, each away from its ends and its middle
		let along_edges = [(150., 100.), (300., 140.), (250., 240.), (100., 200.), (550., 100.), (700., 140.)];
		for (index, position) in along_edges.into_iter().enumerate() {
			assert!(rest(&mut session, &editor, position.into(), index as u64 * 2000), "nothing woke at {position:?}");
		}
		assert_eq!(session.awake.len(), AWAKE_LIMIT);

		// The first to wake was the first to go back to sleep, which resting on it again shows
		assert!(rest(&mut session, &editor, DVec2::new(150., 100.), 20_000));
		assert!(!rest(&mut session, &editor, DVec2::new(700., 140.), 22_000));
	}

	#[tokio::test]
	async fn woken_targets_go_back_to_sleep_when_told_to_or_when_their_layer_changes() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 100., 300., 240.).await;
		let rectangle = editor.get_selected_layer().await.expect("the rectangle should be selected");
		let mut session = SnapSession::default();
		let on_an_edge = DVec2::new(150., 100.);

		assert!(rest(&mut session, &editor, on_an_edge, 0));
		session.sleep_all();
		assert!(session.awake.is_empty());

		// A pointer that simply stays where it was does not wake the same thing again
		assert!(!rest(&mut session, &editor, on_an_edge, 5000));

		// Once the rectangle has moved, the place that was woken in it may hold something else
		assert!(rest(&mut session, &editor, on_an_edge + DVec2::new(60., 0.), 7000));
		let nudge = GraphOperationMessage::TransformChange {
			layer: rectangle,
			transform: DAffine2::from_translation(DVec2::new(40., 0.)),
			transform_in: TransformIn::Local,
			skip_rerender: false,
		};
		editor.handle_message(nudge).await;
		snap_at_time(&mut session, &editor, DVec2::new(900., 600.), 9000);
		assert!(session.awake.is_empty());
	}

	#[tokio::test]
	async fn a_target_put_back_to_sleep_while_the_pointer_lines_up_with_it_stays_asleep() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 100., 300., 240.).await;
		let mut session = SnapSession::default();
		assert!(rest(&mut session, &editor, DVec2::new(300., 100.), 0));

		// Level with the woken corner from afar, the pointer starts a rest of its own
		snap_at_time(&mut session, &editor, DVec2::new(600., 101.), 5000);
		assert_eq!(engaged_kinds(&session), vec![Kind::Alignment]);

		session.sleep_all();
		assert!(!session.wants_ticks());
		assert!(!session.tick(5000 + WAKE_DWELL_ON_APPROACH) && session.awake.is_empty());
	}

	#[tokio::test]
	async fn a_woken_midpoint_goes_with_its_layer() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 100., 300., 240.).await;
		let rectangle = editor.get_selected_layer().await.expect("the rectangle should be selected");
		let mut session = SnapSession::default();

		assert!(rest(&mut session, &editor, DVec2::new(201., 101.), 0));
		assert!(matches!(session.awake.as_slice(), [Awake::Position { .. }]));

		// A gesture moving the rectangle can't line up with where its middle was
		let input = InputPreprocessorMessageHandler::default();
		let moving = Moving {
			layers: vec![rectangle],
			..Default::default()
		};
		session.begin_gesture(SnapView::new(editor.active_document(), DOCUMENT, &input), moving);
		snap_at_time(&mut session, &editor, DVec2::new(201., 500.), 5000);
		assert!(!engaged_kinds(&session).contains(&Kind::Alignment));
		session.end_gesture();

		// Once the rectangle has moved, the point goes back to sleep
		let nudge = GraphOperationMessage::TransformChange {
			layer: rectangle,
			transform: DAffine2::from_translation(DVec2::new(40., 0.)),
			transform_in: TransformIn::Local,
			skip_rerender: false,
		};
		editor.handle_message(nudge).await;
		snap_at_time(&mut session, &editor, DVec2::new(900., 600.), 9000);
		assert!(session.awake.is_empty());
	}

	#[tokio::test]
	async fn woken_targets_stay_awake_through_a_change_of_view() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 100., 300., 240.).await;
		let mut session = SnapSession::default();
		assert!(rest(&mut session, &editor, DVec2::new(150., 100.), 0));

		editor.handle_message(NavigationMessage::CanvasZoomSet { zoom_factor: 1.37 }).await;
		editor.handle_message(NavigationMessage::CanvasTiltSet { angle_radians: 0.3 }).await;
		editor.handle_message(NavigationMessage::CanvasPan { delta: DVec2::new(10.3, -7.7) }).await;
		snap_at_time(&mut session, &editor, DVec2::new(900., 600.), 5000);

		assert!(matches!(session.awake.as_slice(), [Awake::Curve { .. }]));
	}

	#[tokio::test]
	async fn a_point_can_be_laid_on_the_line_that_rests_across_two_woken_ellipses() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let ellipses = [(DVec2::new(120., 240.), DVec2::new(160., 120.)), (DVec2::new(460., 300.), DVec2::new(80., 60.))];
		for (corner, size) in ellipses {
			editor.draw_ellipse(corner.x, corner.y, corner.x + size.x, corner.y + size.y).await;
		}
		let mut session = SnapSession::default();

		// Before either is woken, the open space between them offers nothing
		let between = DVec2::new(380., 270.);
		snap_at_time(&mut session, &editor, between, 0);
		assert!(engaged_kinds(&session).is_empty());

		// Each is woken on the stretch of its rim that a line laid across the tops of both would touch
		for (index, (corner, size)) in ellipses.into_iter().enumerate() {
			assert!(rest(&mut session, &editor, on_rim(corner, size, -60.), 1000 + index as u64 * 2000));
		}

		// One of the lines the two now offer touches both, and a point brought near it far from either lands on it
		snap_at_time(&mut session, &editor, between, 8000);
		let offered = session.constructions.as_ref().map(|(_, constructions)| constructions.clone()).unwrap_or_default();
		let across_both = offered.iter().find_map(|candidate| match candidate.shape {
			snapping::Shape::Track(line @ Locus::Line { .. }) if candidate.key.kind == Kind::CommonTangent => Some(line),
			_ => None,
		});
		let across_both = across_both.expect("two woken curves should offer the line touching both");

		let on_the_line = across_both.nearest(between);
		let landed = snap_at_time(&mut session, &editor, on_the_line + DVec2::new(1., 3.), 9000);
		assert_eq!(engaged_kinds(&session), vec![Kind::CommonTangent]);
		assert!(across_both.nearest(landed).abs_diff_eq(landed, 1e-6));
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
