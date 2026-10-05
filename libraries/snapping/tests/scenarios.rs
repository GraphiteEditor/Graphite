//! Whole gestures played through the engine, each checking a behavior someone using the editor would notice.

use glam::{DAffine2, DMat3, DVec2, DVec3};
use snapping::{AnchorId, CrossingCache, FeatureMap};
use snapping::{
	Awake, Candidate, Context, Curve, Exclusions, Frame, GeometryRole, Grid, Incumbent, Kind, LayerId, LayerKind, Locus, Outcome, Request, Scene, SceneAnchor, SceneCurve, SceneLayer, Settings, Snap,
	Tunables, ViewMapping, find_constructions, find_constructions_from, snap_features, snap_point, snap_slide,
};
use std::collections::{HashMap, HashSet};

/// A scene and a way of looking at it, with the state a gesture carries from one pointer position to the next.
struct Stage {
	scene: Scene,
	view: ViewMapping,
	frame: Frame,
	tunables: Tunables,
	settings: Settings,
	exclusions: Exclusions,
	awake: Vec<Awake>,
	grid: Option<Grid>,
	incumbent: Option<Incumbent>,
	crossings: CrossingCache,
}

impl Stage {
	fn new(layers: impl IntoIterator<Item = SceneLayer>) -> Self {
		let mut scene = Scene::new();
		for (index, layer) in layers.into_iter().enumerate() {
			scene.set_layer(LayerId(index as u64 + 1), layer);
		}
		scene.prepare();

		Self {
			scene,
			view: ViewMapping::IDENTITY,
			frame: Frame::default(),
			tunables: Tunables::default(),
			settings: Settings::default(),
			exclusions: Exclusions::default(),
			awake: Vec::new(),
			grid: None,
			incumbent: None,
			crossings: CrossingCache::default(),
		}
	}

	fn with_context<T>(&self, run: impl FnOnce(&Context) -> T) -> T {
		run(&Context {
			scene: &self.scene,
			view: &self.view,
			frame: &self.frame,
			tunables: &self.tunables,
			settings: &self.settings,
			exclusions: &self.exclusions,
			awake: &self.awake,
		})
	}

	fn request<'a>(&'a self, constructions: &'a [Candidate], required: Option<Locus>) -> Request<'a> {
		Request {
			grid: self.grid.as_ref(),
			required,
			incumbent: self.incumbent,
			cycle: 0,
			quiet: false,
			constructions,
			lead_constructions: &[],
			crossings: Some(&self.crossings),
		}
	}

	/// Moves the pointer while placing a point, remembering what engaged for the next move.
	fn point(&mut self, free: DVec2, previous: Option<DVec2>) -> Outcome {
		self.point_along(free, previous, None)
	}

	fn point_along(&mut self, free: DVec2, previous: Option<DVec2>, required: Option<Locus>) -> Outcome {
		let outcome = self.with_context(|context| {
			let constructions = find_constructions(context);
			let lead_constructions = previous.map(|previous| find_constructions_from(context, previous, None)).unwrap_or_default();

			let request = Request {
				lead_constructions: &lead_constructions,
				..self.request(&constructions, required)
			};
			snap_point(context, &request, free, previous)
		});

		self.incumbent = outcome.incumbent();
		outcome
	}

	/// Moves the pointer while sliding a set of features, remembering what engaged for the next move.
	fn slide(&mut self, feature_starts: &[DVec2], free: DVec2) -> Outcome {
		let outcome = self.with_context(|context| {
			let constructions = find_constructions(context);
			snap_slide(context, &self.request(&constructions, None), feature_starts, 0, free)
		});

		self.incumbent = outcome.incumbent();
		outcome
	}
}

fn engaged_kinds(outcome: &Outcome) -> Vec<Kind> {
	let mut kinds: Vec<Kind> = outcome.engaged_candidates().map(|candidate| candidate.key.kind).collect();
	kinds.sort();
	kinds
}

