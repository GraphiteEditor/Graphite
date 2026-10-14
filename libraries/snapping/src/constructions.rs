use crate::candidate::{Candidate, CandidateKey, Highlight, Kind};
use crate::curve::Curve;
use crate::find::{Awake, Context, find_lines_from_previous, mix, position_tag};
use crate::locus::Locus;
use crate::tangency::common_tangents;
use glam::DVec2;

/// An awake target looked up in the scene, with a pair of numbers that tells it apart from every other.
enum Woken {
	Point { position: DVec2, identity: [u64; 2], highlight: Option<Highlight> },
	Curve { curve: Curve, identity: [u64; 2], highlight: Highlight },
	Box { quad: [DVec2; 4], identity: [u64; 2], highlight: Highlight },
}

impl Woken {
	fn identity(&self) -> [u64; 2] {
		match *self {
			Self::Point { identity, .. } | Self::Curve { identity, .. } | Self::Box { identity, .. } => identity,
		}
	}
}

/// Looks up the awake targets, leaving out any that no longer exist or that the gesture is now moving.
///
/// They come back in an order that does not depend on when each woke, so that what a pair of them offers is always named the same way.
fn woken(context: &Context) -> Vec<Woken> {
	let Context { scene, exclusions, .. } = *context;

	let look_up = |awake: &Awake| match *awake {
		Awake::Anchor { layer, index } => {
			let anchor = scene.layer(layer)?.anchors.get(index as usize)?;
			(!exclusions.skips_anchor(layer, anchor)).then_some(Woken::Point {
				position: anchor.position,
				identity: [layer.0, index as u64 * 4],
				highlight: Some(Highlight::Anchor { layer, index }),
			})
		}
		Awake::Curve { layer, index } => {
			let curve = scene.layer(layer)?.curves.get(index as usize)?;
			(!exclusions.skips_curve(layer, curve)).then_some(Woken::Curve {
				curve: curve.curve,
				identity: [layer.0, index as u64 * 4 + 1],
				highlight: Highlight::Curve { layer, index },
			})
		}
		Awake::Box { layer } => {
			let quad = scene.layer(layer)?.quad?;
			(!exclusions.skips_box(layer)).then_some(Woken::Box {
				quad,
				identity: [layer.0, 2],
				highlight: Highlight::Box { layer },
			})
		}
		Awake::Position { id, position } => Some(Woken::Point {
			position,
			identity: [id, 3],
			highlight: None,
		}),
	};

	let mut woken: Vec<Woken> = context.awake.iter().filter_map(look_up).collect();
	woken.sort_by_key(Woken::identity);
	woken.dedup_by_key(|target| target.identity());
	woken
}

/// Folds a target's identity into one number, for candidates that come from two targets and have only so much room to say which.
fn fold(identity: [u64; 2]) -> u64 {
	mix(identity[0], identity[1])
}

/// The layer of an awake curve and its index there, which is how the finder near the pointer knows the same curve.
fn place_in_layer(identity: [u64; 2]) -> [u64; 2] {
	[identity[0], identity[1] / 4]
}

fn key(kind: Kind, identity: [u64; 3]) -> CandidateKey {
	CandidateKey { kind, feature: 0, identity }
}

/// A line along each axis of the frame through a point, known by where the point is so that it is the same line whatever put a point there.
fn lines_along_axes<'a>(context: &Context<'a>, position: DVec2) -> impl Iterator<Item = Candidate> + 'a {
	let axes = context.frame.axes().iter().enumerate();

	axes.filter_map(move |(axis_index, &axis)| {
		let identity = [u64::MAX, position_tag(position), axis_index as u64];
		Some(Candidate::track(key(Kind::Alignment, identity), Locus::line(position, axis)?).with_reference(position))
	})
}

fn highlighted(candidate: Candidate, highlight: Option<Highlight>) -> Candidate {
	Candidate { highlight, ..candidate }
}

