//! Measures the engine against a document far larger than snapping has had to cope with, of 100,000 anchors across 5,000 layers, and against geometry crowded into one place.

use criterion::{Criterion, criterion_group, criterion_main};
use glam::DVec2;
use snapping::{
	AnchorId, Context, CrossingCache, Curve, Exclusions, Frame, GeometryRole, LayerId, LayerKind, Outcome, Request, Scene, SceneAnchor, SceneCurve, SceneLayer, Settings, Tunables, ViewMapping,
	snap_point,
};
use std::hint::black_box;

const LAYERS: u64 = 5000;
const ANCHORS_PER_LAYER: usize = 20;

/// A small closed zigzag path, placed by its index on a lattice of layers that keeps neighbors a few pixels apart.
fn layer(index: u64) -> SceneLayer {
	let origin = DVec2::new((index % 100) as f64, (index / 100) as f64) * 60.;
	let points: Vec<DVec2> = (0..ANCHORS_PER_LAYER)
		.map(|corner| {
			let angle = corner as f64 / ANCHORS_PER_LAYER as f64 * std::f64::consts::TAU;
			let radius = if corner % 2 == 0 { 25. } else { 14. };
			origin + DVec2::from_angle(angle) * radius
		})
		.collect();

	SceneLayer {
		parent: None,
		kind: LayerKind::Artwork,
		quad: Some([
			origin + DVec2::new(-25., -25.),
			origin + DVec2::new(25., -25.),
			origin + DVec2::new(25., 25.),
			origin + DVec2::new(-25., 25.),
		]),
		anchors: (0..ANCHORS_PER_LAYER)
			.map(|corner| SceneAnchor {
				position: points[corner],
				id: Some(AnchorId(corner as u64)),
				role: GeometryRole::Source,
			})
			.collect(),
		curves: (0..ANCHORS_PER_LAYER)
			.map(|corner| SceneCurve {
				curve: Curve::line(points[corner], points[(corner + 1) % ANCHORS_PER_LAYER]),
				ends: [Some(AnchorId(corner as u64)), Some(AnchorId(((corner + 1) % ANCHORS_PER_LAYER) as u64))],
				role: GeometryRole::Source,
			})
			.collect(),
	}
}

fn scene() -> Scene {
	let mut scene = Scene::new();
	for index in 0..LAYERS {
		scene.set_layer(LayerId(index), layer(index));
	}
	scene.prepare();
	scene
}

/// A layer holding loose curves and nothing else.
fn curves(curves: impl IntoIterator<Item = Curve>) -> SceneLayer {
	SceneLayer {
		parent: None,
		kind: LayerKind::Artwork,
		quad: None,
		anchors: Vec::new(),
		curves: curves
			.into_iter()
			.map(|curve| SceneCurve {
				curve,
				ends: [None, None],
				role: GeometryRole::Output,
			})
			.collect(),
	}
}

/// Thirty-two bent curves that each cross all the others within a few units of one place.
fn knot() -> Scene {
	let strand = |index: u64| {
		let angle = index as f64 / 32. * std::f64::consts::PI;
		let (along, across) = (DVec2::from_angle(angle) * 300., DVec2::from_angle(angle).perp() * (40. + index as f64));
		let middle = DVec2::new(30. + (index % 5) as f64, 9. + (index % 3) as f64);

		Curve::cubic(middle - along, middle - along / 3. + across, middle + along / 3. - across, middle + along)
	};

	let mut scene = Scene::new();
	scene.set_layer(LayerId(1), curves((0..32).map(strand)));
	scene.prepare();
	scene
}

/// Ninety-six short strokes a third of a unit apart, at slightly differing slants.
fn hatching() -> Scene {
	let stroke = |index: u64| {
		let start = DVec2::new(15. + index as f64 * 0.3, -6.);
		Curve::line(start, start + DVec2::new(4. + (index % 7) as f64, 30.))
	};

	let mut scene = Scene::new();
	scene.set_layer(LayerId(1), curves((0..96).map(stroke)));
	scene.prepare();
	scene
}

/// The usual way of looking at a scene, with nothing awake and nothing moving.
#[derive(Default)]
struct Looking {
	frame: Frame,
	tunables: Tunables,
	settings: Settings,
	exclusions: Exclusions,
}

impl Looking {
	fn at<'a>(&'a self, scene: &'a Scene) -> Context<'a> {
		Context {
			scene,
			view: &ViewMapping::IDENTITY,
			frame: &self.frame,
			tunables: &self.tunables,
			settings: &self.settings,
			exclusions: &self.exclusions,
			awake: &[],
		}
	}
}

/// A pointer sweeping back and forth around one place, where each snap carries on from the one before as it does while the pointer really moves.
fn sweep<'a>(context: Context<'a>, crossings: Option<&'a CrossingCache>, around: DVec2, previous: Option<DVec2>) -> impl FnMut() -> Outcome + 'a {
	let mut step = 0;
	let mut incumbent = None;

	move || {
		step += 1;
		let free = around + DVec2::new((step % 240) as f64 * 0.5 - 60., (step % 97) as f64 * 0.37 - 18.);

		let request = Request {
			incumbent,
			crossings,
			..Request::default()
		};
		let outcome = snap_point(&context, &request, free, previous);
		incumbent = outcome.incumbent();
		black_box(outcome)
	}
}

fn scale(criterion: &mut Criterion) {
	criterion.bench_function("build the whole scene", |bencher| bencher.iter(|| black_box(scene())));

	let mut changing = scene();
	criterion.bench_function("replace one layer and prepare", |bencher| {
		bencher.iter(|| {
			changing.set_layer(LayerId(2500), layer(2500));
			changing.prepare();
		})
	});

	let looking = Looking::default();

	// Across several layers of the large scene, which covers empty space, edges, and corners alike
	let scene = scene();
	let mut over_the_scene = sweep(looking.at(&scene), None, DVec2::new(2960., 2949.), Some(DVec2::new(2000., 2000.)));
	criterion.bench_function("snap a point as the pointer moves", |bencher| bencher.iter(&mut over_the_scene));

	// Through the middle of a knot of curves that all cross one another there
	let knot = knot();
	let middle = DVec2::new(30., 9.);
	let crossings = CrossingCache::default();
	let mut through_the_knot = sweep(looking.at(&knot), Some(&crossings), middle, None);
	criterion.bench_function("snap among crossing curves", |bencher| bencher.iter(&mut through_the_knot));
	let mut finding_every_crossing_again = sweep(looking.at(&knot), None, middle, None);
	criterion.bench_function("snap among crossing curves without remembering crossings", |bencher| bencher.iter(&mut finding_every_crossing_again));

	// Over closely spaced strokes while drawing a line from elsewhere, which draws a line to every one of them
	let hatching = hatching();
	let mut over_the_hatching = sweep(looking.at(&hatching), None, DVec2::new(30., 9.), Some(DVec2::new(-400., 300.)));
	criterion.bench_function("snap over hatching while drawing a line", |bencher| bencher.iter(&mut over_the_hatching));
}

criterion_group!(benches, scale);
criterion_main!(benches);