fn path(points: &[DVec2], closed: bool) -> SceneLayer {
	let count = points.len();
	let segments = if closed { count } else { count - 1 };

	SceneLayer {
		parent: None,
		kind: LayerKind::Artwork,
		quad: None,
		anchors: (0..count)
			.map(|index| SceneAnchor {
				position: points[index],
				id: Some(AnchorId(index as u64)),
				role: GeometryRole::Source,
			})
			.collect(),
		curves: (0..segments)
			.map(|index| SceneCurve {
				curve: Curve::line(points[index], points[(index + 1) % count]),
				ends: [Some(AnchorId(index as u64)), Some(AnchorId(((index + 1) % count) as u64))],
				role: GeometryRole::Source,
			})
			.collect(),
	}
}

fn rectangle(min: DVec2, max: DVec2) -> SceneLayer {
	let corners = [min, DVec2::new(max.x, min.y), max, DVec2::new(min.x, max.y)];
	SceneLayer {
		quad: Some(corners),
		..path(&corners, true)
	}
}

/// A floor seen from above its near edge, where plane points further along Y shrink toward a horizon at the screen line Y = 1000.
fn perspective() -> ViewMapping {
	ViewMapping::from_homography(DMat3::from_cols(DVec3::new(100., 0., 0.), DVec3::new(0., 100., 0.1), DVec3::new(400., 300., 1.))).unwrap()
}