/// Finds what awake targets offer wherever the pointer is: alignment, extensions, lines through two points, and lines touching or squarely meeting curves.
///
/// The result changes only with the scene, the frame, the settings, and what is awake or moving, so it can be kept between pointer moves.
pub fn find_constructions(context: &Context) -> Vec<Candidate> {
	let settings = context.settings;
	let woken = woken(context);
	let mut candidates = Vec::new();

	for target in &woken {
		match *target {
			Woken::Point { position, highlight, .. } if settings.alignment => {
				candidates.extend(lines_along_axes(context, position).map(|track| highlighted(track, highlight)));
			}
			Woken::Box { quad, identity, highlight } if settings.alignment => {
				let points = [quad[0], quad[1], quad[2], quad[3], (quad[0] + quad[2]) / 2.];
				let size = (quad[2] - quad[0]).length();

				for (axis_index, &axis) in context.frame.axes().iter().enumerate() {
					// Corners that share a line along this axis offer it once
					let mut offsets_across: Vec<f64> = Vec::new();

					for (point_index, &point) in points.iter().enumerate() {
						let across = axis.perp_dot(point);
						let tolerance = 1e-9 * size + 8. * f64::EPSILON * across.abs();
						if offsets_across.iter().any(|&offset| (offset - across).abs() <= tolerance) {
							continue;
						}
						offsets_across.push(across);

						let Some(line) = Locus::line(point, axis) else { continue };
						let track = Candidate::track(key(Kind::Alignment, [identity[0], identity[1], (axis_index * 8 + point_index) as u64]), line);
						candidates.push(track.with_reference(point).with_highlight(highlight));
					}
				}
			}
			Woken::Curve { curve, identity, highlight } if settings.extensions => {
				// A straight curve continues along one line from both ends, while a bent one leaves each end in its own direction
				let ends = if curve.is_straight() {
					vec![(curve.start(), 0.)]
				} else {
					vec![(curve.start(), 0.), (curve.end(), 1.)]
				};

				for (end_index, (end, t)) in ends.into_iter().enumerate() {
					let Some(line) = curve.tangent(t).and_then(|tangent| Locus::line(end, tangent)) else { continue };
					let track = Candidate::track(key(Kind::Extension, [identity[0], identity[1], end_index as u64]), line);
					candidates.push(track.with_reference(end).with_highlight(highlight));
				}
			}
			_ => {}
		}
	}

	// O(n^2) in the awake targets, of which there are only ever a handful
	for (index, first) in woken.iter().enumerate() {
		for second in &woken[index + 1..] {
			match (first, second) {
				(
					&Woken::Point { position, identity, .. },
					&Woken::Point {
						position: other,
						identity: other_identity,
						..
					},
				) if settings.constructions => {
					let pair = [fold(identity), fold(other_identity)];

					if let Some(line) = Locus::line_through(position, other) {
						candidates.push(Candidate::track(key(Kind::Through, [pair[0], pair[1], 0]), line).with_reference(position));
						candidates.push(Candidate::spot(key(Kind::Between, [pair[0], pair[1], 0]), (position + other) / 2.).with_reference(position));
					}
				}
				(
					&Woken::Curve { curve, identity, highlight },
					&Woken::Curve {
						curve: other,
						identity: other_identity,
						highlight: other_highlight,
					},
				) if settings.tangents => {
					let pair = [fold(identity), fold(other_identity)];

					for (ordinal, (s, t)) in common_tangents(&curve, &other).into_iter().enumerate() {
						let touches = [curve.evaluate(s), other.evaluate(t)];
						let Some(line) = Locus::line_through(touches[0], touches[1]) else { continue };
						let ordinal = ordinal as u64;

						candidates.push(
							Candidate::track(key(Kind::CommonTangent, [pair[0], pair[1], ordinal]), line)
								.with_reference(touches[0])
								.with_highlight(highlight),
						);
						candidates.push(
							Candidate::spot(key(Kind::TangentPoint, [pair[0], pair[1], ordinal * 2]), touches[0])
								.with_reference(touches[1])
								.with_highlight(highlight),
						);
						candidates.push(
							Candidate::spot(key(Kind::TangentPoint, [pair[0], pair[1], ordinal * 2 + 1]), touches[1])
								.with_reference(touches[0])
								.with_highlight(other_highlight),
						);
					}
				}
				(&Woken::Point { position, .. }, &Woken::Curve { curve, identity, highlight }) | (&Woken::Curve { curve, identity, highlight }, &Woken::Point { position, .. }) => {
					find_lines_from_previous(context, 0, position, &curve, place_in_layer(identity), highlight, &mut candidates);
				}
				_ => {}
			}
		}
	}

	candidates
}

