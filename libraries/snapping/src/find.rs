use crate::candidate::{Candidate, CandidateKey, Highlight, Kind};
use crate::curve::Curve;
use crate::frame::Frame;
use crate::locus::Locus;
use crate::scene::{Exclusions, LayerId, LayerKind, Scene};
use crate::settings::Settings;
use crate::tunables::Tunables;
use crate::view::ViewMapping;
use glam::DVec2;

/// A target allowed to offer snaps that are far from its own geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Awake {
	Anchor {
		layer: LayerId,
		index: u32,
	},
	Curve {
		layer: LayerId,
		index: u32,
	},
	Box {
		layer: LayerId,
	},
	/// A position belonging to no layer, such as a pivot.
	Position {
		id: u64,
		position: DVec2,
	},
}

/// Everything the finders need to know about the scene and how it is being looked at.
#[derive(Clone, Copy, Debug)]
pub struct Context<'a> {
	pub scene: &'a Scene,
	pub view: &'a ViewMapping,
	pub frame: &'a Frame,
	pub tunables: &'a Tunables,
	pub settings: &'a Settings,
	pub exclusions: &'a Exclusions,
	pub awake: &'a [Awake],
}

/// Blends two numbers into one that stands for the pair, for identities with more parts than a key has room for.
pub(crate) fn mix(first: u64, second: u64) -> u64 {
	let mut mixed = first.wrapping_add(0x9E37_79B9_7F4A_7C15).wrapping_mul(0xBF58_476D_1CE4_E5B9) ^ second;
	mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
	mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
	mixed ^ (mixed >> 31)
}

/// A number standing for a position, by which something drawn from that position is known as the same thing whichever finder offers it.
pub(crate) fn position_tag(position: DVec2) -> u64 {
	// Adding zero turns a negative zero into a positive one, which is the same position
	let [x, y] = (position + DVec2::ZERO).to_array().map(f64::to_bits);
	mix(x, y)
}

/// One feature of the moving thing, at the position the pointer asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Probe {
	pub feature: u32,
	pub position: DVec2,
	/// The point this feature joins back to with a straight segment, which tangents and perpendiculars are drawn from.
	pub previous: Option<DVec2>,
}

