//! The one place tools go to snap. It mirrors the document into the snapping library's scene and carries what a gesture needs from one pointer move to the next.

mod presenter;
mod scene_mirror;
mod settings;

use crate::consts::{BIG_NUDGE_AMOUNT, DRAG_THRESHOLD, NUDGE_AMOUNT};
use crate::messages::input_mapper::utility_types::keyboard::Key;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::{HintGroup, HintInfo};
use glam::DVec2;
use graphene_std::vector::PointId;
use scene_mirror::layer_id;

pub use scene_mirror::anchor_positions;
use snapping::{
	AnchorId, Awake, Bounds, Candidate, Context, CrossingCache, Exclusions, Frame, Highlight, Incumbent, LayerId, LayerKind, Outcome, Request, Scene, Settings, Side, Snap, SpacedBox, Tunables,
	ViewMapping, find_constructions, find_constructions_from, find_equal_sizes, find_spacing,
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

/// How many of the boxes beside a moving box are awake without being rested on, which are the nearest ones.
const NEAREST_SIBLINGS_AWAKE: usize = 3;

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

/// What a gesture moves, which it must not snap to, and the layers it copies, which stay put.
#[derive(Clone, Debug, Default)]
pub struct Moving {
	/// Layers moving as a whole, along with everything inside them.
	pub layers: Vec<LayerNodeIdentifier>,
	/// Layers that stay put while copies of them move, which are snapped to like anything else.
	pub copied: Vec<LayerNodeIdentifier>,
	/// Layers with a path being edited, where what the layer draws follows the edit.
	pub edited: Vec<LayerNodeIdentifier>,
	/// The anchors being moved within those paths.
	pub anchors: Vec<(LayerNodeIdentifier, PointId)>,
	/// A layer being drawn, which is not yet in the document for the gesture to look up.
	pub drawn: Option<Drawn>,
}

/// A layer that a gesture is drawing.
#[derive(Clone, Copy, Debug)]
pub struct Drawn {
	/// The layer it goes inside, which may be the root.
	pub parent: LayerNodeIdentifier,
	/// Whether it is an artboard rather than artwork.
	pub artboard: bool,
}

/// What a snap is told beyond its features and where the pointer asks them to be.
#[derive(Clone, Copy, Debug, Default)]
pub struct PointOptions {
	/// The point it joins back to with a straight segment.
	pub previous: Option<DVec2>,
	/// The length of the segment before that one.
	pub previous_length: Option<f64>,
	/// A track the result must stay on, in the result's own terms: a position for a point, or the parameters for features.
	pub required: Option<Locus>,
	/// How many of the features, counted from the first, snap, which is all of them when `None`.
	/// The rest only follow along, as a grip the pointer holds on by does, or the corners of the box around a round shape.
	pub snapping: Option<usize>,
	/// The features that make up a box being moved or stretched, which can then even out its gaps and match the sizes of the boxes beside it.
	pub box_features: Option<BoxFeatures>,
	/// Whether the grid is left out, as for a turn, which the grid would otherwise step at odd angles.
	pub ignores_grid: bool,
}

/// Which features of a gesture make up a box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxFeatures {
	/// The features at the box's corners, in order around it.
	pub corners: [usize; 4],
	/// For each of the two axes, the feature carrying the box's low side and the one carrying its high side. A side that stays put is `None`.
	/// With both sides carried, the box moves whole along that axis, so a stretch about its center names neither.
	pub sides: [[Option<usize>; 2]; 2],
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
#[derive(Clone, Debug, PartialEq)]
struct FoundFor {
	previous: Option<DVec2>,
	previous_length: Option<f64>,
	settings: Settings,
	awake: Vec<Awake>,
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

/// A gesture under way.
#[derive(Debug)]
struct Gesture {
	/// What the gesture is moving.
	exclusions: Exclusions,
	/// Whether the scene goes on taking in changes to the document, rather than staying as it was when the gesture began.
	follows_edits: bool,
	/// The kind of the layers the gesture moves or stretches as a box.
	kind: LayerKind,
	/// The boxes of that kind sharing a parent with those layers, each with the bounds of its box.
	siblings: Vec<(LayerId, Bounds)>,
	/// The artboard those layers are in.
	artboard: Option<LayerId>,
	/// The targets awake for as long as a box is being moved near them, without having been rested on.
	automatic: Vec<Awake>,
	/// Where the pointer was in the viewport when the gesture began.
	pressed: DVec2,
	/// Whether the pointer has since moved past the drag threshold, before which nothing snaps.
	dragged: bool,
	/// Where the press put the feature the pointer carries, in document space, which it stays at until then.
	held: Option<DVec2>,
}

#[derive(Debug, Default)]
pub struct SnapSession {
	/// The document the scene mirrors, and the generation of its metadata that the scene was mirrored at.
	mirrored: Option<(DocumentId, u64)>,
	scene: Scene,
	gesture: Option<Gesture>,
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
	/// Where a press has just landed in document space, and where the pointer was pressed in the viewport, for the gesture begun next to start from.
	landed_press: Option<(DVec2, DVec2)>,
	/// Whether the latest attempt to snap held the feature the pointer carries where the press put it.
	holding_press: bool,
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
		let grid = if options.ignores_grid { None } else { settings::grid(document) };

		// A box is taken where the pointer asks for it, once any track it must stay on has had its say
		let asked = options.required.map_or(free, |track| track.nearest(free));
		let kind = self.gesture.as_ref().map_or(LayerKind::Artwork, |gesture| gesture.kind);
		let spaced = options.box_features.filter(|_| !quiet).and_then(|box_features| spaced_box(kind, &box_features, features, asked));
		let awake = self.awake_around(spaced.as_ref());

		// Nothing snaps until the pointer has moved past the drag threshold from where the gesture began
		let within_threshold = self.gesture.as_mut().is_some_and(|gesture| {
			gesture.dragged |= input.mouse.position.distance(gesture.pressed) > DRAG_THRESHOLD;
			!gesture.dragged
		});

		let (frame, tunables, nothing_moving) = (Frame::default(), Tunables::default(), Exclusions::default());

		// A woken point found on what the gesture moves or edits goes along with it, so it is no target
		let awake: Vec<Awake> = match &self.gesture {
			Some(Gesture { exclusions, .. }) => {
				let moving = |layer: &LayerId| exclusions.skips_layer(*layer) || exclusions.edited.contains(layer) || exclusions.anchors.contains_key(layer);
				let still = |target: &Awake| match target {
					Awake::Position { id, .. } => self.position_sources.get(id).is_none_or(|sources| !sources.iter().any(moving)),
					_ => true,
				};
				awake.into_iter().filter(still).collect()
			}
			None => awake,
		};

		let context = Context {
			scene: &self.scene,
			view: &view_mapping,
			frame: &frame,
			tunables: &tunables,
			settings: &settings,
			exclusions: self.gesture.as_ref().map_or(&nothing_moving, |gesture| &gesture.exclusions),
			awake: &awake,
		};

		// What the awake targets offer is only found again when something it was found for has changed
		let found_for = FoundFor {
			previous: options.previous,
			previous_length: options.previous_length,
			settings,
			awake: awake.clone(),
		};
		if !quiet && self.constructions.as_ref().is_none_or(|(kept_for, _)| *kept_for != found_for) {
			let from_previous = options.previous.map(|previous| find_constructions_from(&context, previous, options.previous_length));
			self.constructions_from_previous = from_previous.unwrap_or_default();
			self.constructions = Some((found_for, find_constructions(&context)));
		}

		// Where a box would even out its gaps to the boxes beside it, or come to the size of one of them
		let mut particular = Vec::new();
		if let Some(spaced) = &spaced {
			if document.snapping_state.bounding_box.distribute_evenly {
				find_spacing(&context, spaced, &mut particular);
			}
			if document.snapping_state.bounding_box.equal_sizes {
				find_equal_sizes(&context, spaced, &mut particular);
			}
		}

		let probed: Option<Vec<u32>> = options.snapping.map(|count| (0..count.min(features.len()) as u32).collect());

		// A tap of the cycling key counts once, however long the key is held down
		let cycle_key_held = input.keyboard.key(Key::KeyW);
		let cycle = usize::from(cycle_key_held && !self.cycle_key_was_held);
		self.cycle_key_was_held = cycle_key_held;

		let request = Request {
			grid: grid.as_ref(),
			required: options.required,
			incumbent: self.incumbent,
			cycle,
			quiet: quiet || within_threshold,
			constructions: self.constructions.as_ref().map_or(&[], |(_, constructions)| constructions.as_slice()),
			lead_constructions: &self.constructions_from_previous,
			crossings: Some(&self.crossings),
			particular: &particular,
			probed: probed.as_deref(),
		};
		let outcome = snapping::snap_features(&context, &request, features, lead, free, options.previous);

		// Until then, what a press landed somewhere stays there
		let held = self.gesture.as_ref().and_then(|gesture| gesture.held).filter(|_| within_threshold);
		let held_parameters = held.and_then(|held| features.get(lead)?.parameters_for(held)).filter(|parameters| parameters.is_finite());
		self.holding_press = held_parameters.is_some();

		self.incumbent = outcome.incumbent();
		let parameters = held_parameters.unwrap_or(outcome.parameters);
		self.shown = Some(Shown {
			features: features.iter().map(|feature| feature.position(parameters)).collect(),
			lead,
			outcome,
		});

		// A crossing, or the grid, can hold the box anywhere along the track that evens out its gaps, so they are measured again where it landed
		if let Some(shown) = &mut self.shown
			&& shown.outcome.engaged_candidates().any(|candidate| candidate.gaps.is_some())
			&& let Some(landed) = options.box_features.and_then(|box_features| spaced_box(kind, &box_features, features, parameters))
		{
			let mut measured = Vec::new();
			find_spacing(&context, &landed, &mut measured);

			let engaged = shown.outcome.engagement().map(|engagement| engagement.candidates.clone()).unwrap_or_default();
			for index in engaged {
				let Some(candidate) = shown.outcome.candidates.get_mut(index) else { continue };
				if let Some(remeasured) = measured.iter().find(|remeasured| remeasured.key == candidate.key) {
					candidate.gaps = remeasured.gaps;
				}
			}
		}

		parameters
	}

	/// Starts a gesture, which goes on snapping to the scene as it is now and to none of what it moves.
	///
	/// What the pointer was snapped to on the approach is let go, so a press should take what was shown before starting the gesture.
	pub fn begin_gesture(&mut self, view: SnapView, moving: Moving) {
		self.start_gesture(view, moving, false);
	}

	/// Starts a gesture that goes on taking in changes to the document, for a tool that adds to the document as the gesture goes.
	/// The Pen is one, where each anchor placed becomes something the next can snap to.
	///
	/// Whatever follows the pointer must be named as moving, or it would be snapped to as it moves.
	pub fn begin_gesture_following_edits(&mut self, view: SnapView, moving: Moving) {
		self.start_gesture(view, moving, true);
	}

	fn start_gesture(&mut self, view: SnapView, moving: Moving, follows_edits: bool) {
		// A gesture begun again partway through keeps where it was pressed, unless a new press has landed
		let landed = self.landed_press.take();
		let (pressed, dragged, held) = match (self.gesture.take(), landed) {
			(Some(gesture), None) => (gesture.pressed, gesture.dragged, gesture.held),
			(_, Some((position, pointer))) => (pointer, false, Some(position)),
			(None, None) => (view.input.mouse.position, false, None),
		};
		self.forget();
		self.bring_up_to_date(view.document, view.document_id);

		let metadata = view.document.metadata();
		let mut exclusions = Exclusions::default();
		for &layer in &moving.layers {
			let with_everything_inside = std::iter::once(layer).chain(layer.descendants(metadata));
			exclusions.layers.extend(with_everything_inside.map(layer_id));
		}
		let first_edited = moving.edited.first().copied();
		exclusions.edited.extend(moving.edited.into_iter().map(layer_id));
		for (layer, anchor) in moving.anchors {
			exclusions.anchors.entry(layer_id(layer)).or_default().insert(AnchorId(anchor.inner()));
		}

		// A box lines up with the boxes of its kind sharing its parent and with its artboard. O(n) in the scene's layers, once per gesture.
		let (mut kind, mut siblings, mut artboard) = (LayerKind::Artwork, Vec::new(), None);
		let first = moving.copied.first().or(moving.layers.first()).or(first_edited.as_ref());
		let inside = match (moving.drawn, first) {
			(Some(drawn), _) => Some((drawn.parent, if drawn.artboard { LayerKind::Artboard } else { LayerKind::Artwork })),
			(None, Some(&first)) => {
				let first_kind = self.scene.layer(layer_id(first)).map_or(kind, |layer| layer.kind);
				Some((first.parent(metadata).unwrap_or(LayerNodeIdentifier::ROOT_PARENT), first_kind))
			}
			(None, None) => None,
		};
		if let Some((parent, kind_inside)) = inside {
			kind = kind_inside;
			let is_layer = |layer: &LayerNodeIdentifier| *layer != LayerNodeIdentifier::ROOT_PARENT;
			let parent_id = Some(parent).filter(is_layer).map(layer_id);

			let beside = |(id, layer): (LayerId, &snapping::SceneLayer)| {
				let is_beside = layer.parent == parent_id && layer.kind == kind && !exclusions.skips_box(id);
				Some((id, Bounds::from_points(layer.quad.filter(|_| is_beside)?)))
			};
			siblings = self.scene.layers().filter_map(beside).collect();

			let is_artboard = |id: &LayerId| self.scene.layer(*id).is_some_and(|layer| layer.kind == LayerKind::Artboard);
			artboard = parent.ancestors(metadata).filter(is_layer).map(layer_id).find(is_artboard);
		}

		self.gesture = Some(Gesture {
			exclusions,
			follows_edits,
			kind,
			siblings,
			artboard,
			automatic: Vec::new(),
			pressed,
			dragged,
			held,
		});
		self.constructions = None;
		self.rest = None;
	}

	/// Notes that the pointer, pressed where it is now, has landed a press at a position in document space, so the gesture begun next starts from it afresh.
	/// Until the pointer has moved past the drag threshold, the feature that gesture carries stays at that position.
	pub fn land_press(&mut self, view: SnapView, position: DVec2) {
		self.landed_press = Some((position, view.input.mouse.position));
	}

	/// Where a press lands, in document space: on what the pointer was shown snapping to, or else on what it snaps to now.
	/// The press is noted as landed there for the gesture begun next.
	pub fn snap_press(&mut self, view: SnapView, pointer: DVec2) -> DVec2 {
		let landed = self.shown_position().unwrap_or_else(|| self.snap_point(view, pointer, PointOptions::default()));
		self.land_press(view, landed);
		landed
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
		self.landed_press = None;
		self.holding_press = false;
	}

	/// What the latest attempt to snap came to, if the pointer is still placing what it snapped.
	pub fn shown(&self) -> Option<&Shown> {
		self.shown.as_ref()
	}

	/// Whether the latest attempt to snap caught anything, or held what a press caught, rather than leaving things where the pointer asked for them.
	pub fn caught(&self) -> bool {
		self.holding_press || self.shown.as_ref().is_some_and(|shown| shown.outcome.snap != Snap::Free)
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

	/// The woken targets, oldest first.
	#[cfg(test)]
	pub fn awake(&self) -> &[Awake] {
		&self.awake
	}

	/// The woken targets, joined while a box is moved by the nearest of the boxes sharing its parent, by any such box the snap is relying on, and by its artboard.
	fn awake_around(&mut self, spaced: Option<&SpacedBox>) -> Vec<Awake> {
		let mut awake = self.awake.clone();
		let relied_on = self.shown.as_ref().map(relied_on_boxes).unwrap_or_default();
		let Some(gesture) = self.gesture.as_mut() else { return awake };
		gesture.automatic.clear();
		let Some(spaced) = spaced else { return awake };

		// O(s) in the boxes sharing the moving one's parent
		let moving = Bounds::from_points(spaced.corners);
		let mut nearest: Vec<(f64, LayerId)> = gesture.siblings.iter().map(|(layer, bounds)| (distance_apart(&moving, bounds), *layer)).collect();
		let nearer = |a: &(f64, LayerId), b: &(f64, LayerId)| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1));
		if nearest.len() > NEAREST_SIBLINGS_AWAKE {
			nearest.select_nth_unstable_by(NEAREST_SIBLINGS_AWAKE, nearer);
			nearest.truncate(NEAREST_SIBLINGS_AWAKE);
		}

		let is_sibling = |layer: &LayerId| gesture.siblings.iter().any(|(sibling, _)| sibling == layer);
		let mut layers: Vec<LayerId> = nearest
			.into_iter()
			.map(|(_, layer)| layer)
			.chain(relied_on.into_iter().filter(is_sibling))
			.chain(gesture.artboard)
			.collect();
		layers.sort();
		layers.dedup();

		let automatic = layers.into_iter().map(|layer| Awake::Box { layer }).filter(|target| !awake.contains(target));
		gesture.automatic.extend(automatic);

		awake.extend(&gesture.automatic);
		awake
	}

	/// The targets the pointer is snapped to that are not awake already, whether woken or awake unasked.
	fn resting_on(&self) -> Vec<Awake> {
		let Some(shown) = &self.shown else { return Vec::new() };
		let automatic = self.gesture.as_ref().map_or(&[][..], |gesture| gesture.automatic.as_slice());

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
			[first, second] => first.key.feature == second.key.feature && on_a_path(first) && on_a_path(second),
			_ => false,
		};
		let found_on: Vec<Awake> = match (point_of_its_own, engaged.first()) {
			(true, Some(candidate)) => {
				let Some(&position) = shown.features.get(candidate.key.feature as usize) else { return Vec::new() };

				// Known by the paths it was found on and by where they cross
				let mut relationships: Vec<_> = engaged.iter().map(|candidate| (candidate.key.kind, candidate.key.identity)).collect();
				relationships.sort();
				let mut hasher = DefaultHasher::new();
				relationships.hash(&mut hasher);
				position.to_array().map(f64::to_bits).hash(&mut hasher);

				vec![Awake::Position { id: hasher.finish(), position }]
			}
			_ => found_on.collect(),
		};

		let mut resting_on: Vec<Awake> = Vec::new();
		for target in found_on {
			if !self.awake.contains(&target) && !automatic.contains(&target) && !resting_on.contains(&target) {
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
		let scene_is_held = self.gesture.as_ref().is_some_and(|gesture| !gesture.follows_edits);
		if scene_is_held || self.mirrored == Some(now) {
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

/// How far a nudge moves something in document space. With grid snapping on, the position furthest along the nudge lands on the next grid line ahead.
/// A diagonal nudge moves as a nudge along each of its two directions would, one after the other.
pub fn nudge_translation(document: &DocumentMessageHandler, nudge: DVec2, positions: impl IntoIterator<Item = DVec2>) -> DVec2 {
	let positions: Vec<DVec2> = positions.into_iter().collect();
	if nudge.x != 0. && nudge.y != 0. {
		let first_move = nudge_along(document, DVec2::new(nudge.x, 0.), &positions);
		let moved_positions: Vec<DVec2> = positions.iter().map(|position| position + first_move).collect();
		return first_move + nudge_along(document, DVec2::new(0., nudge.y), &moved_positions);
	}
	nudge_along(document, nudge, &positions)
}

/// Whether a nudge lands on the next grid line, rather than moving by its own amount.
pub fn nudges_land_on_grid(document: &DocumentMessageHandler) -> bool {
	document.snapping_state.snapping_enabled && settings::grid(document).is_some()
}

/// How far a nudge in one direction moves something in document space, as [`nudge_translation`] describes.
fn nudge_along(document: &DocumentMessageHandler, nudge: DVec2, positions: &[DVec2]) -> DVec2 {
	let translation = DVec2::from_angle(-document.document_ptz.tilt()).rotate(nudge);

	if !nudges_land_on_grid(document) {
		return translation;
	}

	// A big nudge lands on the next major line, or ten lines on when the major lines are the only ones drawn
	let big_nudge = nudge.abs().max_element() > NUDGE_AMOUNT;
	let has_minor_lines = document.snapping_state.grid.drawn_levels(&document.document_ptz).is_some_and(|levels| levels.minor.is_some());
	let (grid, lines_ahead) = match (big_nudge, has_minor_lines) {
		(true, true) => (settings::major_grid(document), 1),
		(true, false) => (settings::grid(document), (BIG_NUDGE_AMOUNT / NUDGE_AMOUNT).round() as usize),
		(false, _) => (settings::grid(document), 1),
	};
	let (Some(grid), Some(direction)) = (grid, translation.try_normalize()) else { return translation };
	let leading = positions
		.iter()
		.copied()
		.filter(|position| position.is_finite())
		.max_by(|a, b| a.dot(direction).total_cmp(&b.dot(direction)));
	let Some(leading) = leading else { return translation };

	let distance = (0..lines_ahead).try_fold(0., |distance, _| Some(distance + grid.distance_to_next_line(leading + direction * distance, direction)?));
	distance.map_or(translation, |distance| direction * distance)
}

/// The hints for the keys that steer snapping, shown wherever a tool snaps.
pub fn snapping_hints() -> HintGroup {
	HintGroup(vec![HintInfo::keys([Key::KeyQ], "Quiet Snapping"), HintInfo::keys([Key::KeyW], "Cycle Snap")])
}

/// The box a gesture's features make up, with each feature where the parameters put it.
fn spaced_box(kind: LayerKind, box_features: &BoxFeatures, features: &[FeatureMap], parameters: DVec2) -> Option<SpacedBox> {
	let position = |index: usize| features.get(index).map(|feature| feature.position(parameters));
	let side = |index: Option<usize>| {
		let index = index?;
		let (feature, position) = (index as u32, position(index)?);
		Some(Side { feature, position })
	};

	let [first, second, third, fourth] = box_features.corners;
	Some(SpacedBox {
		kind,
		corners: [position(first)?, position(second)?, position(third)?, position(fourth)?],
		sides: box_features.sides.map(|sides| sides.map(side)),
	})
}

/// How far apart two boxes are at their nearest, which is nothing where they overlap.
fn distance_apart(first: &Bounds, second: &Bounds) -> f64 {
	(first.min - second.max).max(second.min - first.max).max(DVec2::ZERO).length()
}

/// The layers with boxes that a shown snap relies on: those its engaged candidates were found on, and that of the track it is riding.
fn relied_on_boxes(shown: &Shown) -> Vec<LayerId> {
	let outcome = &shown.outcome;
	let riding = outcome.engagement().and_then(|engagement| engagement.riding);
	let ridden = riding.and_then(|key| outcome.candidates.iter().find(|candidate| candidate.key == key));

	let on_a_box = |candidate: &Candidate| match candidate.highlight? {
		Highlight::Box { layer } => Some(layer),
		_ => None,
	};
	outcome.engaged_candidates().chain(ridden).filter_map(on_a_box).collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
	use crate::messages::portfolio::document::utility_types::misc::GridType;
	use crate::test_utils::test_prelude::*;
	use glam::{DAffine2, DMat2};
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
	async fn a_gesture_that_follows_edits_snaps_to_what_is_added_while_it_goes() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 200., 300., 400.).await;
		let moving_layer = editor.get_selected_layer().await.expect("the rectangle should be selected");

		let mut session = SnapSession::default();
		let input = InputPreprocessorMessageHandler::default();
		let moving = Moving {
			layers: vec![moving_layer],
			..Default::default()
		};
		session.begin_gesture_following_edits(SnapView::new(editor.active_document(), DOCUMENT, &input), moving);

		// A rectangle that appears while the gesture is under way can be snapped to at once
		editor.draw_rect(500., 100., 560., 160.).await;
		assert!(snap(&mut session, &editor, DVec2::new(503., 102.)).abs_diff_eq(in_document(&editor, DVec2::new(500., 100.)), 1e-6));

		// What the gesture moves is still left alone
		let beside_the_moving_corner = DVec2::new(304., 397.);
		assert_eq!(snap(&mut session, &editor, beside_the_moving_corner), in_document(&editor, beside_the_moving_corner));
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
	async fn a_woken_box_offers_alignment_by_its_own_option_alone() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 100., 300., 240.).await;
		let mut session = SnapSession::default();

		// Resting on the center of the rectangle wakes its box, with no anchor there to wake along with it
		assert!(rest(&mut session, &editor, DVec2::new(200., 170.), 0));
		assert!(matches!(session.awake.as_slice(), [Awake::Box { .. }]));

		// Level with the center from afar, the box offers no alignment while its option is off, whatever the option for anchors
		let level_with_the_center = DVec2::new(600., 171.);
		let state = &mut editor.active_document_mut().snapping_state;
		(state.bounding_box.align_with_edges, state.path.align_with_anchor_point) = (false, true);
		snap_at_time(&mut session, &editor, level_with_the_center, 5000);
		assert!(!engaged_kinds(&session).contains(&Kind::Alignment));

		// And it offers it again with its option on, even with the option for anchors off
		let state = &mut editor.active_document_mut().snapping_state;
		(state.bounding_box.align_with_edges, state.path.align_with_anchor_point) = (true, false);
		snap_at_time(&mut session, &editor, level_with_the_center, 6000);
		assert_eq!(engaged_kinds(&session), vec![Kind::Alignment]);
	}

	#[tokio::test]
	async fn each_place_one_pair_of_paths_crosses_is_woken_as_a_point_of_its_own() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let (corner, size) = (DVec2::new(200., 200.), DVec2::new(200., 200.));
		editor.draw_ellipse(corner.x, corner.y, corner.x + size.x, corner.y + size.y).await;

		// A line cuts across one quarter of the circle's rim, crossing that same piece of it twice
		let crossings = [on_rim(corner, size, 20.), on_rim(corner, size, 70.)];
		let beyond = (crossings[1] - crossings[0]).normalize() * 40.;
		let (start, end) = (crossings[0] - beyond, crossings[1] + beyond);
		editor.drag_tool(ToolType::Line, start.x, start.y, end.x, end.y, ModifierKeys::default()).await;
		let mut session = SnapSession::default();

		// Both crossings are found on the same two curves
		let mut crossed_curves = Vec::new();
		for (index, crossing) in crossings.into_iter().enumerate() {
			assert!(rest(&mut session, &editor, crossing, index as u64 * 2000), "nothing woke at {crossing}");
			assert_eq!(engaged_kinds(&session), vec![Kind::OnCurve, Kind::OnCurve]);

			let shown = session.shown().expect("an attempt to snap should have been made");
			let mut curves: Vec<[u64; 3]> = shown.outcome.engaged_candidates().map(|candidate| candidate.key.identity).collect();
			curves.sort();
			crossed_curves.push(curves);
		}
		assert_eq!(crossed_curves[0], crossed_curves[1]);
		let &[Awake::Position { .. }, Awake::Position { position: second_crossing, .. }] = session.awake.as_slice() else {
			panic!("both crossings should be awake, not {:?}", session.awake)
		};

		// Far off but level with the second crossing, the pointer lines up with it
		snap_at_time(&mut session, &editor, DVec2::new(600., crossings[1].y + 2.), 9000);
		let shown = session.shown().expect("an attempt to snap should have been made");
		let lined_up_with: Vec<_> = shown.outcome.engaged_candidates().map(|candidate| (candidate.key.kind, candidate.reference)).collect();
		assert_eq!(lined_up_with, vec![(Kind::Alignment, Some(second_crossing))]);
	}

	#[tokio::test]
	async fn two_features_resting_on_paths_of_their_own_wake_those_paths() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(320., 200., 520., 260.).await;
		editor.draw_rect(100., 340., 200., 520.).await;
		let mut session = SnapSession::default();

		// A line grown both ways from its middle has one end on the first rectangle's top side and the other on the second's right side
		let mirrored = FeatureMap {
			matrix: DMat2::from_diagonal(DVec2::splat(-1.)),
			offset: in_document(&editor, DVec2::new(300., 300.)) * 2.,
		};
		let mut input = InputPreprocessorMessageHandler::default();
		input.mouse.position = DVec2::new(400., 201.);
		let view = SnapView::new(editor.active_document(), DOCUMENT, &input);
		session.snap_features(view, &[FeatureMap::IDENTITY, mirrored], 0, in_document(&editor, input.mouse.position), PointOptions::default());

		let shown = session.shown().expect("an attempt to snap should have been made");
		let mut held: Vec<u32> = shown.outcome.engaged_candidates().map(|candidate| candidate.key.feature).collect();
		held.sort();
		assert_eq!(held, vec![0, 1]);

		// Each end wakes what it rests on, as it would alone, rather than a point where nothing crosses
		assert!(session.tick(WAKE_DWELL_ON_APPROACH));
		assert_eq!(session.awake.len(), 2);
		assert!(!session.awake.iter().any(|target| matches!(target, Awake::Position { .. })));
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
	async fn deactivating_the_tools_lets_go_of_the_snap_and_wakes_nothing_the_pointer_was_resting_on() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 100., 300., 240.).await;
		editor.handle_message(DocumentMessage::DeselectAllLayers).await;
		editor.select_tool(ToolType::Line).await;
		fn session(editor: &EditorTestUtils) -> &SnapSession {
			&editor.editor.dispatcher.message_handlers.tool_message_handler.snap_session
		}

		// With the Line tool ready, the pointer comes to rest beside a corner, which starts the wait to wake it
		editor.handle_message(InputPreprocessorMessage::CurrentTime { timestamp: 1000 }).await;
		editor.move_mouse(302., 242., ModifierKeys::empty(), MouseKeys::empty()).await;
		assert!(engaged_kinds(session(&editor)).contains(&Kind::Anchor));
		assert!(session(&editor).wants_ticks());

		// Deactivating the tools, as opening the graph does, lets go of the corner at once
		editor.handle_message(ToolMessage::DeactivateTools).await;
		assert!(session(&editor).shown().is_none() && !session(&editor).wants_ticks());

		// The frame on which the corner would have woken wakes nothing
		editor
			.handle_message(InputPreprocessorMessage::CurrentTime {
				timestamp: 1000 + WAKE_DWELL_ON_APPROACH,
			})
			.await;
		editor.handle_message(BroadcastMessage::TriggerEvent(EventMessage::AnimationFrame)).await;
		assert!(session(&editor).awake.is_empty());
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

	/// The corners of a box between two viewport positions, in document space and in order around it from the first.
	fn box_corners(editor: &EditorTestUtils, from: DVec2, to: DVec2) -> [DVec2; 4] {
		[from, DVec2::new(to.x, from.y), to, DVec2::new(from.x, to.y)].map(|corner| in_document(editor, corner))
	}

	/// The four corners of a box sliding together, with the sides that the first, second, and third of them carry.
	fn sliding_box(corners: [DVec2; 4]) -> (Vec<FeatureMap>, BoxFeatures) {
		let box_features = BoxFeatures {
			corners: [0, 1, 2, 3],
			sides: [[Some(0), Some(1)], [Some(0), Some(2)]],
		};
		(corners.map(FeatureMap::sliding_from).to_vec(), box_features)
	}

	/// Begins a gesture moving the selected layer as a whole, and returns the session for it.
	async fn moving_the_selected_layer(editor: &mut EditorTestUtils) -> SnapSession {
		let layer = editor.get_selected_layer().await.expect("the layer drawn last should be selected");
		let mut session = SnapSession::default();
		let input = InputPreprocessorMessageHandler::default();
		let moving = Moving {
			layers: vec![layer],
			..Default::default()
		};
		session.begin_gesture(SnapView::new(editor.active_document(), DOCUMENT, &input), moving);
		session
	}

	/// Snaps features slid by `free`, with the pointer well past the drag threshold from where the gesture began.
	fn slide(session: &mut SnapSession, editor: &EditorTestUtils, features: &[FeatureMap], free: DVec2, options: PointOptions) -> DVec2 {
		let mut input = InputPreprocessorMessageHandler::default();
		input.mouse.position = DVec2::splat(100.);
		session.snap_features(SnapView::new(editor.active_document(), DOCUMENT, &input), features, 0, free, options)
	}

	#[tokio::test]
	async fn a_box_slid_to_nearly_halfway_between_two_others_lands_with_the_same_gap_on_both_sides() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 100., 160., 160.).await;
		editor.draw_rect(300., 100., 360., 160.).await;
		editor.draw_rect(200., 112., 240., 148.).await;
		let mut session = moving_the_selected_layer(&mut editor).await;

		// Halfway is ten further along, where the gap on either side comes to fifty
		let (features, box_features) = sliding_box(box_corners(&editor, DVec2::new(200., 112.), DVec2::new(240., 148.)));
		let unit = in_document(&editor, DVec2::X) - in_document(&editor, DVec2::ZERO);
		let options = PointOptions {
			box_features: Some(box_features),
			..Default::default()
		};
		let slid = slide(&mut session, &editor, &features, unit * 7., options);

		assert!(slid.abs_diff_eq(unit * 10., 1e-6), "the box slid by {slid}");
		assert!(engaged_kinds(&session).contains(&Kind::Spacing));
	}

	#[tokio::test]
	async fn a_box_both_evenly_spaced_and_lined_up_has_its_gaps_drawn_across_where_it_lands() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 100., 160., 160.).await;
		editor.draw_rect(300., 100., 360., 160.).await;
		editor.draw_rect(200., 104., 240., 164.).await;
		let mut session = moving_the_selected_layer(&mut editor).await;

		// Centered between the boxes is ten further along, and level with them is four pixels up
		let (features, box_features) = sliding_box(box_corners(&editor, DVec2::new(200., 104.), DVec2::new(240., 164.)));
		let unit = in_document(&editor, DVec2::X) - in_document(&editor, DVec2::ZERO);
		let options = PointOptions {
			box_features: Some(box_features),
			..Default::default()
		};
		let slid = slide(&mut session, &editor, &features, unit * 7., options);

		assert!(slid.abs_diff_eq(unit * 10. - unit.perp() * 4., 1e-6), "the box slid by {slid}");
		let kinds = engaged_kinds(&session);
		assert!(kinds.contains(&Kind::Spacing) && kinds.contains(&Kind::Alignment), "engaged {kinds:?}");

		// Each gap is drawn halfway across where the box overlaps its neighbor once lined up, rather than where the pointer alone would have put it
		let shown = session.shown().expect("an attempt to snap should have been made");
		let gaps = shown.outcome.engaged_candidates().find_map(|candidate| candidate.gaps).expect("the evened-out gaps should be shown");
		let to_viewport = editor.active_document().metadata().document_to_viewport;
		let ends: Vec<DVec2> = gaps.spans.iter().flatten().map(|&end| to_viewport.transform_point2(end)).collect();
		let wanted = [(160., 130.), (210., 130.), (250., 130.), (300., 130.)].map(DVec2::from);
		assert!(ends.iter().zip(wanted).all(|(end, wanted)| end.abs_diff_eq(wanted, 1e-6)), "the gaps are drawn between {ends:?}");
	}

	#[tokio::test]
	async fn an_artboard_slid_to_nearly_halfway_between_two_others_lands_with_the_same_gap_on_both_sides() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		for [left, top, right, bottom] in [[0., 0., 100., 100.], [300., 0., 400., 100.], [160., 10., 220., 90.]] {
			editor.drag_tool(ToolType::Artboard, left, top, right, bottom, ModifierKeys::default()).await;
		}

		let corners = box_corners(&editor, DVec2::new(160., 10.), DVec2::new(220., 90.));
		let metadata = editor.active_document().metadata();
		let is_between = |layer: &LayerNodeIdentifier| metadata.bounding_box_document(*layer).is_some_and(|[min, _]| min.abs_diff_eq(corners[0], 1e-6));
		let between = metadata.all_layers().find(is_between).expect("the artboard drawn last should be where it was drawn");

		let mut session = SnapSession::default();
		let input = InputPreprocessorMessageHandler::default();
		let moving = Moving {
			layers: vec![between],
			..Default::default()
		};
		session.begin_gesture(SnapView::new(editor.active_document(), DOCUMENT, &input), moving);

		let gesture = session.gesture.as_ref().expect("a gesture should be under way");
		assert_eq!((gesture.kind, gesture.siblings.len(), gesture.artboard), (LayerKind::Artboard, 2, None));

		// Halfway is ten further along, where the gap on either side comes to seventy
		let (features, box_features) = sliding_box(corners);
		let unit = in_document(&editor, DVec2::X) - in_document(&editor, DVec2::ZERO);
		let options = PointOptions {
			box_features: Some(box_features),
			..Default::default()
		};
		let slid = slide(&mut session, &editor, &features, unit * 7., options);

		assert!(slid.abs_diff_eq(unit * 10., 1e-6), "the artboard slid by {slid}");
		assert!(engaged_kinds(&session).contains(&Kind::Spacing));
	}

	#[tokio::test]
	async fn a_box_moved_nearly_level_with_the_one_beside_it_lines_up_with_it_unasked() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 100., 160., 160.).await;
		editor.draw_rect(300., 104., 340., 144.).await;
		let mut session = moving_the_selected_layer(&mut editor).await;

		let (features, box_features) = sliding_box(box_corners(&editor, DVec2::new(300., 104.), DVec2::new(340., 144.)));
		let unit = in_document(&editor, DVec2::X) - in_document(&editor, DVec2::ZERO);
		let asked = unit * 5.;

		// As four points and no more, nothing far from them catches the box
		assert_eq!(slide(&mut session, &editor, &features, asked, PointOptions::default()), asked);

		// As a box, its top comes level with the top of the box beside it, four pixels up
		let options = PointOptions {
			box_features: Some(box_features),
			..Default::default()
		};
		let slid = slide(&mut session, &editor, &features, asked, options);
		assert!(slid.abs_diff_eq(asked - unit.perp() * 4., 1e-6), "the box slid by {slid}");
		assert!(engaged_kinds(&session).contains(&Kind::Alignment));
	}

	#[tokio::test]
	async fn nothing_snaps_until_the_pointer_has_moved_past_the_drag_threshold_from_where_the_gesture_began() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 100., 200., 200.).await;
		let corner = in_document(&editor, DVec2::new(200., 200.));

		let mut session = SnapSession::default();
		let mut input = InputPreprocessorMessageHandler::default();
		let mut snap_with_pointer_at = |session: &mut SnapSession, position: DVec2| {
			input.mouse.position = position;
			if session.gesture.is_none() {
				session.begin_gesture(SnapView::new(editor.active_document(), DOCUMENT, &input), Moving::default());
			}
			session.snap_point(SnapView::new(editor.active_document(), DOCUMENT, &input), in_document(&editor, position), PointOptions::default())
		};

		// Pressed three pixels from the corner, the point stays put until the pointer has moved more than a pixel
		let press = DVec2::new(203., 203.);
		assert_eq!(snap_with_pointer_at(&mut session, press), in_document(&editor, press));
		let barely_moved = press + DVec2::new(0.5, 0.);
		assert_eq!(snap_with_pointer_at(&mut session, barely_moved), in_document(&editor, barely_moved));
		assert!(snap_with_pointer_at(&mut session, press + DVec2::new(0., -2.)).abs_diff_eq(corner, 1e-6));
	}

	#[tokio::test]
	async fn a_press_landed_on_a_corner_holds_there_until_the_pointer_has_moved_past_the_drag_threshold() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 100., 200., 200.).await;
		let corner = in_document(&editor, DVec2::new(200., 200.));
		let press = DVec2::new(203., 203.);

		let begin_with_pointer_at = |session: &mut SnapSession, position: DVec2| {
			let mut input = InputPreprocessorMessageHandler::default();
			input.mouse.position = position;
			session.begin_gesture(SnapView::new(editor.active_document(), DOCUMENT, &input), Moving::default());
		};

		let mut session = SnapSession::default();
		let mut input = InputPreprocessorMessageHandler::default();
		input.mouse.position = press;
		let landed = session.snap_press(SnapView::new(editor.active_document(), DOCUMENT, &input), in_document(&editor, press));
		assert!(landed.abs_diff_eq(corner, 1e-6));
		begin_with_pointer_at(&mut session, press);

		let barely_moved = press + DVec2::new(0.5, 0.);
		assert!(snap(&mut session, &editor, barely_moved).abs_diff_eq(corner, 1e-6));
		assert!(session.caught());

		// Begun again partway through, as when a key changes what the drag does, the gesture keeps its press
		let moved_on = press + DVec2::new(0., -2.);
		assert!(snap(&mut session, &editor, moved_on).abs_diff_eq(corner, 1e-6));
		begin_with_pointer_at(&mut session, moved_on);
		assert!(snap(&mut session, &editor, moved_on + DVec2::new(0.5, 0.)).abs_diff_eq(corner, 1e-6));

		// A new press landing partway through starts the threshold afresh from it
		let far_away = DVec2::new(400., 400.);
		let mut input = InputPreprocessorMessageHandler::default();
		input.mouse.position = far_away;
		session.land_press(SnapView::new(editor.active_document(), DOCUMENT, &input), in_document(&editor, far_away));
		begin_with_pointer_at(&mut session, far_away);
		assert_eq!(snap(&mut session, &editor, far_away + DVec2::new(0.5, 0.)), in_document(&editor, far_away));
	}

	#[tokio::test]
	async fn a_box_being_drawn_lines_up_with_the_boxes_it_will_be_beside() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 100., 160., 160.).await;

		let mut session = SnapSession::default();
		let input = InputPreprocessorMessageHandler::default();
		let drawn = Drawn {
			parent: LayerNodeIdentifier::ROOT_PARENT,
			artboard: false,
		};
		let moving = Moving {
			drawn: Some(drawn),
			..Default::default()
		};
		session.begin_gesture(SnapView::new(editor.active_document(), DOCUMENT, &input), moving);

		// Its top comes level with the top of the box already there, four pixels up
		let (features, box_features) = sliding_box(box_corners(&editor, DVec2::new(300., 104.), DVec2::new(340., 144.)));
		let unit = in_document(&editor, DVec2::X) - in_document(&editor, DVec2::ZERO);
		let options = PointOptions {
			box_features: Some(box_features),
			..Default::default()
		};
		let slid = slide(&mut session, &editor, &features, unit * 5., options);

		assert!(slid.abs_diff_eq(unit * 5. - unit.perp() * 4., 1e-6), "the box slid by {slid}");
		assert!(engaged_kinds(&session).contains(&Kind::Alignment));
	}

	#[tokio::test]
	async fn a_grip_that_a_box_is_held_by_snaps_to_nothing_itself() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 200., 300., 400.).await;
		let mut session = SnapSession::default();

		// The pointer has hold of something a few pixels from the rectangle's corner
		let grip = [FeatureMap::sliding_from(in_document(&editor, DVec2::new(304., 397.)))];
		let nothing_snaps = PointOptions {
			snapping: Some(0),
			..Default::default()
		};
		assert_eq!(slide(&mut session, &editor, &grip, DVec2::ZERO, nothing_snaps), DVec2::ZERO);

		// Were that a point of the thing itself, it would land on the corner
		let slid = slide(&mut session, &editor, &grip, DVec2::ZERO, PointOptions::default());
		assert!(grip[0].position(slid).abs_diff_eq(in_document(&editor, DVec2::new(300., 400.)), 1e-6));

		// A point of the thing that the grip carries along still lands on what it comes near, which here is another corner
		session.forget();
		let carried = FeatureMap::sliding_from(in_document(&editor, DVec2::new(97., 203.)));
		let only_the_first_snaps = PointOptions {
			snapping: Some(1),
			..Default::default()
		};
		let input = InputPreprocessorMessageHandler::default();
		let view = SnapView::new(editor.active_document(), DOCUMENT, &input);
		let slid = session.snap_features(view, &[carried, grip[0]], 1, DVec2::ZERO, only_the_first_snaps);
		assert!(carried.position(slid).abs_diff_eq(in_document(&editor, DVec2::new(100., 200.)), 1e-6));
	}

	#[tokio::test]
	async fn a_box_awake_unasked_is_not_woken_for_good_by_resting_where_it_has_lined_the_moving_box_up() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(100., 100., 160., 160.).await;
		editor.draw_rect(300., 104., 340., 144.).await;
		let mut session = moving_the_selected_layer(&mut editor).await;

		let (features, box_features) = sliding_box(box_corners(&editor, DVec2::new(300., 104.), DVec2::new(340., 144.)));
		let options = PointOptions {
			box_features: Some(box_features),
			..Default::default()
		};
		slide(&mut session, &editor, &features, DVec2::ZERO, options);
		assert!(engaged_kinds(&session).contains(&Kind::Alignment));

		// However long the pointer stays, the box it lined up with is awake only for as long as it is beside the moving one
		assert!(!session.wants_ticks());
		assert!(!session.tick(WAKE_DWELL_IN_GESTURE));
		session.end_gesture();
		assert!(session.awake.is_empty());
	}

	#[tokio::test]
	async fn a_box_keeps_to_the_line_of_a_far_box_while_nearer_ones_come_up_and_loses_it_once_it_leaves() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(0., 100., 40., 140.).await;
		for left in [300., 400., 500.] {
			editor.draw_rect(left, 300., left + 40., 340.).await;
		}
		editor.draw_rect(100., 104., 140., 144.).await;
		let mut session = moving_the_selected_layer(&mut editor).await;

		let (features, box_features) = sliding_box(box_corners(&editor, DVec2::new(100., 104.), DVec2::new(140., 144.)));
		let unit = in_document(&editor, DVec2::X) - in_document(&editor, DVec2::ZERO);
		let options = PointOptions {
			box_features: Some(box_features),
			..Default::default()
		};
		let risen = |slid: DVec2| -slid.dot(unit.perp()) / unit.length();

		// Beside the first box, the moving one comes level with its top, four pixels up
		let slid = slide(&mut session, &editor, &features, DVec2::ZERO, options);
		assert!((risen(slid) - 4.).abs() < 1e-6, "the box rose by {}", risen(slid));

		// Slid along that line to where the three boxes below are all nearer, it is still held level with the first
		let slid = slide(&mut session, &editor, &features, unit * 320., options);
		assert!((risen(slid) - 4.).abs() < 1e-6, "the box rose by {}", risen(slid));

		// Once pulled clear of every line the first box offers, coming back beside this one finds that box no longer awake
		slide(&mut session, &editor, &features, unit * 320. + unit.perp() * 70., options);
		let slid = slide(&mut session, &editor, &features, unit * 320., options);
		assert!(risen(slid).abs() < 1e-6, "the box rose by {}", risen(slid));
	}

	/// Stretches the right side of a box forty wide by seventeen pixels, beside a box sixty wide, and returns how far it slid, a pixel along, and the session.
	async fn stretch_a_box_beside_one_sixty_wide(editor: &mut EditorTestUtils, equal_sizes: bool) -> (DVec2, DVec2, SnapSession) {
		editor.draw_rect(100., 200., 160., 260.).await;
		editor.draw_rect(300., 200., 340., 240.).await;
		editor.active_document_mut().snapping_state.bounding_box.equal_sizes = equal_sizes;
		let mut session = moving_the_selected_layer(editor).await;

		// The right side slides along with its two corners, while the left side stays where it is
		let [top_left, top_right, bottom_right, bottom_left] = box_corners(editor, DVec2::new(300., 200.), DVec2::new(340., 240.));
		let stays = |offset: DVec2| FeatureMap { matrix: glam::DMat2::ZERO, offset };
		let features = [FeatureMap::sliding_from(top_right), FeatureMap::sliding_from(bottom_right), stays(top_left), stays(bottom_left)];
		let unit = in_document(editor, DVec2::X) - in_document(editor, DVec2::ZERO);
		let options = PointOptions {
			required: Locus::line(DVec2::ZERO, unit),
			box_features: Some(BoxFeatures {
				corners: [2, 0, 1, 3],
				sides: [[None, Some(0)], [None, None]],
			}),
			..Default::default()
		};

		let slid = slide(&mut session, editor, &features, unit * 17. + unit.perp() * 5., options);
		(slid, unit, session)
	}

	#[tokio::test]
	async fn a_box_stretched_to_nearly_the_width_of_the_one_beside_it_lands_on_that_width() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		// Sixty wide, as the other box is, takes sliding the side twenty, wherever the pointer is up or down
		let (slid, unit, session) = stretch_a_box_beside_one_sixty_wide(&mut editor, true).await;
		assert!(slid.abs_diff_eq(unit * 20., 1e-6), "the side slid by {slid}");
		assert!(engaged_kinds(&session).contains(&Kind::EqualSize));
	}

	#[tokio::test]
	async fn a_box_stretched_with_equal_sizes_turned_off_stays_at_the_width_the_pointer_asks_for() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		let (slid, unit, session) = stretch_a_box_beside_one_sixty_wide(&mut editor, false).await;
		assert!(slid.abs_diff_eq(unit * 17., 1e-6), "the side slid by {slid}");
		assert!(!engaged_kinds(&session).contains(&Kind::EqualSize));
	}

	#[tokio::test]
	async fn a_point_slides_along_the_edge_of_a_box_only_while_along_box_edges_is_on() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_ellipse(300., 200., 500., 320.).await;
		let mut session = SnapSession::default();

		// Three pixels below the top of the ellipse's box, far from its corners, its middle, and the ellipse itself
		let below_the_top = DVec2::new(320., 203.);
		assert_eq!(snap(&mut session, &editor, below_the_top), in_document(&editor, below_the_top));

		editor.active_document_mut().snapping_state.bounding_box.edges = true;
		session.forget();
		assert!(snap(&mut session, &editor, below_the_top).abs_diff_eq(in_document(&editor, DVec2::new(320., 200.)), 1e-6));
		assert_eq!(engaged_kinds(&session), vec![Kind::OnBoxEdge]);
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