/// Finds what a straight segment drawn from `previous` is offered: lines level and plumb with where it starts, and from awake targets their parallels, right angles, matching lengths, and lines touching or squarely meeting their curves.
///
/// `previous_length` is the length of the segment drawn before this one, which is offered to match as well.
pub fn find_constructions_from(context: &Context, previous: DVec2, previous_length: Option<f64>) -> Vec<Candidate> {
	let settings = context.settings;
	let mut candidates = Vec::new();

	if settings.alignment {
		candidates.extend(lines_along_axes(context, previous));
	}

	if settings.equal_lengths
		&& let Some(circle) = previous_length.and_then(|length| Locus::circle(previous, length))
	{
		candidates.push(Candidate::track(key(Kind::EqualLength, [u64::MAX, 0, 0]), circle).with_reference(previous));
	}

	for target in woken(context) {
		let Woken::Curve { curve, identity, highlight } = target else { continue };
		find_lines_from_previous(context, 0, previous, &curve, place_in_layer(identity), highlight, &mut candidates);

		let along = curve.end() - curve.start();
		let Some(direction) = along.try_normalize().filter(|_| curve.is_straight()) else { continue };
		let identity = [identity[0], identity[1], 0];

		// A parallel through a point already on the segment's own line would only repeat its extension
		let tolerance = 1e-9 * along.length() + 8. * f64::EPSILON * previous.abs().max_element();
		let on_its_line = direction.perp_dot(previous - curve.start()).abs() <= tolerance;
		if settings.constructions && !on_its_line {
			candidates.extend(Locus::line(previous, direction).map(|line| Candidate::track(key(Kind::Parallel, identity), line).with_reference(previous).with_highlight(highlight)));
		}
		if settings.constructions {
			candidates.extend(Locus::line(previous, direction.perp()).map(|line| Candidate::track(key(Kind::RightAngle, identity), line).with_reference(previous).with_highlight(highlight)));
		}
		if settings.equal_lengths {
			candidates.extend(Locus::circle(previous, along.length()).map(|circle| Candidate::track(key(Kind::EqualLength, identity), circle).with_reference(previous).with_highlight(highlight)));
		}
	}

	candidates
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::candidate::Shape;
	use crate::frame::Frame;
	use crate::scene::{Exclusions, GeometryRole, LayerId, LayerKind, Scene, SceneAnchor, SceneCurve, SceneLayer};
	use crate::settings::Settings;
	use crate::tunables::Tunables;
	use crate::view::ViewMapping;

	fn layer(anchors: &[DVec2], curves: &[Curve], quad: Option<[DVec2; 4]>) -> SceneLayer {
		SceneLayer {
			parent: None,
			kind: LayerKind::Artwork,
			quad,
			anchors: anchors
				.iter()
				.map(|&position| SceneAnchor {
					position,
					id: None,
					role: GeometryRole::Source,
				})
				.collect(),
			curves: curves
				.iter()
				.map(|&curve| SceneCurve {
					curve,
					ends: [None, None],
					role: GeometryRole::Source,
				})
				.collect(),
		}
	}

	fn dome(center: DVec2, radius: f64) -> Curve {
		let handle = radius * 4. / 3.;
		Curve::cubic(
			center + DVec2::new(-radius, 0.),
			center + DVec2::new(-radius, handle),
			center + DVec2::new(radius, handle),
			center + DVec2::new(radius, 0.),
		)
	}

	fn with_context<T>(scene: &Scene, awake: &[Awake], run: impl FnOnce(&Context) -> T) -> T {
		let (view, frame, tunables, settings, exclusions) = (ViewMapping::IDENTITY, Frame::default(), Tunables::default(), Settings::default(), Exclusions::default());
		run(&Context {
			scene,
			view: &view,
			frame: &frame,
			tunables: &tunables,
			settings: &settings,
			exclusions: &exclusions,
			awake,
		})
	}

	fn of_kind(candidates: &[Candidate], kind: Kind) -> Vec<&Candidate> {
		candidates.iter().filter(|candidate| candidate.key.kind == kind).collect()
	}

	fn track_passes_through(candidate: &Candidate, point: DVec2) -> bool {
		matches!(candidate.shape, Shape::Track(locus) if locus.nearest(point).abs_diff_eq(point, 1e-9))
	}

	#[test]
	fn nothing_awake_offers_nothing() {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), layer(&[DVec2::new(3., 4.)], &[], None));

		assert!(with_context(&scene, &[], find_constructions).is_empty());
	}

	#[test]
	fn an_awake_anchor_offers_a_line_along_each_axis_through_it() {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), layer(&[DVec2::new(3., 4.)], &[], None));
		let awake = [Awake::Anchor { layer: LayerId(1), index: 0 }];

		let candidates = with_context(&scene, &awake, find_constructions);

		let alignments = of_kind(&candidates, Kind::Alignment);
		assert_eq!(alignments.len(), 2);
		assert!(alignments.iter().any(|track| track_passes_through(track, DVec2::new(100., 4.))));
		assert!(alignments.iter().any(|track| track_passes_through(track, DVec2::new(3., -50.))));
	}

	#[test]
	fn an_awake_box_offers_its_edges_and_center_lines_once_each() {
		let quad = [DVec2::ZERO, DVec2::new(10., 0.), DVec2::new(10., 6.), DVec2::new(0., 6.)];
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), layer(&[], &[], Some(quad)));
		let awake = [Awake::Box { layer: LayerId(1) }];

		let candidates = with_context(&scene, &awake, find_constructions);

		// Along each axis that is two edges and one line through the center
		let alignments = of_kind(&candidates, Kind::Alignment);
		assert_eq!(alignments.len(), 6);
		for point in [
			DVec2::new(50., 0.),
			DVec2::new(50., 3.),
			DVec2::new(50., 6.),
			DVec2::new(0., 50.),
			DVec2::new(5., 50.),
			DVec2::new(10., 50.),
		] {
			assert_eq!(alignments.iter().filter(|track| track_passes_through(track, point)).count(), 1);
		}
	}

	#[test]
	fn an_awake_box_far_from_the_origin_still_offers_each_of_its_lines() {
		let far = DVec2::splat(1e12);
		let quad = [DVec2::ZERO, DVec2::new(10., 0.), DVec2::new(10., 6.), DVec2::new(0., 6.)].map(|corner| far + corner);
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), layer(&[], &[], Some(quad)));
		let awake = [Awake::Box { layer: LayerId(1) }];

		let candidates = with_context(&scene, &awake, find_constructions);

		assert_eq!(of_kind(&candidates, Kind::Alignment).len(), 6);
	}

	#[test]
	fn an_awake_curve_offers_the_extension_of_each_end() {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), layer(&[], &[Curve::line(DVec2::ZERO, DVec2::new(10., 10.)), dome(DVec2::new(50., 0.), 10.)], None));

		let straight = with_context(&scene, &[Awake::Curve { layer: LayerId(1), index: 0 }], find_constructions);
		let extensions = of_kind(&straight, Kind::Extension);
		assert_eq!(extensions.len(), 1);
		assert!(track_passes_through(extensions[0], DVec2::new(-30., -30.)) && track_passes_through(extensions[0], DVec2::new(30., 30.)));

		// Each end of the dome stands upright, so it continues along a vertical line
		let bent = with_context(&scene, &[Awake::Curve { layer: LayerId(1), index: 1 }], find_constructions);
		let extensions = of_kind(&bent, Kind::Extension);
		assert_eq!(extensions.len(), 2);
		assert!(extensions.iter().any(|track| track_passes_through(track, DVec2::new(40., -25.))));
		assert!(extensions.iter().any(|track| track_passes_through(track, DVec2::new(60., -25.))));
	}

	#[test]
	fn an_awake_point_on_an_awake_curve_offers_the_line_square_to_the_curve_there() {
		let slanted = Curve::line(DVec2::ZERO, DVec2::new(40., 30.));
		let bent = Curve::quadratic(DVec2::new(100., 0.), DVec2::new(150., 80.), DVec2::new(200., 0.));
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), layer(&[], &[slanted, bent], None));

		// The middle of the slanted line and a point partway along the bent curve, each awake with its curve
		let middle = DVec2::new(20., 15.);
		let partway = bent.evaluate(0.3);
		let awake = [
			Awake::Curve { layer: LayerId(1), index: 0 },
			Awake::Position { id: 1, position: middle },
			Awake::Curve { layer: LayerId(1), index: 1 },
			Awake::Position { id: 2, position: partway },
		];
		let candidates = with_context(&scene, &awake, find_constructions);
		let square = of_kind(&candidates, Kind::PerpendicularLine);

		// The line through the middle of the slanted one at a right angle to it, which bisects it
		assert!(
			square
				.iter()
				.any(|candidate| track_passes_through(candidate, middle) && track_passes_through(candidate, middle + DVec2::new(-30., 40.)))
		);

		// The line through the point on the bent curve along the curve's normal there
		let normal = bent.tangent(0.3).expect("the curve should have a direction there").perp();
		assert!(
			square
				.iter()
				.any(|candidate| track_passes_through(candidate, partway) && track_passes_through(candidate, partway + normal * 50.))
		);

		// No line lands back on either point, where it would leave in no direction in particular
		let lands_on = |point: DVec2| move |foot: &&Candidate| matches!(foot.shape, Shape::Spot(position) if position.distance(point) < 1e-6);
		let feet = of_kind(&candidates, Kind::PerpendicularFoot);
		assert!(!feet.iter().any(lands_on(middle)) && !feet.iter().any(lands_on(partway)));

		let from_partway = with_context(&scene, &[Awake::Curve { layer: LayerId(1), index: 1 }], |context| find_constructions_from(context, partway, None));
		assert!(!of_kind(&from_partway, Kind::PerpendicularFoot).iter().any(lands_on(partway)));
	}

	#[test]
	fn two_awake_points_offer_the_line_through_them_and_the_point_between() {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), layer(&[DVec2::ZERO], &[], None));
		let awake = [Awake::Anchor { layer: LayerId(1), index: 0 }, Awake::Position { id: 7, position: DVec2::new(8., 6.) }];

		let candidates = with_context(&scene, &awake, find_constructions);

		assert!(track_passes_through(of_kind(&candidates, Kind::Through)[0], DVec2::new(16., 12.)));
		assert_eq!(of_kind(&candidates, Kind::Between)[0].shape, Shape::Spot(DVec2::new(4., 3.)));
	}

	#[test]
	fn two_awake_humps_offer_the_line_resting_across_them_and_where_it_touches() {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), layer(&[], &[dome(DVec2::ZERO, 10.)], None));
		scene.set_layer(LayerId(2), layer(&[], &[dome(DVec2::new(40., 0.), 10.)], None));
		let awake = [Awake::Curve { layer: LayerId(1), index: 0 }, Awake::Curve { layer: LayerId(2), index: 0 }];

		let candidates = with_context(&scene, &awake, find_constructions);

		let common = of_kind(&candidates, Kind::CommonTangent);
		assert_eq!(common.len(), 1);
		assert!(track_passes_through(common[0], DVec2::new(-100., 10.)) && track_passes_through(common[0], DVec2::new(100., 10.)));

		// Each touch point belongs to the hump it lies on
		let touches = of_kind(&candidates, Kind::TangentPoint);
		assert_eq!(touches.len(), 2);
		for (position, layer) in [(DVec2::new(0., 10.), LayerId(1)), (DVec2::new(40., 10.), LayerId(2))] {
			assert!(
				touches
					.iter()
					.any(|touch| matches!(touch.shape, Shape::Spot(at) if at.abs_diff_eq(position, 1e-9)) && touch.highlight == Some(Highlight::Curve { layer, index: 0 }))
			);
		}
	}

	#[test]
	fn what_awake_targets_offer_is_named_the_same_whatever_order_they_woke_in() {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), layer(&[DVec2::new(-30., 0.)], &[dome(DVec2::ZERO, 10.)], None));
		scene.set_layer(LayerId(2), layer(&[DVec2::new(80., 5.)], &[dome(DVec2::new(40., 0.), 14.)], None));
		let awake = [
			Awake::Curve { layer: LayerId(1), index: 0 },
			Awake::Anchor { layer: LayerId(2), index: 0 },
			Awake::Curve { layer: LayerId(2), index: 0 },
			Awake::Anchor { layer: LayerId(1), index: 0 },
		];
		let offered = |awake: &[Awake]| {
			let mut offered: Vec<(CandidateKey, Shape)> = with_context(&scene, awake, find_constructions).iter().map(|candidate| (candidate.key, candidate.shape)).collect();
			offered.sort_by_key(|&(key, _)| key);
			offered
		};
		let expected = offered(&awake);
		assert!(!of_kind(&with_context(&scene, &awake, find_constructions), Kind::CommonTangent).is_empty());

		let mut reordered = awake;
		for _ in 0..awake.len() {
			reordered.rotate_left(1);
			assert_eq!(offered(&reordered), expected);

			let reversed: Vec<Awake> = reordered.iter().rev().copied().collect();
			assert_eq!(offered(&reversed), expected);
		}

		// A target listed twice is only awake once
		let twice: Vec<Awake> = awake.iter().chain(&awake).copied().collect();
		assert_eq!(offered(&twice), expected);
	}

	#[test]
	fn an_awake_point_and_curve_offer_the_lines_from_one_that_touch_the_other() {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), layer(&[], &[dome(DVec2::ZERO, 10.)], None));
		let outside = DVec2::new(-30., 0.);
		let awake = [Awake::Position { id: 1, position: outside }, Awake::Curve { layer: LayerId(1), index: 0 }];

		let candidates = with_context(&scene, &awake, find_constructions);

		let tangent_lines = of_kind(&candidates, Kind::TangentLine);
		assert_eq!(tangent_lines.len(), 1);
		assert!(track_passes_through(tangent_lines[0], outside));
		assert_eq!(of_kind(&candidates, Kind::TangentPoint).len(), 1);
	}

	#[test]
	fn a_previous_point_gains_parallels_right_angles_and_matching_lengths_from_awake_segments() {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), layer(&[], &[Curve::line(DVec2::ZERO, DVec2::new(30., 40.))], None));
		let awake = [Awake::Curve { layer: LayerId(1), index: 0 }];
		let previous = DVec2::new(100., 0.);

		let candidates = with_context(&scene, &awake, |context| find_constructions_from(context, previous, Some(7.)));

		assert!(track_passes_through(of_kind(&candidates, Kind::Parallel)[0], previous + DVec2::new(3., 4.)));
		assert!(track_passes_through(of_kind(&candidates, Kind::RightAngle)[0], previous + DVec2::new(-4., 3.)));

		// One circle matches the awake segment's length of 50, and one the length of the segment drawn before
		let lengths = of_kind(&candidates, Kind::EqualLength);
		assert_eq!(lengths.len(), 2);
		assert!(lengths.iter().any(|track| track_passes_through(track, previous + DVec2::new(50., 0.))));
		assert!(lengths.iter().any(|track| track_passes_through(track, previous + DVec2::new(0., 7.))));
	}

	#[test]
	fn a_previous_point_offers_lines_level_and_plumb_with_it_as_an_awake_point_there_would() {
		let previous = DVec2::new(100., 40.);
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), layer(&[previous], &[], None));

		let from_previous = with_context(&scene, &[], |context| find_constructions_from(context, previous, None));
		let lines = of_kind(&from_previous, Kind::Alignment);
		assert_eq!(lines.len(), 2);
		assert!(lines.iter().any(|track| track_passes_through(track, DVec2::new(-500., 40.))));
		assert!(lines.iter().any(|track| track_passes_through(track, DVec2::new(100., 900.))));

		// An anchor awake at that very place offers the same lines, known by the same keys so that neither is counted twice
		let from_the_anchor = with_context(&scene, &[Awake::Anchor { layer: LayerId(1), index: 0 }], find_constructions);
		let mut keys = [&from_previous, &from_the_anchor].map(|candidates| of_kind(candidates, Kind::Alignment).iter().map(|track| track.key).collect::<Vec<_>>());
		keys.iter_mut().for_each(|keys| keys.sort());
		assert_eq!(keys[0], keys[1]);
	}

	#[test]
	fn a_previous_point_on_a_segments_own_line_gets_no_parallel() {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), layer(&[], &[Curve::line(DVec2::ZERO, DVec2::new(30., 40.))], None));
		let awake = [Awake::Curve { layer: LayerId(1), index: 0 }];

		let candidates = with_context(&scene, &awake, |context| find_constructions_from(context, DVec2::new(60., 80.), None));

		assert!(of_kind(&candidates, Kind::Parallel).is_empty());
		assert_eq!(of_kind(&candidates, Kind::RightAngle).len(), 1);
	}

	#[test]
	fn awake_targets_that_have_gone_or_are_moving_offer_nothing() {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), layer(&[DVec2::new(3., 4.)], &[], None));

		let gone = [Awake::Anchor { layer: LayerId(9), index: 0 }, Awake::Anchor { layer: LayerId(1), index: 5 }];
		assert!(with_context(&scene, &gone, find_constructions).is_empty());

		let (view, frame, tunables, settings) = (ViewMapping::IDENTITY, Frame::default(), Tunables::default(), Settings::default());
		let exclusions = Exclusions {
			layers: [LayerId(1)].into(),
			..Default::default()
		};
		let context = Context {
			scene: &scene,
			view: &view,
			frame: &frame,
			tunables: &tunables,
			settings: &settings,
			exclusions: &exclusions,
			awake: &[Awake::Anchor { layer: LayerId(1), index: 0 }],
		};
		assert!(find_constructions(&context).is_empty());
	}
}