/// Finds every candidate that depends on where the feature is: the geometry near it, and the lines from its previous point that touch or meet that geometry squarely.
pub fn find_near_probe(context: &Context, probe: &Probe, candidates: &mut Vec<Candidate>) {
	let Context {
		scene,
		view,
		tunables,
		settings,
		exclusions,
		..
	} = *context;

	// Candidates are gathered from further than reach so that an engaged snap, or one along a ridden track, can still be recognized
	let gathering_distance = tunables.gathering_distance();
	let area = view.plane_bounds_within_reach(probe.position, gathering_distance);
	let nearby = |position: DVec2| view.screen_distance(probe.position, position) <= gathering_distance;
	let seen = view.jacobian_at(probe.position);
	let passes_nearby = |curve: Curve| nearby(Locus::Curve(curve).nearest_as_seen(probe.position, seen));
	let key = |kind: Kind, identity: [u64; 3]| CandidateKey {
		kind,
		feature: probe.feature,
		identity,
	};

	if settings.anchors {
		scene.anchors_within(&area, exclusions, |layer, index, anchor| {
			if nearby(anchor.position) {
				// An anchor's own ID outlasts its place in the list, which shifts as others are added and removed
				let identity = anchor.id.map_or([layer.0, index as u64, 0], |id| [layer.0, id.0, 1]);
				let spot = Candidate::spot(key(Kind::Anchor, identity), anchor.position);
				candidates.push(spot.with_highlight(Highlight::Anchor { layer, index }));
			}
		});
	}

	scene.curves_within(&area, exclusions, |layer, index, scene_curve| {
		let curve = scene_curve.curve;
		if !passes_nearby(curve) {
			return;
		}
		let identity = [layer.0, index as u64, 0];
		let highlight = Highlight::Curve { layer, index };

		if settings.curves {
			candidates.push(Candidate::track(key(Kind::OnCurve, identity), Locus::Curve(curve)).with_highlight(highlight));
		}

		let midpoint = (curve.start() + curve.end()) / 2.;
		if settings.midpoints && curve.is_straight() && nearby(midpoint) {
			candidates.push(Candidate::spot(key(Kind::Midpoint, identity), midpoint).with_highlight(highlight));
		}

		if let Some(previous) = probe.previous {
			find_lines_from_previous(context, probe.feature, previous, &curve, [layer.0, index as u64], highlight, candidates);
		}
	});

	scene.boxes_within(&area, exclusions, |layer, scene_layer, quad| {
		let artboard = scene_layer.kind == LayerKind::Artboard;
		let (corner_kind, center_kind, edge_kind) = if artboard {
			(Kind::ArtboardCorner, Kind::ArtboardCenter, Kind::OnArtboardEdge)
		} else {
			(Kind::BoxCorner, Kind::BoxCenter, Kind::OnBoxEdge)
		};
		let [corners_enabled, edge_midpoints_enabled, centers_enabled, edges_enabled] = if artboard {
			[settings.artboards, false, settings.artboards, settings.artboards]
		} else {
			[settings.box_corners, settings.box_edge_midpoints, settings.box_centers, settings.box_edges]
		};
		let highlight = Highlight::Box { layer };

		for corner in 0..4 {
			let [start, end] = [quad[corner], quad[(corner + 1) % 4]];
			let identity = [layer.0, corner as u64, 0];

			if corners_enabled && nearby(start) {
				candidates.push(Candidate::spot(key(corner_kind, identity), start).with_highlight(highlight));
			}
			if edge_midpoints_enabled && nearby((start + end) / 2.) {
				candidates.push(Candidate::spot(key(Kind::BoxEdgeMidpoint, identity), (start + end) / 2.).with_highlight(highlight));
			}

			let edge = Curve::line(start, end);
			if edges_enabled && passes_nearby(edge) {
				candidates.push(Candidate::track(key(edge_kind, identity), Locus::Curve(edge)).with_highlight(highlight));
			}
		}

		let center = (quad[0] + quad[2]) / 2.;
		if centers_enabled && nearby(center) {
			candidates.push(Candidate::spot(key(center_kind, [layer.0, 0, 0]), center).with_highlight(highlight));
		}
	});
}