/// A hump rising from the X axis, which is the upper half of a circle approximated by one cubic.
fn hump(center_x: f64, radius: f64) -> SceneLayer {
	let handle = radius * 4. / 3.;
	let curve = Curve::cubic(
		DVec2::new(center_x - radius, 0.),
		DVec2::new(center_x - radius, handle),
		DVec2::new(center_x + radius, handle),
		DVec2::new(center_x + radius, 0.),
	);

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

#[test]
fn a_line_end_brought_near_a_corner_lands_on_its_anchor() {
	let mut stage = Stage::new([rectangle(DVec2::ZERO, DVec2::new(100., 60.))]);

	let outcome = stage.point(DVec2::new(104., 63.), Some(DVec2::new(300., 200.)));

	assert_eq!(outcome.parameters, DVec2::new(100., 60.));
	assert_eq!(engaged_kinds(&outcome), vec![Kind::Anchor]);
}

#[test]
fn a_line_end_brought_near_an_edge_slides_along_it_until_a_point_takes_over() {
	let mut stage = Stage::new([rectangle(DVec2::ZERO, DVec2::new(100., 60.))]);

	let along_the_edge = stage.point(DVec2::new(30., -3.), None);
	assert_eq!(along_the_edge.parameters, DVec2::new(30., 0.));
	assert_eq!(engaged_kinds(&along_the_edge), vec![Kind::OnCurve]);

	let at_the_midpoint = stage.point(DVec2::new(46., -3.), None);
	assert_eq!(at_the_midpoint.parameters, DVec2::new(50., 0.));
	assert_eq!(engaged_kinds(&at_the_midpoint), vec![Kind::Midpoint]);
}

#[test]
fn two_crossing_paths_offer_their_intersection() {
	// The paths cross away from the middle of either, so their midpoints do not compete
	let mut stage = Stage::new([path(&[DVec2::new(0., 0.), DVec2::new(100., 100.)], false), path(&[DVec2::new(0., 60.), DVec2::new(90., 0.)], false)]);

	let outcome = stage.point(DVec2::new(38., 35.), None);

	assert!(outcome.parameters.abs_diff_eq(DVec2::new(36., 36.), 1e-9));
	assert_eq!(engaged_kinds(&outcome), vec![Kind::OnCurve, Kind::OnCurve]);
}

#[test]
fn an_engaged_snap_holds_until_the_pointer_has_clearly_left() {
	let mut stage = Stage::new([path(&[DVec2::ZERO, DVec2::new(0., -200.)], false)]);

	// The end of the path is caught within reach, then held a little beyond it, then let go
	let positions = [(20., None), (7., Some(DVec2::ZERO)), (11., Some(DVec2::ZERO)), (13., None), (11., None)];
	for (distance, expected) in positions {
		let free = DVec2::new(distance, distance * 0.01);
		let outcome = stage.point(free, None);

		assert_eq!(outcome.parameters, expected.unwrap_or(free), "at a distance of {distance}");
	}
}

#[test]
fn a_line_drawn_toward_a_hump_touches_it_tangentially() {
	let mut stage = Stage::new([hump(0., 10.)]);
	let previous = DVec2::new(-30., 0.);

	// Near the left shoulder the end lands exactly where the line from the previous point touches the hump
	let outcome = stage.point(DVec2::new(-4., 9.5), Some(previous));
	assert_eq!(engaged_kinds(&outcome), vec![Kind::TangentPoint]);

	let touch = outcome.parameters;
	let curve = stage.scene.layer(LayerId(1)).unwrap().curves[0].curve;
	let on_the_curve = curve.nearest(touch);
	assert!(on_the_curve.distance < 1e-9);
	assert!(curve.tangent(on_the_curve.t).unwrap().perp_dot((touch - previous).normalize()).abs() < 1e-9);
}

#[test]
fn a_line_passing_an_awake_hump_grazes_its_crest_on_the_way_past() {
	let mut stage = Stage::new([hump(0., 10.)]);
	stage.awake = vec![Awake::Curve { layer: LayerId(1), index: 0 }];
	let previous = DVec2::new(-30., 0.);

	// The end is far beyond the hump, where nothing of the hump itself is within reach, yet the line still rests on its shoulder
	let far_side = DVec2::new(40., 26.);
	let outcome = stage.point(far_side, Some(previous));
	assert_eq!(engaged_kinds(&outcome), vec![Kind::TangentLine]);

	let curve = stage.scene.layer(LayerId(1)).unwrap().curves[0].curve;
	let crossings = curve.line_intersections(previous, outcome.parameters - previous);
	assert_eq!(crossings.len(), 1, "a tangent line meets the hump at just one place");
	assert!(outcome.parameters.distance(far_side) < stage.tunables.reach);
}

#[test]
fn a_hump_that_is_not_awake_offers_nothing_from_afar() {
	let mut stage = Stage::new([hump(0., 10.)]);

	let far_side = DVec2::new(40., 26.);
	let outcome = stage.point(far_side, Some(DVec2::new(-30., 0.)));

	assert_eq!(outcome.snap, Snap::Free);
	assert_eq!(outcome.parameters, far_side);
}

#[test]
fn a_line_can_be_laid_across_two_awake_humps_from_before_its_first_point() {
	let mut stage = Stage::new([hump(0., 10.), hump(40., 10.)]);
	stage.awake = vec![Awake::Curve { layer: LayerId(1), index: 0 }, Awake::Curve { layer: LayerId(2), index: 0 }];

	// Approaching the first hump's peak with no point placed yet finds where the shared line touches it
	let start = stage.point(DVec2::new(2., 12.), None);
	assert_eq!(engaged_kinds(&start), vec![Kind::TangentPoint]);
	assert!(start.parameters.abs_diff_eq(DVec2::new(0., 10.), 1e-9));

	// With that as the first point, the end lands on the second hump's peak
	stage.incumbent = None;
	let end = stage.point(DVec2::new(41., 12.), Some(start.parameters));
	assert!(end.parameters.abs_diff_eq(DVec2::new(40., 10.), 1e-9));

	// The first point may equally sit before the first hump, anywhere along the shared line
	stage.incumbent = None;
	let before = stage.point(DVec2::new(-25., 13.), None);
	assert_eq!(engaged_kinds(&before), vec![Kind::CommonTangent]);
	assert!(before.parameters.abs_diff_eq(DVec2::new(-25., 10.), 1e-9));
}

#[test]
fn the_end_of_a_segment_never_lands_back_on_the_point_it_is_drawn_from() {
	// The segment starts on the corner of a rectangle, where an anchor sits and two edges and two lines of alignment all meet
	let mut stage = Stage::new([rectangle(DVec2::ZERO, DVec2::new(100., 60.))]);
	let previous = DVec2::new(100., 60.);

	for free in [previous, previous + DVec2::new(3., 2.), previous + DVec2::new(-2., 4.), previous + DVec2::new(5., 5.)] {
		let outcome = stage.point(free, Some(previous));

		assert!(outcome.parameters.distance(previous) > 1e-6 || free == previous, "the end went back to the start from {free:?}");
		assert!(engaged_kinds(&outcome).iter().all(|kind| !matches!(kind, Kind::Anchor | Kind::BoxCorner)));
	}

	// With no segment being drawn, the corner catches a point as usual
	stage.incumbent = None;
	assert_eq!(stage.point(previous + DVec2::new(3., 2.), None).parameters, previous);
}

#[test]
fn a_line_drawn_nearly_level_with_where_it_started_comes_level() {
	let mut stage = Stage::new([]);
	let previous = DVec2::new(50., 80.);

	let nearly_level = stage.point(DVec2::new(250., 85.), Some(previous));
	assert_eq!(nearly_level.parameters, DVec2::new(250., 80.));
	assert_eq!(engaged_kinds(&nearly_level), vec![Kind::Alignment]);

	// Further from level than reach, the line goes where it is asked to
	stage.incumbent = None;
	let sloping = DVec2::new(250., 95.);
	assert_eq!(stage.point(sloping, Some(previous)).parameters, sloping);
}

#[test]
fn an_awake_point_lines_the_pointer_up_with_it_along_each_axis() {
	let mut stage = Stage::new([path(&[DVec2::new(100., 100.), DVec2::new(100., 300.)], false)]);
	stage.awake = vec![Awake::Anchor { layer: LayerId(1), index: 0 }];

	let level_with_it = stage.point(DVec2::new(400., 103.), None);
	assert_eq!(level_with_it.parameters, DVec2::new(400., 100.));
	assert_eq!(engaged_kinds(&level_with_it), vec![Kind::Alignment]);

	// Without being awake, a point that far away offers nothing
	stage.awake.clear();
	stage.incumbent = None;
	assert_eq!(stage.point(DVec2::new(400., 103.), None).snap, Snap::Free);
}

#[test]
fn a_required_track_still_snaps_to_what_it_crosses() {
	let mut stage = Stage::new([path(&[DVec2::new(200., -100.), DVec2::new(200., 100.)], false)]);
	let previous = DVec2::ZERO;
	let locked_level = Locus::line(previous, DVec2::X);

	// The pointer wanders off the locked line, yet the point stays on it and catches where it crosses the path
	let outcome = stage.point_along(DVec2::new(196., 30.), Some(previous), locked_level);
	assert!(outcome.parameters.abs_diff_eq(DVec2::new(200., 0.), 1e-9));

	stage.incumbent = None;
	let elsewhere = stage.point_along(DVec2::new(120., 30.), Some(previous), locked_level);
	assert!(elsewhere.parameters.abs_diff_eq(DVec2::new(120., 0.), 1e-9));
	assert_eq!(elsewhere.snap, Snap::Free);
}

#[test]
fn a_dragged_anchor_snaps_to_the_rest_of_its_own_path_but_not_to_itself() {
	let corners = [DVec2::ZERO, DVec2::new(100., 0.), DVec2::new(100., 60.), DVec2::new(0., 60.)];
	let mut stage = Stage::new([path(&corners, true)]);
	stage.exclusions.anchors = HashMap::from([(LayerId(1), HashSet::from([AnchorId(2)]))]);

	// Right beside where it began, nothing of its own former position or the edges that move with it holds it back
	let near_its_start = DVec2::new(103., 57.);
	assert_eq!(stage.point(near_its_start, None).snap, Snap::Free);

	// Yet the path's other corners and unmoving edges still catch it
	stage.incumbent = None;
	assert_eq!(stage.point(DVec2::new(3., 57.), None).parameters, DVec2::new(0., 60.));
	stage.incumbent = None;
	assert_eq!(stage.point(DVec2::new(40., 3.), None).parameters, DVec2::new(40., 0.));
}

#[test]
fn a_sliding_box_meets_another_corner_to_corner() {
	let mut stage = Stage::new([rectangle(DVec2::new(200., 0.), DVec2::new(300., 60.))]);

	// The moving box began at the origin, and its four corners are its features
	let corners = [DVec2::ZERO, DVec2::new(100., 0.), DVec2::new(100., 60.), DVec2::new(0., 60.)];
	let outcome = stage.slide(&corners, DVec2::new(97., -3.));

	// Its right side comes to rest against the other's left side with their corners together
	assert_eq!(outcome.parameters, DVec2::new(100., 0.));
}

#[test]
fn a_sliding_box_lines_up_with_two_awake_boxes_one_in_each_direction() {
	let mut stage = Stage::new([rectangle(DVec2::new(500., 40.), DVec2::new(560., 90.)), rectangle(DVec2::new(130., 400.), DVec2::new(190., 460.))]);
	stage.awake = vec![Awake::Box { layer: LayerId(1) }, Awake::Box { layer: LayerId(2) }];

	let corners = [DVec2::ZERO, DVec2::new(50., 0.), DVec2::new(50., 50.), DVec2::new(0., 50.)];
	let outcome = stage.slide(&corners, DVec2::new(127., 43.));

	// Its top comes level with the first box's top, and its left side with the second box's left side
	assert!(outcome.parameters.abs_diff_eq(DVec2::new(130., 40.), 1e-9));
	assert_eq!(engaged_kinds(&outcome), vec![Kind::Alignment, Kind::Alignment]);
}

#[test]
fn a_line_growing_from_its_center_snaps_at_either_end() {
	let stage = Stage::new([path(&[DVec2::new(-80., -100.), DVec2::new(-80., 100.)], false)]);
	let center = DVec2::ZERO;

	// One end follows the pointer and the other mirrors it through the center
	let features = [
		FeatureMap {
			matrix: glam::DMat2::IDENTITY,
			offset: center,
		},
		FeatureMap {
			matrix: glam::DMat2::from_diagonal(DVec2::splat(-1.)),
			offset: center,
		},
	];

	// The pointer is to the right, where there is nothing, while the mirrored end comes within reach of the path on the left
	let outcome = stage.with_context(|context| snap_features(context, &stage.request(&[], None), &features, 0, DVec2::new(77., 40.), None));

	assert!(outcome.parameters.abs_diff_eq(DVec2::new(80., 40.), 1e-9));
	assert_eq!(engaged_kinds(&outcome), vec![Kind::OnCurve]);
}

#[test]
fn with_the_grid_on_a_point_lands_on_it_unless_something_real_is_within_reach() {
	let mut stage = Stage::new([path(&[DVec2::new(133., 262.), DVec2::new(133., 900.)], false)]);
	stage.grid = Grid::rectangular(DVec2::ZERO, DVec2::splat(100.));

	let nothing_nearby = stage.point(DVec2::new(480., 530.), None);
	assert_eq!((nothing_nearby.parameters, nothing_nearby.snap.clone()), (DVec2::new(500., 500.), Snap::Grid));

	// The end of the path sits off the grid, and the point can still be attached to it
	let near_the_anchor = stage.point(DVec2::new(130., 260.), None);
	assert_eq!(near_the_anchor.parameters, DVec2::new(133., 262.));
}

#[test]
fn the_same_gesture_on_screen_gives_the_same_snap_at_any_zoom() {
	for zoom in [0.05, 1., 40.] {
		let mut stage = Stage::new([rectangle(DVec2::ZERO, DVec2::new(100., 60.))]);
		stage.view = ViewMapping::from_affine(DAffine2::from_scale_angle_translation(DVec2::splat(zoom), 0.4, DVec2::new(30., 70.))).unwrap();

		// Just under 8 pixels from the corner on screen is within reach, however large the document appears
		let corner = stage.view.to_screen(DVec2::new(100., 60.));
		let within_reach = stage.view.to_plane(corner + DVec2::new(5., 6.));
		assert_eq!(stage.point(within_reach, None).parameters, DVec2::new(100., 60.), "at a zoom of {zoom}");

		// Six pixels across and six down is over 8 pixels away, though neither direction alone is
		stage.incumbent = None;
		let beyond_reach = stage.view.to_plane(corner + DVec2::new(6., 6.));
		assert_eq!(stage.point(beyond_reach, None).snap, Snap::Free, "at a zoom of {zoom}");
	}
}

#[test]
fn a_view_in_perspective_judges_reach_on_screen_and_lands_on_the_grid_of_the_plane() {
	let mut stage = Stage::new([path(&[DVec2::new(0.5, 5.), DVec2::new(0.5, 6.)], false), path(&[DVec2::new(2.6, 0.), DVec2::new(2.6, -1.)], false)]);
	stage.view = perspective();
	stage.grid = Grid::rectangular(DVec2::ZERO, DVec2::splat(1.));

	// The same small step off the end of a path is within reach far away, where it covers 3 pixels, and out of reach up close, where it covers 9
	let [far_end, near_end] = [DVec2::new(0.5, 5.), DVec2::new(2.6, 0.)];
	let step = DVec2::new(0., 0.09);
	assert_eq!(stage.point(far_end - step, None).parameters, far_end);

	// Out of reach of the path, the point lands on the plane's own grid, which is evenly spaced there though not on screen
	stage.incumbent = None;
	let landed = stage.point(near_end + step, None);
	assert_eq!((landed.parameters, landed.snap), (DVec2::new(3., 0.), Snap::Grid));
}

#[test]
fn a_pointer_with_no_place_in_the_plane_snaps_to_nothing() {
	let mut stage = Stage::new([path(&[DVec2::new(0.5, 5.), DVec2::new(0.5, 6.)], false)]);
	stage.view = perspective();
	stage.grid = Grid::rectangular(DVec2::ZERO, DVec2::splat(1.));

	// The plane never reaches this part of the screen, which is past its horizon
	let past_the_horizon = stage.view.to_plane(DVec2::new(400., 1200.));
	let outcome = stage.point(past_the_horizon, None);
	assert!(!outcome.parameters.is_finite());
	assert_eq!(outcome.snap, Snap::Free);

	// A position behind the viewer is likewise left alone, by the grid as much as by the path
	let behind = DVec2::new(0., -20.);
	let outcome = stage.point(behind, None);
	assert_eq!((outcome.parameters, outcome.snap), (behind, Snap::Free));
}

#[test]
fn holding_snapping_quiet_leaves_only_a_required_track_in_effect() {
	let stage = Stage::new([rectangle(DVec2::ZERO, DVec2::new(100., 60.))]);
	let grid = Grid::rectangular(DVec2::ZERO, DVec2::splat(10.)).unwrap();
	let quiet = |required: Option<Locus>| Request {
		quiet: true,
		grid: Some(&grid),
		required,
		..Default::default()
	};

	// Right beside a corner and with the grid on, nothing takes hold
	let free = DVec2::new(103., 62.);
	let outcome = stage.with_context(|context| snap_point(context, &quiet(None), free, None));
	assert_eq!((outcome.parameters, outcome.snap), (free, Snap::Free));

	// A locked direction still confines the point
	let level = Locus::line(DVec2::new(0., 70.), DVec2::X);
	let outcome = stage.with_context(|context| snap_point(context, &quiet(level), free, None));
	assert_eq!((outcome.parameters, outcome.snap), (DVec2::new(103., 70.), Snap::Free));
}

#[test]
fn a_required_track_snaps_to_what_is_near_the_point_however_far_the_pointer_strays() {
	let mut stage = Stage::new([path(&[DVec2::new(200., 0.), DVec2::new(200., 100.)], false)]);
	let previous = DVec2::ZERO;
	let locked_level = Locus::line(previous, DVec2::X);

	// The pointer is 150 pixels from the locked line, far from the path, yet the point it leaves on that line is right beside the path's end
	let outcome = stage.point_along(DVec2::new(197., -150.), Some(previous), locked_level);

	assert_eq!(outcome.parameters, DVec2::new(200., 0.));
	assert_eq!(engaged_kinds(&outcome), vec![Kind::Anchor]);
}

#[test]
fn a_curve_both_awake_and_near_the_pointer_offers_each_relationship_once() {
	// The line is being drawn on from the end of a path, which is awake along with the hump
	let previous = DVec2::new(-30., 0.);
	let mut stage = Stage::new([hump(0., 10.), path(&[DVec2::new(-60., 0.), previous], false)]);
	stage.awake = vec![Awake::Curve { layer: LayerId(1), index: 0 }, Awake::Anchor { layer: LayerId(2), index: 1 }];

	let outcome = stage.point(DVec2::new(-4., 9.5), Some(previous));

	// The lines from that end to the hump are found near the pointer, from the point drawn from, and between the two awake targets
	for (index, candidate) in outcome.candidates.iter().enumerate() {
		let repeats = |earlier: &Candidate| earlier.key == candidate.key || (earlier.key.kind == candidate.key.kind && earlier.shape == candidate.shape);
		assert!(!outcome.candidates[..index].iter().any(repeats), "{candidate:?} is offered twice");
	}
	assert_eq!(engaged_kinds(&outcome), vec![Kind::TangentPoint]);
}

#[test]
fn an_engaged_snap_holds_on_though_the_awake_targets_are_listed_in_another_order() {
	let mut stage = Stage::new([hump(0., 10.), hump(40., 10.)]);
	stage.awake = vec![Awake::Curve { layer: LayerId(1), index: 0 }, Awake::Curve { layer: LayerId(2), index: 0 }];

	let engaged = stage.point(DVec2::new(-25., 13.), None);
	assert_eq!(engaged_kinds(&engaged), vec![Kind::CommonTangent]);

	// Beyond reach of the shared line, only being recognized as the snap already engaged keeps the point on it
	stage.awake.reverse();
	let held = stage.point(DVec2::new(-25., 20.5), None);
	assert!(held.parameters.abs_diff_eq(DVec2::new(-25., 10.), 1e-9));
}

#[test]
fn a_candidate_cycled_to_stays_engaged_while_the_pointer_rests_there() {
	let mut stage = Stage::new([rectangle(DVec2::ZERO, DVec2::new(100., 60.))]);
	let free = DVec2::new(103., 62.);

	let shown = stage.point(free, None);
	assert_eq!(engaged_kinds(&shown), vec![Kind::Anchor]);

	// One step on is some other relationship at that corner
	let stepped = stage.with_context(|context| {
		let request = Request { cycle: 1, ..stage.request(&[], None) };
		snap_point(context, &request, free, None)
	});
	let key = |outcome: &Outcome| outcome.engagement().map(|engagement| engagement.key);
	assert_ne!(key(&stepped), key(&shown));

	// It is still the one engaged a moment later, with no further stepping
	stage.incumbent = stepped.incumbent();
	let a_moment_later = stage.point(free + DVec2::new(0.3, 0.), None);
	assert_eq!(key(&a_moment_later), key(&stepped));
}

#[test]
fn a_point_riding_a_path_from_well_off_it_catches_the_midpoint_and_lets_it_go_without_flickering() {
	let mut stage = Stage::new([path(&[DVec2::ZERO, DVec2::new(200., 0.)], false)]);
	let midpoint = DVec2::new(100., 0.);

	// The path is engaged up close, then ridden from over 10 pixels away, where the midpoint is never within reach of the pointer itself
	assert_eq!(stage.point(DVec2::new(50., 5.), None).parameters, DVec2::new(50., 0.));
	let mut lands_on = |x: f64, expected: DVec2| {
		let landed = stage.point(DVec2::new(x, 10.5), None).parameters;
		assert!(landed.abs_diff_eq(expected, 1e-9), "riding at {x} landed on {landed:?} and not {expected:?}");
	};
	lands_on(50., DVec2::new(50., 0.));

	// It is caught 8 pixels along the path from it and kept until 12 pixels beyond it
	lands_on(91., DVec2::new(91., 0.));
	for x in [95., 100., 105.7, 105.9, 105.7, 105.9, 111.9] {
		lands_on(x, midpoint);
	}
	lands_on(112.5, DVec2::new(112.5, 0.));

	// Having let go, it is not caught again until back within 8 pixels along
	lands_on(110., DVec2::new(110., 0.));
	lands_on(107., midpoint);
}

#[test]
fn leaving_a_point_that_was_met_directly_lets_go_once_and_for_all() {
	// An awake anchor offers lines level with it and plumb with it, and the corner of a box has its edges running through it
	let mut stage = Stage::new([path(&[DVec2::ZERO], false), rectangle(DVec2::new(500., 500.), DVec2::new(600., 560.))]);
	stage.awake = vec![Awake::Anchor { layer: LayerId(1), index: 0 }];

	for (met, away) in [(DVec2::ZERO, DVec2::from_angle(0.7)), (DVec2::new(500., 500.), DVec2::ONE.normalize())] {
		stage.incumbent = None;
		assert_eq!(stage.point(met + away * 5., None).parameters, met);
		assert_eq!(stage.point(met + away * 11., None).parameters, met);

		// Those lines and edges pass just out of reach on the way out, and none of them takes the point up
		for distance in [12.5, 14., 18.] {
			let free = met + away * distance;
			let outcome = stage.point(free, None);
			assert_eq!((outcome.parameters, outcome.snap), (free, Snap::Free), "{distance} from {met:?}");
		}
	}
}

#[test]
fn a_feature_that_cannot_move_every_way_is_left_out_without_harm() {
	let stage = Stage::new([rectangle(DVec2::ZERO, DVec2::new(100., 60.))]);

	// The second feature only ever moves level, and sits right on a corner where there is plenty to snap to
	let features = [
		FeatureMap::sliding_from(DVec2::new(300., 300.)),
		FeatureMap {
			matrix: glam::DMat2::from_diagonal(DVec2::new(1., 0.)),
			offset: DVec2::new(100., 60.),
		},
	];
	let free = DVec2::new(1., 2.);
	let outcome = stage.with_context(|context| snap_features(context, &stage.request(&[], None), &features, 0, free, None));

	assert_eq!((outcome.parameters, outcome.snap), (free, Snap::Free));
}

#[test]
fn the_nearest_anchor_wins_among_many_thousands() {
	let side = 150;
	let points: Vec<DVec2> = (0..side * side).map(|index| DVec2::new((index % side) as f64, (index / side) as f64) * 20.).collect();
	let layers = points.chunks(50).map(|chunk| path(chunk, false));
	let mut stage = Stage::new(layers);

	// Curves are left out so that only the lattice of anchors competes
	stage.settings.curves = false;
	stage.settings.midpoints = false;

	let outcome = stage.point(DVec2::new(1263., 2077.), None);

	assert_eq!(outcome.parameters, DVec2::new(1260., 2080.));
}

#[test]
fn only_the_lead_feature_is_offered_lines_from_the_point_it_joins_back_to() {
	let stage = Stage::new([hump(200., 10.)]);
	let previous = DVec2::new(-100., 0.);

	// The other feature passes over the hump, which a line from the previous point would touch
	let features = [DVec2::ZERO, DVec2::new(200., 15.)].map(FeatureMap::sliding_from);
	let outcome = stage.with_context(|context| {
		let lead_constructions = find_constructions_from(context, previous, None);
		let request = Request {
			lead_constructions: &lead_constructions,
			..stage.request(&[], None)
		};
		snap_features(context, &request, &features, 0, DVec2::ZERO, Some(previous))
	});

	let other_feature = outcome.candidates.iter().filter(|candidate| candidate.key.feature == 1);
	assert!(other_feature.clone().count() > 0);
	assert!(other_feature.clone().all(|candidate| candidate.reference != Some(previous)));
}