/// Offers each line from the previous point that touches a curve, or meets it at a right angle, both as a track to slide along and as a spot where it lands on the curve.
pub(crate) fn find_lines_from_previous(context: &Context, feature: u32, previous: DVec2, curve: &Curve, curve_identity: [u64; 2], highlight: Highlight, candidates: &mut Vec<Candidate>) {
	let from = position_tag(previous);
	let key = |kind: Kind, ordinal: usize| CandidateKey {
		kind,
		feature,
		identity: [curve_identity[0], curve_identity[1], mix(from, ordinal as u64)],
	};
	let tolerance = 1e-9 * curve.bounds().size().length() + 8. * f64::EPSILON * previous.abs().max_element();

	// A landing at the previous point itself gives no direction, so the line meeting the curve squarely there is offered below instead
	let mut offer = |parameters: Vec<f64>, line_kind: Kind, spot_kind: Kind| {
		for (ordinal, t) in parameters.into_iter().enumerate() {
			let landing = curve.evaluate(t);
			if landing.distance(previous) <= tolerance {
				continue;
			}
			let Some(line) = Locus::line_through(previous, landing) else { continue };

			candidates.push(Candidate::track(key(line_kind, ordinal), line).with_highlight(highlight).with_reference(landing));
			candidates.push(Candidate::spot(key(spot_kind, ordinal), landing).with_highlight(highlight).with_reference(previous));
		}
	};

	if context.settings.tangents {
		offer(curve.tangent_parameters_from(previous), Kind::TangentLine, Kind::TangentPoint);
	}
	if context.settings.perpendiculars {
		offer(curve.perpendicular_parameters_from(previous), Kind::PerpendicularLine, Kind::PerpendicularFoot);

		// From a point on the curve itself, the line meeting it squarely leaves from right there
		let nearest = curve.nearest(previous);
		if nearest.distance <= tolerance
			&& let Some(normal) = curve.tangent(nearest.t).map(|tangent| tangent.perp())
			&& let Some(line) = Locus::line(previous, normal)
		{
			candidates.push(Candidate::track(key(Kind::PerpendicularLine, usize::MAX), line).with_highlight(highlight).with_reference(previous));
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::candidate::Shape;
	use crate::scene::{AnchorId, GeometryRole, SceneAnchor, SceneCurve, SceneLayer};

	/// A layer holding a rectangle's four anchors, four edges, and bounding box.
	fn rectangle(min: DVec2, max: DVec2, kind: LayerKind) -> SceneLayer {
		let corners = [min, DVec2::new(max.x, min.y), max, DVec2::new(min.x, max.y)];
		let anchor = |index: usize| SceneAnchor {
			position: corners[index],
			id: Some(AnchorId(index as u64)),
			role: GeometryRole::Source,
		};
		let edge = |index: usize| SceneCurve {
			curve: Curve::line(corners[index], corners[(index + 1) % 4]),
			ends: [Some(AnchorId(index as u64)), Some(AnchorId((index as u64 + 1) % 4))],
			role: GeometryRole::Source,
		};

		SceneLayer {
			parent: None,
			kind,
			quad: Some(corners),
			anchors: if kind == LayerKind::Artwork { (0..4).map(anchor).collect() } else { Vec::new() },
			curves: if kind == LayerKind::Artwork { (0..4).map(edge).collect() } else { Vec::new() },
		}
	}

	/// A layer holding one arch that rises from the X axis at -5 to a peak at a height of 6 and returns at +5.
	fn arch() -> SceneLayer {
		let curve = Curve::cubic(DVec2::new(-5., 0.), DVec2::new(-5., 8.), DVec2::new(5., 8.), DVec2::new(5., 0.));

		SceneLayer {
			parent: None,
			kind: LayerKind::Artwork,
			quad: None,
			anchors: Vec::new(),
			curves: vec![SceneCurve {
				curve,
				ends: [None, None],
				role: GeometryRole::Output,
			}],
		}
	}

	fn found(scene: &Scene, settings: &Settings, probe: Probe) -> Vec<Candidate> {
		let (view, frame, tunables, exclusions) = (ViewMapping::IDENTITY, Frame::default(), Tunables::default(), Exclusions::default());
		let context = Context {
			scene,
			view: &view,
			frame: &frame,
			tunables: &tunables,
			settings,
			exclusions: &exclusions,
			awake: &[],
		};

		let mut candidates = Vec::new();
		find_near_probe(&context, &probe, &mut candidates);
		candidates
	}

	fn kinds(candidates: &[Candidate]) -> Vec<Kind> {
		let mut kinds: Vec<Kind> = candidates.iter().map(|candidate| candidate.key.kind).collect();
		kinds.sort();
		kinds
	}

	fn probe_at(x: f64, y: f64) -> Probe {
		Probe {
			feature: 0,
			position: DVec2::new(x, y),
			previous: None,
		}
	}

	#[test]
	fn a_rectangle_corner_offers_its_anchor_its_two_edges_and_its_box() {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), rectangle(DVec2::ZERO, DVec2::new(100., 60.), LayerKind::Artwork));

		let candidates = found(&scene, &Settings::default(), probe_at(102., 58.));

		assert_eq!(kinds(&candidates), vec![Kind::Anchor, Kind::BoxCorner, Kind::OnCurve, Kind::OnCurve, Kind::OnBoxEdge, Kind::OnBoxEdge]);
	}

	#[test]
	fn an_anchor_is_known_by_its_own_id_wherever_it_sits_in_its_layer() {
		let anchor = |position: DVec2, id: Option<u64>| SceneAnchor {
			position,
			id: id.map(AnchorId),
			role: GeometryRole::Source,
		};
		let layer = |anchors: Vec<SceneAnchor>| SceneLayer {
			parent: None,
			kind: LayerKind::Artwork,
			quad: None,
			anchors,
			curves: Vec::new(),
		};
		let key_found = |anchors: Vec<SceneAnchor>| {
			let mut scene = Scene::new();
			scene.set_layer(LayerId(1), layer(anchors));
			found(&scene, &Settings::default(), probe_at(1., 1.))[0].key
		};

		let alone = key_found(vec![anchor(DVec2::ZERO, Some(7))]);
		let after_another = key_found(vec![anchor(DVec2::splat(500.), Some(3)), anchor(DVec2::ZERO, Some(7))]);
		assert_eq!(alone, after_another);

		// An anchor without an ID is known by its place, and is not mistaken for one that has that number as its ID
		let unnamed = key_found(vec![anchor(DVec2::ZERO, None)]);
		let named_the_same = key_found(vec![anchor(DVec2::ZERO, Some(0))]);
		assert_ne!(unnamed, named_the_same);
	}

	#[test]
	fn a_curve_is_found_where_it_appears_near_in_a_view_that_stretches() {
		let mut scene = Scene::new();
		scene.set_layer(
			LayerId(1),
			SceneLayer {
				parent: None,
				kind: LayerKind::Artwork,
				quad: None,
				anchors: Vec::new(),
				curves: vec![SceneCurve {
					curve: Curve::line(DVec2::splat(-20.), DVec2::splat(80.)),
					ends: [None, None],
					role: GeometryRole::Source,
				}],
			},
		);

		// Heights appear four times as large as widths, so the diagonal appears under 10 pixels away, though not at its nearest place in the plane
		let view = ViewMapping::from_affine(glam::DAffine2::from_scale(DVec2::new(1., 4.))).unwrap();
		let (frame, tunables, settings, exclusions) = (Frame::default(), Tunables::default(), Settings::default(), Exclusions::default());
		let context = Context {
			scene: &scene,
			view: &view,
			frame: &frame,
			tunables: &tunables,
			settings: &settings,
			exclusions: &exclusions,
			awake: &[],
		};

		let mut candidates = Vec::new();
		find_near_probe(&context, &probe_at(10., 0.), &mut candidates);

		assert_eq!(kinds(&candidates), vec![Kind::OnCurve]);
	}

	#[test]
	fn the_middle_of_a_straight_edge_offers_its_midpoint() {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), rectangle(DVec2::ZERO, DVec2::new(100., 60.), LayerKind::Artwork));

		let candidates = found(&scene, &Settings::default(), probe_at(48., -3.));

		assert_eq!(kinds(&candidates), vec![Kind::Midpoint, Kind::BoxEdgeMidpoint, Kind::OnCurve, Kind::OnBoxEdge]);
		let midpoint = candidates.iter().find(|candidate| candidate.key.kind == Kind::Midpoint).unwrap();
		assert_eq!(midpoint.shape, Shape::Spot(DVec2::new(50., 0.)));
	}

	#[test]
	fn an_artboard_offers_its_corners_center_and_edges_but_no_edge_midpoints() {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), rectangle(DVec2::ZERO, DVec2::new(16., 16.), LayerKind::Artboard));

		// The whole artboard is within the release distance of its center
		let candidates = found(&scene, &Settings::default(), probe_at(8., 8.));

		let expected = [vec![Kind::ArtboardCorner; 4], vec![Kind::ArtboardCenter], vec![Kind::OnArtboardEdge; 4]].concat();
		assert_eq!(kinds(&candidates), expected);
	}

	#[test]
	fn turning_a_kind_off_removes_just_that_kind() {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), rectangle(DVec2::ZERO, DVec2::new(100., 60.), LayerKind::Artwork));
		let settings = Settings {
			anchors: false,
			box_edges: false,
			..Default::default()
		};

		let candidates = found(&scene, &settings, probe_at(102., 58.));

		assert_eq!(kinds(&candidates), vec![Kind::BoxCorner, Kind::OnCurve, Kind::OnCurve]);

		// The points of a box are each their own kind
		let settings = Settings {
			box_edge_midpoints: false,
			..Default::default()
		};
		let candidates = found(&scene, &settings, probe_at(48., -3.));
		assert_eq!(kinds(&candidates), vec![Kind::Midpoint, Kind::OnCurve, Kind::OnBoxEdge]);
	}

	#[test]
	fn nothing_is_offered_from_beyond_the_gathering_distance() {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), rectangle(DVec2::ZERO, DVec2::new(100., 60.), LayerKind::Artwork));

		// With the release distance at 12 pixels, candidates are gathered from just under 17 pixels away
		assert!(found(&scene, &Settings::default(), probe_at(117.5, 30.)).is_empty());
		assert!(!found(&scene, &Settings::default(), probe_at(116.5, 30.)).is_empty());
	}

	#[test]
	fn a_previous_point_offers_the_tangent_line_and_where_it_touches() {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), arch());

		// From well to the left and below, one line grazes the left shoulder of the arch
		let previous = DVec2::new(-20., 0.);
		let tangent_parameters = arch().curves[0].curve.tangent_parameters_from(previous);
		assert_eq!(tangent_parameters.len(), 1);
		let touch = arch().curves[0].curve.evaluate(tangent_parameters[0]);

		let probe = Probe {
			feature: 0,
			position: touch + DVec2::new(0.5, 0.5),
			previous: Some(previous),
		};
		let candidates = found(&scene, &Settings::default(), probe);

		let tangent_point = candidates.iter().find(|candidate| candidate.key.kind == Kind::TangentPoint).unwrap();
		assert_eq!(tangent_point.shape, Shape::Spot(touch));
		assert_eq!(tangent_point.reference, Some(previous));

		let tangent_line = candidates.iter().find(|candidate| candidate.key.kind == Kind::TangentLine).unwrap();
		let Shape::Track(line) = tangent_line.shape else { panic!("a tangent line is a track") };
		assert!(line.nearest(previous).abs_diff_eq(previous, 1e-9));
		assert!(line.nearest(touch).abs_diff_eq(touch, 1e-9));

		// The line only touches the arch, so stepping along it stays on one side of the curve
		let direction = (touch - previous).normalize();
		let side = |along: f64| (arch().curves[0].curve.nearest(touch + direction * along).position - (touch + direction * along)).perp_dot(direction);
		assert!(side(-0.5) * side(0.5) > 0.);
	}

	#[test]
	fn a_previous_point_offers_the_perpendicular_line_and_its_foot() {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), rectangle(DVec2::ZERO, DVec2::new(100., 60.), LayerKind::Artwork));

		let previous = DVec2::new(30., -40.);
		let probe = Probe {
			feature: 0,
			position: DVec2::new(32., -2.),
			previous: Some(previous),
		};
		let candidates = found(&scene, &Settings::default(), probe);

		let foot = candidates.iter().find(|candidate| candidate.key.kind == Kind::PerpendicularFoot).unwrap();
		assert!(matches!(foot.shape, Shape::Spot(position) if position.abs_diff_eq(DVec2::new(30., 0.), 1e-9)));
		assert!(candidates.iter().all(|candidate| candidate.key.kind != Kind::TangentLine));
	}
}
