use crate::candidate::{Candidate, CandidateKey, Kind, Shape};
use crate::grid::Grid;
use crate::locus::Locus;
use crate::tunables::Tunables;
use crate::view::ViewMapping;
use glam::{DMat2, DVec2};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;

/// Crossings are looked for among this many of the nearest pieces of paths and boxes, and as many of the nearest lines drawn to touch or squarely meet them.
/// That bounds the pairs examined where geometry is dense.
const MAX_CROSSING_CURVES: usize = 16;

/// Crossings are looked for among this many of the nearest of all other lines and circles, which come from the few targets that are awake.
const MAX_CROSSING_LINES: usize = 24;

/// How a feature's position follows the two parameters a gesture is solved in: `position = matrix * parameters + offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureMap {
	pub matrix: DMat2,
	pub offset: DVec2,
}

impl FeatureMap {
	/// A feature positioned at the parameters themselves, as when placing a single point.
	pub const IDENTITY: Self = Self {
		matrix: DMat2::IDENTITY,
		offset: DVec2::ZERO,
	};

	/// A feature carried along by a slide, where the parameters are how far everything has moved from where it began.
	pub fn sliding_from(start: DVec2) -> Self {
		Self {
			matrix: DMat2::IDENTITY,
			offset: start,
		}
	}

	pub fn position(&self, parameters: DVec2) -> DVec2 {
		self.matrix * parameters + self.offset
	}

	/// The parameters that put the feature at a position. Returns `None` if there is a direction the feature cannot move in.
	pub fn parameters_for(&self, position: DVec2) -> Option<DVec2> {
		// The area the columns span is negligible beside their lengths when they are parallel to within rounding
		let spans_the_plane = self.matrix.determinant().abs() > 1e-12 * self.matrix.x_axis.length() * self.matrix.y_axis.length();
		spans_the_plane.then(|| self.matrix.inverse() * (position - self.offset))
	}
}

/// How completely a snap fixes a feature, from best to worst.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tier {
	/// A spot, or a crossing of two tracks.
	Spot,
	/// A single track.
	Track,
}

/// Identifies what is engaged from one moment to the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EngagementKey {
	Single(CandidateKey),
	Crossing(CandidateKey, CandidateKey),
}

impl EngagementKey {
	fn crossing(a: CandidateKey, b: CandidateKey) -> Self {
		Self::Crossing(a.min(b), a.max(b))
	}
}

/// The snap that was engaged a moment ago, which is favored so that snaps hold on rather than flicker.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Incumbent {
	pub key: EngagementKey,
	/// The parameters it gave, which tell apart the several places one pair of tracks can cross.
	pub parameters: DVec2,
	/// Whether it was chosen deliberately, which keeps it engaged against any rival for as long as it holds.
	pub pinned: bool,
	/// The track the point was riding.
	pub riding: Option<CandidateKey>,
}

/// Remembers where pairs of curves cross, which is the costly part of a solve.
///
/// What it holds stays true only while the scene, the features, and the settings do, so it must be cleared when any of them changes.
#[derive(Debug, Default)]
pub struct CrossingCache {
	crossings: RefCell<HashMap<(CandidateKey, CandidateKey), Vec<DVec2>>>,
}

impl CrossingCache {
	pub fn clear(&mut self) {
		self.crossings.get_mut().clear();
	}

	/// Where two tracks cross, looked up for a pair of curves that has been asked about before.
	fn crossings(cache: Option<&Self>, first: &NearbyTrack, second: &NearbyTrack) -> Vec<DVec2> {
		// Finding them in the order of the keys gives one answer whichever track happens to be nearer
		let [first, second] = if first.key <= second.key { [first, second] } else { [second, first] };
		let find = || first.locus.crossings(&second.locus);

		let both_curves = matches!((first.locus, second.locus), (Locus::Curve(_), Locus::Curve(_)));
		let Some(mut remembered) = cache.filter(|_| both_curves).and_then(|cache| cache.crossings.try_borrow_mut().ok()) else {
			return find();
		};
		remembered.entry((first.key, second.key)).or_insert_with(find).clone()
	}
}

/// One moment of a gesture, in the two parameters it is solved in.
#[derive(Clone, Copy, Debug)]
pub struct Problem<'a> {
	pub view: &'a ViewMapping,
	pub tunables: &'a Tunables,
	/// The moving features, which candidates refer to by index. An engaged snap is recognized by that index, so the order must last the gesture.
	pub features: &'a [FeatureMap],
	/// The index of the feature the pointer carries, which is the one that a required track is judged for and that lands on the grid.
	pub lead: u32,
	/// The parameters the pointer asks for.
	pub free: DVec2,
	/// Every way a feature could snap, in the space of the parameters. No two may share a key.
	pub candidates: &'a [Candidate],
	/// A track the parameters must stay on.
	pub required: Option<Locus>,
	pub grid: Option<&'a Grid>,
	pub incumbent: Option<Incumbent>,
	/// How many candidates to step on from the one that would otherwise be chosen, wrapping around. The one reached comes back pinned.
	pub cycle: usize,
	pub crossings: Option<&'a CrossingCache>,
	/// Parameters that no snap may bring the gesture to, as where a segment is drawn from is no place for its other end.
	pub avoid: Option<DVec2>,
}

/// What the parameters were snapped by.
#[derive(Clone, Debug, PartialEq)]
pub enum Snap {
	Free,
	Grid,
	Engaged(Engagement),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Engagement {
	pub key: EngagementKey,
	pub tier: Tier,
	/// The indices of the engaged candidate, or of both tracks of a crossing.
	pub candidates: Vec<usize>,
	/// How far on screen, in pixels, the candidate lies from where the pointer asks for, before any move along it to a grid crossing.
	pub distance: f64,
	/// Whether the position along a straight track was moved to a grid crossing.
	pub quantized: bool,
	/// Whether it was chosen deliberately, by cycling to it now or at an earlier moment.
	pub pinned: bool,
	/// The track the point is riding: the engaged track itself, or the one it was on when a spot or a crossing took over.
	/// It lasts until another track is engaged or the pointer strays beyond the release distance from it.
	pub riding: Option<CandidateKey>,
	/// How many candidates were within reach, which is how many the cycle steps through.
	pub alternatives: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Solution {
	pub parameters: DVec2,
	pub snap: Snap,
}

/// One place the parameters could snap to.
#[derive(Clone, Copy, Debug)]
struct Resolved {
	tier: Tier,
	parameters: DVec2,
	/// How far on screen it lies from where the pointer asks for, which ranks it.
	distance: f64,
	/// The distance that decides whether it is within reach, which is shorter than `distance` for something further along a track the point is riding.
	reach_distance: f64,
	kind: Kind,
	key: EngagementKey,
	candidates: [usize; 2],
	candidate_count: usize,
	/// Whether it is the track the point is riding, with the pointer still within the release distance of it.
	ridden: bool,
}

impl Resolved {
	fn single(tier: Tier, index: usize, key: CandidateKey, parameters: DVec2, distance: f64) -> Self {
		Self {
			tier,
			parameters,
			distance,
			reach_distance: distance,
			kind: key.kind,
			key: EngagementKey::Single(key),
			candidates: [index, index],
			candidate_count: 1,
			ridden: false,
		}
	}

	/// A strict order for listing, by tier and then distance, with the rest only settling exact ties the same way every time.
	fn listing_order(&self, other: &Self) -> Ordering {
		self.tier
			.cmp(&other.tier)
			.then(self.distance.total_cmp(&other.distance))
			.then(self.kind.cmp(&other.kind))
			.then(self.key.cmp(&other.key))
			.then(self.parameters.x.total_cmp(&other.parameters.x))
			.then(self.parameters.y.total_cmp(&other.parameters.y))
	}
}

/// A problem, together with how a small change of the parameters appears to move each feature on screen.
struct Measured<'a> {
	problem: &'a Problem<'a>,
	seen: Vec<DMat2>,
}

impl<'a> Measured<'a> {
	/// Judges how movement appears around one setting of the parameters, which varies from place to place in a view with perspective.
	fn around(problem: &'a Problem<'a>, parameters: DVec2) -> Self {
		let as_seen = |feature: &FeatureMap| problem.view.jacobian_at(feature.position(parameters)) * feature.matrix;
		let seen = problem.features.iter().map(as_seen).collect();

		Self { problem, seen }
	}

	fn feature(&self, feature: u32) -> FeatureMap {
		self.problem.features.get(feature as usize).copied().unwrap_or(FeatureMap::IDENTITY)
	}

	/// The further that either of two features travels on screen between two settings of the parameters.
	fn distance(&self, features: [u32; 2], from: DVec2, to: DVec2) -> f64 {
		let travel = |feature: u32| {
			let map = self.feature(feature);
			self.problem.view.screen_distance(map.position(from), map.position(to))
		};

		if features[0] == features[1] {
			travel(features[0])
		} else {
			travel(features[0]).max(travel(features[1]))
		}
	}

	/// Whether a setting of the parameters puts the lead feature at the place it is told to avoid.
	fn avoided(&self, parameters: DVec2) -> bool {
		let problem = self.problem;
		problem.avoid.is_some_and(|avoid| self.distance([problem.lead; 2], avoid, parameters) <= problem.tunables.on_track)
	}

	/// The position on a locus that appears nearest to a setting of the parameters, for the feature the locus applies to.
	fn nearest(&self, feature: u32, locus: &Locus, parameters: DVec2) -> DVec2 {
		let seen = self.seen.get(feature as usize).copied().unwrap_or(DMat2::IDENTITY);
		locus.nearest_as_seen(parameters, seen)
	}

	fn lies_on(&self, feature: u32, locus: &Locus, parameters: DVec2) -> bool {
		self.distance([feature; 2], parameters, self.nearest(feature, locus, parameters)) <= self.problem.tunables.on_track
	}
}

/// Where the pointer falls on a required track, which is the position on it that appears nearest for the lead feature.
pub(crate) fn confine(view: &ViewMapping, lead: &FeatureMap, required: Option<&Locus>, free: DVec2) -> DVec2 {
	let Some(required) = required else { return free };
	required.nearest_as_seen(free, view.jacobian_at(lead.position(free)) * lead.matrix)
}

/// Decides what, if anything, the parameters snap to.
pub fn solve(problem: &Problem) -> Solution {
	let Tunables { reach, switch_margin, tie, .. } = *problem.tunables;

	// Under a required track everything is measured from where the pointer falls on it
	let lead = problem.features.get(problem.lead as usize).copied().unwrap_or(FeatureMap::IDENTITY);
	let base = confine(problem.view, &lead, problem.required.as_ref(), problem.free);
	if !base.is_finite() {
		return Solution {
			parameters: problem.free,
			snap: Snap::Free,
		};
	}

	let measured = Measured::around(problem, base);
	let mut resolved = match problem.required {
		Some(required) => resolve_along(&measured, required, base),
		None => resolve_freely(&measured),
	};
	// A track goes too where the place on it nearest the pointer is the one to avoid, as at the end of a curve
	resolved.retain(|candidate| !measured.avoided(candidate.parameters));

	let mut within_reach: Vec<&Resolved> = resolved.iter().filter(|candidate| candidate.reach_distance <= reach).collect();
	within_reach.sort_by(|a, b| a.listing_order(b));
	let challenger = preferred(&within_reach, tie);

	// Unless pinned, the engaged snap is unseated by a better tier or by anything of its own tier that is nearer by a clear margin
	let held = hold(problem, &resolved);
	let natural = match (held, challenger) {
		(Some((held, pinned)), Some(challenger)) => {
			let unseats = |rival: &&Resolved| rival.tier < held.tier || (rival.tier == held.tier && rival.distance + switch_margin < held.distance);
			if !pinned && within_reach.iter().any(unseats) { Some(challenger) } else { Some(held) }
		}
		(Some((held, _)), None) => Some(held),
		(None, challenger) => challenger,
	};

	// Cycling steps on through the list from whatever would otherwise be shown
	let cycled = (problem.cycle > 0 && !within_reach.is_empty()).then(|| {
		let count = within_reach.len();
		let shown = natural.and_then(|natural| within_reach.iter().position(|&listed| std::ptr::eq(listed, natural)));
		let index = shown.unwrap_or(count - 1) + problem.cycle % count;
		within_reach[index % count]
	});
	let pinned = cycled.is_some() || held.is_some_and(|(_, pinned)| pinned);

	let Some(chosen) = cycled.or(natural) else {
		// With nothing engaged, the grid decides where the feature lands along whatever freedom remains
		let landed = match problem.required {
			Some(required) => land_on_grid_along(&measured, &required, base),
			None => land_on_grid(problem, base).filter(|&landed| !measured.avoided(landed)),
		};
		return match landed {
			Some(parameters) => Solution { parameters, snap: Snap::Grid },
			None => Solution { parameters: base, snap: Snap::Free },
		};
	};

	// A single track still leaves one freedom, which the grid takes up if the track is straight
	let landed = match problem.candidates.get(chosen.candidates[0]).map(|candidate| candidate.shape) {
		Some(Shape::Track(track)) if chosen.tier == Tier::Track => land_on_grid_along(&measured, &track, chosen.parameters),
		_ => None,
	};

	// An engaged track is the one ridden from now on, while a spot or a crossing leaves the point riding whatever it already was
	let ridden = if chosen.tier == Tier::Track {
		Some(chosen)
	} else {
		resolved.iter().find(|candidate| candidate.ridden)
	};
	let riding = ridden.and_then(|ridden| match ridden.key {
		EngagementKey::Single(key) => Some(key),
		EngagementKey::Crossing(..) => None,
	});

	Solution {
		parameters: landed.unwrap_or(chosen.parameters),
		snap: Snap::Engaged(Engagement {
			key: chosen.key,
			tier: chosen.tier,
			candidates: chosen.candidates[..chosen.candidate_count].to_vec(),
			distance: chosen.distance,
			quantized: landed.is_some(),
			pinned,
			riding,
			alternatives: within_reach.len(),
		}),
	}
}

/// The pick from a list in listing order: the nearest of the best tier, unless a more specific kind lies within the tie distance of it.
fn preferred<'r>(listed: &[&'r Resolved], tie: f64) -> Option<&'r Resolved> {
	let nearest = *listed.first()?;
	let tied = listed.iter().take_while(|candidate| candidate.tier == nearest.tier && candidate.distance <= nearest.distance + tie);

	tied.min_by(|a, b| a.kind.cmp(&b.kind).then(a.listing_order(b))).copied()
}

/// What carries on from the engaged snap now that the pointer has moved, and whether it stays pinned.
fn hold<'r>(problem: &Problem, resolved: &'r [Resolved]) -> Option<(&'r Resolved, bool)> {
	let incumbent = problem.incumbent?;
	let holding = |candidate: &&Resolved| candidate.reach_distance <= problem.tunables.release;

	// One pair of tracks can cross in several places, of which the one it was at carries on
	let moved = |candidate: &&Resolved| candidate.parameters.distance_squared(incumbent.parameters);
	let same = resolved.iter().filter(|candidate| candidate.key == incumbent.key);
	if let Some(same) = same.min_by(|a, b| moved(a).total_cmp(&moved(b))).filter(holding) {
		return Some((same, incumbent.pinned));
	}

	// Having left a spot or a crossing behind, the point carries on along the track it was riding
	let ridden = resolved.iter().find(|candidate| candidate.ridden);
	ridden.map(|ridden| (ridden, false))
}

/// A track close enough to the pointer to take part in crossings.
struct NearbyTrack {
	index: usize,
	key: CandidateKey,
	locus: Locus,
	nearest: DVec2,
	distance: f64,
	ridden: bool,
}

impl NearbyTrack {
	/// How far something on this track is from the point's place on it, which is what decides reach while the point is riding the track.
	fn distance_along(&self, measured: &Measured, other_feature: u32, position: DVec2) -> f64 {
		measured.distance([self.key.feature, other_feature], self.nearest, position)
	}
}

/// Every spot, every track's nearest position, and every crossing of two nearby tracks, each measured from where the pointer asks for.
fn resolve_freely(measured: &Measured) -> Vec<Resolved> {
	let problem = measured.problem;
	let (free, release) = (problem.free, problem.tunables.release);
	let riding = problem.incumbent.and_then(|incumbent| incumbent.riding);

	let mut resolved = Vec::new();
	let mut nearby_tracks = Vec::new();

	for (index, candidate) in problem.candidates.iter().enumerate() {
		let Shape::Track(locus) = candidate.shape else { continue };
		let key = candidate.key;

		let nearest = measured.nearest(key.feature, &locus, free);
		let distance = measured.distance([key.feature; 2], free, nearest);
		if !distance.is_finite() {
			continue;
		}

		let ridden = distance <= release && riding == Some(key);

		resolved.push(Resolved {
			ridden,
			..Resolved::single(Tier::Track, index, key, nearest, distance)
		});
		if distance <= problem.tunables.gathering_distance() {
			nearby_tracks.push(NearbyTrack {
				index,
				key,
				locus,
				nearest,
				distance,
				ridden,
			});
		}
	}

	// A track beyond the release distance can only matter where it crosses the one being ridden
	let is_riding = nearby_tracks.iter().any(|track| track.ridden);
	nearby_tracks.retain(|track| is_riding || track.distance <= release);

	for (index, candidate) in problem.candidates.iter().enumerate() {
		let Shape::Spot(position) = candidate.shape else { continue };
		let key = candidate.key;

		let distance = measured.distance([key.feature; 2], free, position);
		if !distance.is_finite() {
			continue;
		}

		let ridden_through = nearby_tracks.iter().filter(|track| track.ridden && measured.lies_on(track.key.feature, &track.locus, position));
		let reach_distance = ridden_through.map(|track| track.distance_along(measured, key.feature, position)).fold(distance, f64::min);

		resolved.push(Resolved {
			reach_distance,
			..Resolved::single(Tier::Spot, index, key, position, distance)
		});
	}

	// Only the nearest of each sort of track, besides those held, are tried against each other, so a crowd of one sort can't push out another
	let held = match problem.incumbent.map(|incumbent| incumbent.key) {
		Some(EngagementKey::Crossing(first, second)) => [Some(first), Some(second)],
		_ => [None; 2],
	};
	nearby_tracks.sort_by(|a, b| a.distance.total_cmp(&b.distance).then(a.key.cmp(&b.key)));
	let (mut curves, mut drawn_to_curves, mut others) = (0, 0, 0);
	nearby_tracks.retain(|track| {
		let (count, limit) = match (track.locus, track.key.kind) {
			(Locus::Curve(_), _) => (&mut curves, MAX_CROSSING_CURVES),
			(_, Kind::TangentLine | Kind::PerpendicularLine) => (&mut drawn_to_curves, MAX_CROSSING_CURVES),
			_ => (&mut others, MAX_CROSSING_LINES),
		};
		*count += 1;
		track.ridden || held.contains(&Some(track.key)) || *count <= limit
	});

	// O(n^2) in the nearby tracks, which the limits above bound
	for (position, first) in nearby_tracks.iter().enumerate() {
		for second in &nearby_tracks[position + 1..] {
			let within_release = first.distance <= release && second.distance <= release;
			if !within_release && !first.ridden && !second.ridden {
				continue;
			}

			// Lines drawn from one point meet only there, which is no place to snap to
			if let (Locus::Line { point, .. }, Locus::Line { point: other_point, .. }) = (first.locus, second.locus)
				&& point == other_point
			{
				continue;
			}

			let features = [first.key.feature, second.key.feature];

			for crossing in CrossingCache::crossings(problem.crossings, first, second) {
				let distance = measured.distance(features, free, crossing);
				if !distance.is_finite() || joins_end_to_end(&first.locus, &second.locus, crossing) {
					continue;
				}

				let ridden_through = [(first, second), (second, first)].into_iter().filter(|(track, _)| track.ridden);
				let reach_distance = ridden_through
					.map(|(track, other)| track.distance_along(measured, other.key.feature, crossing))
					.fold(distance, f64::min);

				resolved.push(Resolved {
					tier: Tier::Spot,
					parameters: crossing,
					distance,
					reach_distance,
					kind: first.key.kind.min(second.key.kind),
					key: EngagementKey::crossing(first.key, second.key),
					candidates: [first.index, second.index],
					candidate_count: 2,
					ridden: false,
				});
			}
		}
	}

	resolved
}

/// Whether a crossing is only where two curves join end to end, which is an anchor rather than a place they cross.
fn joins_end_to_end(first: &Locus, second: &Locus, crossing: DVec2) -> bool {
	let ends_there = |locus: &Locus| {
		let Locus::Curve(curve) = locus else { return false };

		// An end is judged against the size of its curve, with allowance for the rounding of coordinates far from the origin
		let tolerance = 1e-9 * curve.bounds().size().length() + 8. * f64::EPSILON * crossing.abs().max_element();
		curve.start().distance(crossing) <= tolerance || curve.end().distance(crossing) <= tolerance
	};

	ends_there(first) && ends_there(second)
}

/// Every spot lying on the required track and every place a track crosses it, each measured from where the pointer falls on the required track.
fn resolve_along(measured: &Measured, required: Locus, base: DVec2) -> Vec<Resolved> {
	let problem = measured.problem;
	let mut resolved = Vec::new();

	for (index, candidate) in problem.candidates.iter().enumerate() {
		let key = candidate.key;
		let features = [key.feature, problem.lead];

		let positions = match candidate.shape {
			Shape::Spot(position) => {
				let on_required = measured.nearest(key.feature, &required, position);
				let lies_on_it = measured.distance(features, position, on_required) <= problem.tunables.on_track;
				if lies_on_it { vec![on_required] } else { Vec::new() }
			}
			Shape::Track(locus) => required.crossings(&locus),
		};

		for parameters in positions {
			let distance = measured.distance(features, base, parameters);
			if distance.is_finite() {
				resolved.push(Resolved::single(Tier::Spot, index, key, parameters, distance));
			}
		}
	}

	resolved
}

/// The point and direction of a locus that runs straight, in the space of the parameters.
fn straight_line(locus: &Locus) -> Option<(DVec2, DVec2)> {
	match *locus {
		Locus::Line { point, direction } => Some((point, direction)),
		Locus::Curve(curve) if curve.is_straight() => Some((curve.start(), (curve.end() - curve.start()).try_normalize()?)),
		_ => None,
	}
}

/// The parameters that put the lead feature on the grid point appearing nearest to where `parameters` would put it.
fn land_on_grid(problem: &Problem, parameters: DVec2) -> Option<DVec2> {
	let map = problem.features.get(problem.lead as usize)?;
	let position = map.position(parameters);
	if !problem.view.to_screen(position).is_finite() {
		return None;
	}

	let landed = map.parameters_for(problem.grid?.nearest_point_as_seen(position, problem.view.jacobian_at(position)))?;
	landed.is_finite().then_some(landed)
}

/// The parameters on a straight track that put the lead feature on a grid line, at the crossing on the track that appears nearest to `near`.
/// Returns `None` for a track that bends, or one too short to reach a grid line on either side.
fn land_on_grid_along(measured: &Measured, track: &Locus, near: DVec2) -> Option<DVec2> {
	let problem = measured.problem;
	let lead = [problem.lead; 2];
	let map = problem.features.get(problem.lead as usize)?;
	let (point, direction) = straight_line(track)?;

	let crossings = problem.grid?.crossings_along(map.position(point), map.matrix * direction, map.position(near));
	let landings = crossings.into_iter().filter_map(|crossing| map.parameters_for(crossing));

	let on_the_track = |landed: &DVec2| landed.is_finite() && measured.distance(lead, track.nearest(*landed), *landed) <= problem.tunables.on_track;
	let allowed = |landed: &DVec2| on_the_track(landed) && !measured.avoided(*landed);
	let from_near = |landed: &DVec2| measured.distance(lead, near, *landed);
	landings.filter(allowed).min_by(|a, b| from_near(a).total_cmp(&from_near(b)))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::curve::Curve;
	use glam::DAffine2;

	const POINT: [FeatureMap; 1] = [FeatureMap::IDENTITY];

	fn key(kind: Kind, identity: u64) -> CandidateKey {
		CandidateKey {
			kind,
			feature: 0,
			identity: [identity, 0, 0],
		}
	}

	fn spot(kind: Kind, identity: u64, position: DVec2) -> Candidate {
		Candidate::spot(key(kind, identity), position)
	}

	fn track(kind: Kind, identity: u64, locus: Locus) -> Candidate {
		Candidate::track(key(kind, identity), locus)
	}

	fn horizontal(y: f64) -> Locus {
		Locus::line(DVec2::new(0., y), DVec2::X).unwrap()
	}

	fn vertical(x: f64) -> Locus {
		Locus::line(DVec2::new(x, 0.), DVec2::Y).unwrap()
	}

	fn problem<'a>(view: &'a ViewMapping, tunables: &'a Tunables, candidates: &'a [Candidate], free: DVec2) -> Problem<'a> {
		Problem {
			view,
			tunables,
			features: &POINT,
			lead: 0,
			free,
			candidates,
			required: None,
			grid: None,
			incumbent: None,
			cycle: 0,
			crossings: None,
			avoid: None,
		}
	}

	fn engaged(solution: &Solution) -> &Engagement {
		match &solution.snap {
			Snap::Engaged(engagement) => engagement,
			other => panic!("expected an engaged snap but found {other:?}"),
		}
	}

	/// What to pass as the incumbent a moment after a solution, if it engaged.
	fn incumbent_after(solution: &Solution) -> Option<Incumbent> {
		match &solution.snap {
			Snap::Engaged(engagement) => Some(Incumbent {
				key: engagement.key,
				parameters: solution.parameters,
				pinned: engagement.pinned,
				riding: engagement.riding,
			}),
			_ => None,
		}
	}

	/// An incumbent engaged on a candidate, which is the track being ridden if it is a track.
	fn engaged_on(candidate: &Candidate, parameters: DVec2) -> Option<Incumbent> {
		Some(Incumbent {
			key: EngagementKey::Single(candidate.key),
			parameters,
			pinned: false,
			riding: matches!(candidate.shape, Shape::Track(_)).then_some(candidate.key),
		})
	}

	#[test]
	fn nothing_within_reach_leaves_the_pointer_free() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [spot(Kind::Anchor, 1, DVec2::new(20., 0.)), track(Kind::OnCurve, 2, horizontal(9.))];

		let solution = solve(&problem(&view, &tunables, &candidates, DVec2::ZERO));

		assert_eq!(solution.parameters, DVec2::ZERO);
		assert_eq!(solution.snap, Snap::Free);
	}

	#[test]
	fn nothing_fixes_the_point_at_the_place_it_is_told_to_avoid() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let avoid = Some(DVec2::new(2., 0.));

		// An anchor sits there and two tracks cross there, and all that is left is to ride the nearer track
		let candidates = [
			spot(Kind::Anchor, 1, DVec2::new(2., 0.)),
			track(Kind::OnCurve, 2, horizontal(0.)),
			track(Kind::Alignment, 3, vertical(2.)),
		];
		let free = DVec2::new(1., 3.);
		assert_eq!(solve(&problem(&view, &tunables, &candidates, free)).parameters, DVec2::new(2., 0.));

		let solution = solve(&Problem {
			avoid,
			..problem(&view, &tunables, &candidates, free)
		});
		assert_eq!(solution.parameters, DVec2::new(2., 3.));
		assert_eq!(engaged(&solution).tier, Tier::Track);

		// A curve that ends there does not hold a point that has run past its end
		let candidates = [track(Kind::OnCurve, 1, Locus::Curve(Curve::line(DVec2::new(-50., 0.), DVec2::new(2., 0.))))];
		let past_the_end = solve(&Problem {
			avoid,
			..problem(&view, &tunables, &candidates, DVec2::new(5., 1.))
		});
		assert_eq!(past_the_end.snap, Snap::Free);

		// A spot elsewhere is as good as ever
		let candidates = [spot(Kind::Anchor, 1, DVec2::new(4., 0.))];
		let solution = solve(&Problem {
			avoid,
			..problem(&view, &tunables, &candidates, free)
		});
		assert_eq!(solution.parameters, DVec2::new(4., 0.));
	}

	#[test]
	fn the_grid_lands_the_point_anywhere_but_the_place_it_is_told_to_avoid() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let grid = Grid::rectangular(DVec2::ZERO, DVec2::splat(100.)).unwrap();
		let avoid = Some(DVec2::new(100., 300.));

		// The nearest grid point is the place to avoid, so the point stays with the pointer
		let free = DVec2::new(130., 260.);
		let solution = solve(&Problem {
			grid: Some(&grid),
			avoid,
			..problem(&view, &tunables, &[], free)
		});
		assert_eq!((solution.parameters, solution.snap), (free, Snap::Free));

		// Along a straight track through that place, the grid lands the point on the next line along instead
		let candidates = [track(Kind::Alignment, 1, vertical(100.))];
		let solution = solve(&Problem {
			grid: Some(&grid),
			avoid,
			..problem(&view, &tunables, &candidates, DVec2::new(103., 260.))
		});
		assert!(solution.parameters.abs_diff_eq(DVec2::new(100., 200.), 1e-9));
	}

	#[test]
	fn a_spot_within_reach_beats_a_nearer_track() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [track(Kind::OnCurve, 1, horizontal(1.)), spot(Kind::Anchor, 2, DVec2::new(7., 0.))];

		let solution = solve(&problem(&view, &tunables, &candidates, DVec2::ZERO));

		assert_eq!(solution.parameters, DVec2::new(7., 0.));
		assert_eq!(engaged(&solution).tier, Tier::Spot);
		assert_eq!(engaged(&solution).candidates, vec![1]);
		assert!(!engaged(&solution).pinned);
	}

	#[test]
	fn the_nearest_candidate_of_a_tier_wins_and_kind_settles_a_near_tie() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let winner = |corner_distance: f64, anchor_distance: f64| {
			let candidates = [spot(Kind::BoxCorner, 1, DVec2::new(0., corner_distance)), spot(Kind::Anchor, 2, DVec2::new(anchor_distance, 0.))];
			engaged(&solve(&problem(&view, &tunables, &candidates, DVec2::ZERO))).candidates[0]
		};

		assert_eq!(winner(3., 5.), 0);

		// Within half a pixel of the nearest, the more specific kind is chosen though it is slightly further
		assert_eq!(winner(3.1, 3.3), 1);
		assert_eq!(winner(2.9, 3.1), 1);
		assert_eq!(winner(3., 3.4), 1);
		assert_eq!(winner(3., 3.6), 0);
	}

	#[test]
	fn a_track_within_reach_is_slid_along() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [track(Kind::OnCurve, 1, horizontal(5.))];

		let solution = solve(&problem(&view, &tunables, &candidates, DVec2::new(40., 0.)));

		assert_eq!(solution.parameters, DVec2::new(40., 5.));
		assert_eq!(engaged(&solution).tier, Tier::Track);
	}

	#[test]
	fn two_tracks_crossing_within_reach_fix_the_point_at_their_crossing() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [track(Kind::Alignment, 1, horizontal(4.)), track(Kind::Alignment, 2, vertical(-3.))];

		let solution = solve(&problem(&view, &tunables, &candidates, DVec2::ZERO));

		assert!(solution.parameters.abs_diff_eq(DVec2::new(-3., 4.), 1e-9));
		assert_eq!(engaged(&solution).tier, Tier::Spot);

		// The nearer track is listed first
		assert_eq!(engaged(&solution).candidates, vec![1, 0]);
	}

	#[test]
	fn a_crossing_beyond_reach_leaves_the_nearer_track_engaged() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [track(Kind::Alignment, 1, horizontal(6.)), track(Kind::Alignment, 2, vertical(7.))];

		let solution = solve(&problem(&view, &tunables, &candidates, DVec2::ZERO));

		assert_eq!(solution.parameters, DVec2::new(0., 6.));
		assert_eq!(engaged(&solution).candidates, vec![0]);
	}

	#[test]
	fn curves_far_from_the_origin_still_cross_at_their_middles() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let far = DVec2::splat(1e9);
		let segment = |from: DVec2, to: DVec2| Locus::Curve(Curve::line(far + from, far + to));
		let candidates = [
			track(Kind::OnCurve, 1, segment(DVec2::new(-1., 0.), DVec2::new(1., 0.))),
			track(Kind::OnCurve, 2, segment(DVec2::new(0., -1.), DVec2::new(0., 1.))),
		];

		let solution = solve(&problem(&view, &tunables, &candidates, far + DVec2::new(0.25, 0.25)));

		// The ends are a whole unit from the crossing, which is far more than coordinates this large are rounded by
		assert!(solution.parameters.abs_diff_eq(far, 1e-6));
		assert_eq!(engaged(&solution).tier, Tier::Spot);
	}

	#[test]
	fn a_crossing_is_as_far_as_the_feature_that_must_travel_furthest_to_it() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());

		// The second feature moves three times as far as the first for the same change of the parameters
		let features = [
			FeatureMap::IDENTITY,
			FeatureMap {
				matrix: DMat2::from_diagonal(DVec2::splat(3.)),
				offset: DVec2::ZERO,
			},
		];
		let for_feature = |feature: u32, locus: Locus| {
			let key = CandidateKey {
				kind: Kind::Alignment,
				feature,
				identity: [feature as u64, 0, 0],
			};
			Candidate::track(key, locus)
		};
		let candidates = [for_feature(0, horizontal(4.)), for_feature(1, vertical(-2.))];

		let solution = solve(&Problem {
			features: &features,
			..problem(&view, &tunables, &candidates, DVec2::ZERO)
		});

		// Reaching the crossing would move the first feature under 5 pixels, yet the second over 13, which is out of reach
		assert_eq!(engaged(&solution).tier, Tier::Track);
		assert_eq!(engaged(&solution).candidates, vec![0]);
	}

	#[test]
	fn an_engaged_spot_holds_until_the_release_distance() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [spot(Kind::Anchor, 1, DVec2::ZERO)];
		let incumbent = engaged_on(&candidates[0], DVec2::ZERO);

		// Beyond reach but short of the release distance, only an already engaged snap stays
		let beyond_reach = DVec2::new(10., 0.);
		assert_eq!(solve(&problem(&view, &tunables, &candidates, beyond_reach)).snap, Snap::Free);
		assert_eq!(
			solve(&Problem {
				incumbent,
				..problem(&view, &tunables, &candidates, beyond_reach)
			})
			.parameters,
			DVec2::ZERO
		);

		let beyond_release = DVec2::new(13., 0.);
		assert_eq!(
			solve(&Problem {
				incumbent,
				..problem(&view, &tunables, &candidates, beyond_release)
			})
			.snap,
			Snap::Free
		);
	}

	#[test]
	fn a_rival_of_the_same_tier_must_be_clearly_nearer_to_take_over() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [spot(Kind::Anchor, 1, DVec2::new(-5., 0.)), spot(Kind::Anchor, 2, DVec2::new(5., 0.))];
		let incumbent = engaged_on(&candidates[0], DVec2::new(-5., 0.));

		// The rival is nearer by one pixel, which is within the switch margin
		let slightly_past_the_middle = DVec2::new(0.5, 0.);
		assert_eq!(
			solve(&Problem {
				incumbent,
				..problem(&view, &tunables, &candidates, slightly_past_the_middle)
			})
			.parameters,
			DVec2::new(-5., 0.)
		);

		let well_past_the_middle = DVec2::new(1.5, 0.);
		assert_eq!(
			solve(&Problem {
				incumbent,
				..problem(&view, &tunables, &candidates, well_past_the_middle)
			})
			.parameters,
			DVec2::new(5., 0.)
		);
	}

	#[test]
	fn any_clearly_nearer_rival_unseats_the_engaged_snap_in_favor_of_the_usual_pick() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [
			spot(Kind::Anchor, 1, DVec2::new(-5.2, 0.)),
			spot(Kind::BoxCorner, 2, DVec2::new(0., 3.)),
			spot(Kind::Anchor, 3, DVec2::new(3.4, 0.)),
		];
		let incumbent = engaged_on(&candidates[0], DVec2::new(-5.2, 0.));

		let solution = solve(&Problem {
			incumbent,
			..problem(&view, &tunables, &candidates, DVec2::ZERO)
		});

		// Only the corner is nearer by the margin, and with the engaged snap unseated the anchor tying with that corner is the pick
		assert_eq!(engaged(&solution).candidates, vec![2]);
	}

	#[test]
	fn a_better_tier_takes_over_from_an_engaged_track_at_once() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [track(Kind::OnCurve, 1, horizontal(0.)), spot(Kind::Anchor, 2, DVec2::new(7., 3.))];
		let incumbent = engaged_on(&candidates[0], DVec2::ZERO);

		let solution = solve(&Problem {
			incumbent,
			..problem(&view, &tunables, &candidates, DVec2::ZERO)
		});

		assert_eq!(solution.parameters, DVec2::new(7., 3.));
	}

	#[test]
	fn a_pinned_snap_holds_against_any_rival_until_released() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [track(Kind::OnCurve, 1, horizontal(0.)), spot(Kind::Anchor, 2, DVec2::new(1., 3.))];
		let incumbent = engaged_on(&candidates[0], DVec2::ZERO).map(|incumbent| Incumbent { pinned: true, ..incumbent });

		let pinned = solve(&Problem {
			incumbent,
			..problem(&view, &tunables, &candidates, DVec2::new(0., 2.))
		});
		assert_eq!(pinned.parameters, DVec2::ZERO);
		assert!(engaged(&pinned).pinned);

		let released = solve(&Problem {
			incumbent,
			..problem(&view, &tunables, &candidates, DVec2::new(1., 15.))
		});
		assert_eq!(released.snap, Snap::Free);
	}

	#[test]
	fn riding_a_track_catches_crossings_along_it_and_keeps_them() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [track(Kind::OnCurve, 1, horizontal(0.)), track(Kind::Alignment, 2, vertical(5.))];
		let riding = engaged_on(&candidates[0], DVec2::ZERO);

		// The pointer has strayed 11 pixels from the track, yet along the track the crossing is only 5 pixels away
		let strayed = DVec2::new(0., 11.);
		assert_eq!(engaged(&solve(&problem(&view, &tunables, &candidates, strayed))).candidates, vec![1]);

		let caught = solve(&Problem {
			incumbent: riding,
			..problem(&view, &tunables, &candidates, strayed)
		});
		assert!(caught.parameters.abs_diff_eq(DVec2::new(5., 0.), 1e-9));
		assert_eq!(engaged(&caught).tier, Tier::Spot);

		// It is further than the release distance from the pointer itself, and stays engaged all the same
		assert!(engaged(&caught).distance > tunables.release);
		let a_moment_later = solve(&Problem {
			incumbent: incumbent_after(&caught),
			..problem(&view, &tunables, &candidates, strayed + DVec2::new(0.2, 0.))
		});
		assert_eq!(a_moment_later.parameters, caught.parameters);

		// Moving on along the track leaves the crossing behind and carries on riding the track
		let moved_on = solve(&Problem {
			incumbent: incumbent_after(&a_moment_later),
			..problem(&view, &tunables, &candidates, DVec2::new(-9., 11.))
		});
		assert_eq!(moved_on.parameters, DVec2::new(-9., 0.));
		assert_eq!(engaged(&moved_on).candidates, vec![0]);
	}

	#[test]
	fn riding_a_track_catches_spots_along_it() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [track(Kind::OnCurve, 1, horizontal(0.)), spot(Kind::Midpoint, 2, DVec2::new(6., 0.))];

		// The midpoint is 9 pixels from the pointer, which is out of reach until the point is riding the track 6 pixels along from it
		let beside_the_track = DVec2::new(0., 7.);
		let fresh = solve(&problem(&view, &tunables, &candidates, beside_the_track));
		assert_eq!(fresh.parameters, DVec2::new(0., 0.));

		let riding = solve(&Problem {
			incumbent: incumbent_after(&fresh),
			..problem(&view, &tunables, &candidates, beside_the_track)
		});
		assert_eq!(riding.parameters, DVec2::new(6., 0.));

		// A spot off the track gets no such help
		let candidates = [track(Kind::OnCurve, 1, horizontal(0.)), spot(Kind::Anchor, 2, DVec2::new(6., -3.))];
		let riding = solve(&Problem {
			incumbent: engaged_on(&candidates[0], DVec2::ZERO),
			..problem(&view, &tunables, &candidates, beside_the_track)
		});
		assert_eq!(riding.parameters, DVec2::new(0., 0.));
	}

	#[test]
	fn leaving_a_spot_carries_on_along_the_track_that_was_being_ridden() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [track(Kind::OnCurve, 1, horizontal(0.)), spot(Kind::Anchor, 2, DVec2::new(6., 0.))];
		let on_the_anchor = engaged_on(&candidates[1], DVec2::new(6., 0.));

		// The track is beyond reach here, so a point that met the anchor directly simply comes free of it
		let further_along = DVec2::new(30., 10.);
		let met_directly = solve(&Problem {
			incumbent: on_the_anchor,
			..problem(&view, &tunables, &candidates, further_along)
		});
		assert_eq!(met_directly.snap, Snap::Free);

		// One that came to the anchor along the track goes back to riding it
		let reached_along_the_track = on_the_anchor.map(|incumbent| Incumbent {
			riding: Some(candidates[0].key),
			..incumbent
		});
		let left_the_anchor = solve(&Problem {
			incumbent: reached_along_the_track,
			..problem(&view, &tunables, &candidates, further_along)
		});
		assert_eq!(left_the_anchor.parameters, DVec2::new(30., 0.));
		assert_eq!(engaged(&left_the_anchor).candidates, vec![0]);
	}

	#[test]
	fn the_track_being_ridden_is_remembered_through_a_spot_that_is_not_on_it() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [track(Kind::OnCurve, 1, horizontal(0.)), spot(Kind::Anchor, 2, DVec2::new(100., 15.))];
		let mut incumbent = engaged_on(&candidates[0], DVec2::new(80., 0.));

		// Riding the track from 10 pixels off it, the point is taken by an anchor on the far side of the pointer, then returns to the track
		let mut landed = Vec::new();
		for x in [90., 100., 110., 120.] {
			let solution = solve(&Problem {
				incumbent,
				..problem(&view, &tunables, &candidates, DVec2::new(x, 10.))
			});
			assert_eq!(engaged(&solution).riding, Some(candidates[0].key));

			landed.push(solution.parameters);
			incumbent = incumbent_after(&solution);
		}

		assert_eq!(landed, vec![DVec2::new(90., 0.), DVec2::new(100., 15.), DVec2::new(100., 15.), DVec2::new(120., 0.)]);
	}

	#[test]
	fn an_engaged_crossing_holds_though_more_curves_crowd_in_than_are_tried_against_each_other() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let curve = |from: (f64, f64), to: (f64, f64)| Locus::Curve(Curve::line(from.into(), to.into()));

		// Twenty short edges pass nearer the pointer than the two curves that cross, reaching neither of them
		let mut candidates = vec![track(Kind::OnCurve, 1, curve((-20., 5.), (20., 5.))), track(Kind::OnCurve, 2, curve((5., -20.), (5., 20.)))];
		candidates.extend((0..20).map(|index| track(Kind::OnCurve, 10 + index, curve((-50., 0.1 * index as f64), (3., 0.1 * index as f64)))));
		let incumbent = Some(Incumbent {
			key: EngagementKey::crossing(candidates[0].key, candidates[1].key),
			parameters: DVec2::splat(5.),
			pinned: false,
			riding: None,
		});

		let solution = solve(&Problem {
			incumbent,
			..problem(&view, &tunables, &candidates, DVec2::ONE)
		});
		assert_eq!(solution.parameters, DVec2::splat(5.));
		assert_eq!(engaged(&solution).tier, Tier::Spot);
	}

	#[test]
	fn curves_meeting_end_to_end_do_not_cross_where_they_join() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let curve = |from: (f64, f64), to: (f64, f64)| track(Kind::OnCurve, from.1 as u64 + 100, Locus::Curve(Curve::line(from.into(), to.into())));

		let candidates = [curve((-50., 0.), (0., 0.)), curve((0., 0.), (0., 50.))];
		let solution = solve(&problem(&view, &tunables, &candidates, DVec2::new(2., 3.)));
		assert_eq!(solution.parameters, DVec2::new(0., 3.));
		assert_eq!(engaged(&solution).tier, Tier::Track);

		// Where one curve runs on past the other's end, they do cross there
		let candidates = [curve((-50., 0.), (0., 0.)), curve((0., -50.), (0., 50.))];
		let solution = solve(&problem(&view, &tunables, &candidates, DVec2::new(2., 3.)));
		assert_eq!(solution.parameters, DVec2::ZERO);
		assert_eq!(engaged(&solution).tier, Tier::Spot);
	}

	#[test]
	fn lines_drawn_from_one_point_do_not_cross_there() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());

		let candidates = [track(Kind::Alignment, 1, horizontal(0.)), track(Kind::Alignment, 2, Locus::line(DVec2::ZERO, DVec2::ONE).unwrap())];
		let solution = solve(&problem(&view, &tunables, &candidates, DVec2::new(3., 1.)));
		assert_eq!(solution.parameters, DVec2::new(3., 0.));
		assert_eq!(engaged(&solution).tier, Tier::Track);

		// The same lines drawn from different points cross where they meet
		let candidates = [track(Kind::Alignment, 1, horizontal(0.)), track(Kind::Alignment, 2, Locus::line(DVec2::ONE, DVec2::ONE).unwrap())];
		let solution = solve(&problem(&view, &tunables, &candidates, DVec2::new(3., 1.)));
		assert_eq!(solution.parameters, DVec2::ZERO);
		assert_eq!(engaged(&solution).tier, Tier::Spot);
	}

	#[test]
	fn an_engaged_crossing_holds_against_another_crossing_of_the_same_tracks() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [track(Kind::Alignment, 1, horizontal(0.)), track(Kind::OnCurve, 2, Locus::circle(DVec2::ZERO, 3.).unwrap())];

		for crossing in [DVec2::new(-3., 0.), DVec2::new(3., 0.)] {
			let incumbent = Some(Incumbent {
				key: EngagementKey::crossing(candidates[0].key, candidates[1].key),
				parameters: crossing,
				pinned: false,
				riding: None,
			});
			let solution = solve(&Problem {
				incumbent,
				..problem(&view, &tunables, &candidates, DVec2::new(0.5, 1.))
			});
			assert_eq!(solution.parameters, crossing);
		}
	}

	#[test]
	fn an_engaged_crossing_lets_go_rather_than_jump_to_another_crossing_of_the_same_tracks() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [track(Kind::Alignment, 1, horizontal(0.)), track(Kind::OnCurve, 2, Locus::circle(DVec2::ZERO, 10.).unwrap())];
		let incumbent = Some(Incumbent {
			key: EngagementKey::crossing(candidates[0].key, candidates[1].key),
			parameters: DVec2::new(-10., 0.),
			pinned: false,
			riding: None,
		});

		// The pointer is past the release distance from the crossing it was at, and short of reach of the other
		let solution = solve(&Problem {
			incumbent,
			..problem(&view, &tunables, &candidates, DVec2::new(1., 7.))
		});

		assert_eq!(engaged(&solution).tier, Tier::Track);
		assert_eq!(engaged(&solution).candidates, vec![1]);
	}

	#[test]
	fn lines_drawn_to_crowded_geometry_do_not_push_out_the_lines_of_awake_targets() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());

		// A fan of sixty lines from one far point passes closer to the pointer than two lines that cross each other within reach of it
		let from = DVec2::new(-1000., 0.);
		let fan = (1..=60).map(|index| track(Kind::PerpendicularLine, index, Locus::line_through(from, DVec2::new(0., index as f64 * 0.1)).unwrap()));
		let mut candidates: Vec<Candidate> = fan.collect();
		candidates.push(track(Kind::Alignment, 61, Locus::line_through(DVec2::new(-100., -8.), DVec2::new(0., -3.)).unwrap()));
		candidates.push(track(Kind::Alignment, 62, Locus::line_through(DVec2::new(-100., 2.), DVec2::new(0., -3.)).unwrap()));

		let solution = solve(&problem(&view, &tunables, &candidates, DVec2::ZERO));

		// The fan meets nothing within reach, which leaves the crossing of the two lines as the one place that fixes the point
		assert!(solution.parameters.abs_diff_eq(DVec2::new(0., -3.), 1e-9));
		let mut crossing = engaged(&solution).candidates.clone();
		crossing.sort();
		assert_eq!(crossing, vec![60, 61]);
	}

	#[test]
	fn a_required_track_confines_the_point_and_snaps_only_along_itself() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let required = Some(horizontal(0.));

		// A spot off the required track is ignored however near, and the pointer is simply projected
		let candidates = [spot(Kind::Anchor, 1, DVec2::new(3., 2.))];
		let solution = solve(&Problem {
			required,
			..problem(&view, &tunables, &candidates, DVec2::new(1., 4.))
		});
		assert_eq!(solution.parameters, DVec2::new(1., 0.));
		assert_eq!(solution.snap, Snap::Free);

		// A spot on it engages, as does the place another track crosses it
		let candidates = [spot(Kind::Anchor, 1, DVec2::new(6., 0.2)), track(Kind::Alignment, 2, vertical(-4.))];
		let solution = solve(&Problem {
			required,
			..problem(&view, &tunables, &candidates, DVec2::new(1., 4.))
		});
		assert!(solution.parameters.abs_diff_eq(DVec2::new(6., 0.), 1e-9));

		let solution = solve(&Problem {
			required,
			..problem(&view, &tunables, &candidates, DVec2::new(-2., 4.))
		});
		assert!(solution.parameters.abs_diff_eq(DVec2::new(-4., 0.), 1e-9));
	}

	#[test]
	fn the_grid_takes_whatever_freedom_nothing_else_claims() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let grid = Grid::rectangular(DVec2::ZERO, DVec2::splat(100.)).unwrap();
		let free = DVec2::new(130., 260.);

		// Nothing else nearby, so the point lands on the nearest grid point however far that is
		let solution = solve(&Problem {
			grid: Some(&grid),
			..problem(&view, &tunables, &[], free)
		});
		assert_eq!((solution.parameters, solution.snap), (DVec2::new(100., 300.), Snap::Grid));

		// A real spot within reach beats the grid
		let candidates = [spot(Kind::Anchor, 1, DVec2::new(133., 262.))];
		let solution = solve(&Problem {
			grid: Some(&grid),
			..problem(&view, &tunables, &candidates, free)
		});
		assert_eq!(solution.parameters, DVec2::new(133., 262.));

		// A straight track fixes one direction and the grid quantizes along it
		let candidates = [track(Kind::Alignment, 1, vertical(133.))];
		let solution = solve(&Problem {
			grid: Some(&grid),
			..problem(&view, &tunables, &candidates, free)
		});
		assert!(solution.parameters.abs_diff_eq(DVec2::new(133., 300.), 1e-9));
		assert!(engaged(&solution).quantized);

		// A curved track is slid along freely
		let candidates = [track(Kind::OnCurve, 1, Locus::circle(DVec2::new(130., 160.), 97.).unwrap())];
		let solution = solve(&Problem {
			grid: Some(&grid),
			..problem(&view, &tunables, &candidates, free)
		});
		assert!(solution.parameters.abs_diff_eq(DVec2::new(130., 257.), 1e-9));
		assert!(!engaged(&solution).quantized);

		// A required straight track is quantized at its grid crossings
		let solution = solve(&Problem {
			grid: Some(&grid),
			required: Some(horizontal(250.)),
			..problem(&view, &tunables, &[], free)
		});
		assert!(solution.parameters.abs_diff_eq(DVec2::new(100., 250.), 1e-9));
	}

	#[test]
	fn a_bounded_straight_track_is_quantized_only_where_it_reaches_a_grid_line() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let grid = Grid::rectangular(DVec2::ZERO, DVec2::splat(100.)).unwrap();
		let along_an_edge = |edge_start: f64, edge_end: f64, pointer: f64| {
			let edge = Locus::Curve(Curve::line(DVec2::new(edge_start, 50.), DVec2::new(edge_end, 50.)));
			let candidates = [track(Kind::OnBoxEdge, 1, edge)];
			let solution = solve(&Problem {
				grid: Some(&grid),
				..problem(&view, &tunables, &candidates, DVec2::new(pointer, 52.))
			});

			(solution.parameters, engaged(&solution).quantized)
		};

		// The edge lies between two grid lines and reaches neither, so the point slides along it freely
		assert_eq!(along_an_edge(110., 160., 155.), (DVec2::new(155., 50.), false));

		// The grid line nearest the pointer is past the end of the edge, which leaves the one the edge does reach
		let (landed, quantized) = along_an_edge(190., 260., 255.);
		assert!(landed.abs_diff_eq(DVec2::new(200., 50.), 1e-9) && quantized);
	}

	#[test]
	fn cycling_steps_through_the_candidates_within_reach_in_order_of_preference() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [
			track(Kind::OnCurve, 1, horizontal(2.)),
			spot(Kind::Anchor, 2, DVec2::new(3., 0.)),
			spot(Kind::BoxCorner, 3, DVec2::new(0., 6.)),
		];
		let choose = |cycle| {
			solve(&Problem {
				cycle,
				..problem(&view, &tunables, &candidates, DVec2::ZERO)
			})
		};

		assert_eq!(engaged(&choose(0)).alternatives, 3);
		assert_eq!(engaged(&choose(0)).candidates, vec![1]);
		assert_eq!(engaged(&choose(1)).candidates, vec![2]);
		assert_eq!(engaged(&choose(2)).candidates, vec![0]);
		assert_eq!(engaged(&choose(3)).candidates, vec![1]);
	}

	#[test]
	fn cycling_steps_on_from_what_is_shown_and_pins_what_it_reaches() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [
			spot(Kind::Anchor, 1, DVec2::new(3., 0.)),
			spot(Kind::BoxCorner, 2, DVec2::new(0., 4.)),
			spot(Kind::Anchor, 3, DVec2::new(-5., 0.)),
		];

		// The corner is engaged and stays so, since the anchor nearer than it is not nearer by the margin
		let shown = solve(&Problem {
			incumbent: engaged_on(&candidates[1], DVec2::new(0., 4.)),
			..problem(&view, &tunables, &candidates, DVec2::ZERO)
		});
		assert_eq!(engaged(&shown).candidates, vec![1]);

		// One step on from the corner is the furthest anchor, where starting over from the nearest would have given the corner again
		let stepped = solve(&Problem {
			incumbent: incumbent_after(&shown),
			cycle: 1,
			..problem(&view, &tunables, &candidates, DVec2::ZERO)
		});
		assert_eq!(engaged(&stepped).candidates, vec![2]);
		assert!(engaged(&stepped).pinned);

		// What was cycled to then holds, though the nearest anchor is nearer by far more than the margin
		let a_moment_later = solve(&Problem {
			incumbent: incumbent_after(&stepped),
			..problem(&view, &tunables, &candidates, DVec2::ZERO)
		});
		assert_eq!(engaged(&a_moment_later).candidates, vec![2]);
		assert!(engaged(&a_moment_later).pinned);

		// A further step wraps around to the nearest
		let wrapped = solve(&Problem {
			incumbent: incumbent_after(&a_moment_later),
			cycle: 1,
			..problem(&view, &tunables, &candidates, DVec2::ZERO)
		});
		assert_eq!(engaged(&wrapped).candidates, vec![0]);
	}

	#[test]
	fn lines_still_cross_where_more_curves_crowd_in_than_are_tried_against_each_other() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());

		// Twenty edges pass closer to the pointer than either line does, and none of them reaches either line
		let edge = |index: u64| Locus::Curve(Curve::line(DVec2::new(-50., index as f64 * 0.1), DVec2::new(3., index as f64 * 0.1)));
		let mut candidates: Vec<Candidate> = (1..=20).map(|index| track(Kind::OnCurve, index, edge(index))).collect();
		candidates.push(track(Kind::Alignment, 21, horizontal(5.)));
		candidates.push(track(Kind::Alignment, 22, vertical(5.)));

		let solution = solve(&problem(&view, &tunables, &candidates, DVec2::ZERO));

		assert!(solution.parameters.abs_diff_eq(DVec2::new(5., 5.), 1e-9));
		assert_eq!(engaged(&solution).candidates, vec![20, 21]);
	}

	#[test]
	fn two_features_of_a_sliding_thing_snap_each_direction_independently() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let features = [FeatureMap::sliding_from(DVec2::new(100., 100.)), FeatureMap::sliding_from(DVec2::new(200., 150.))];

		// The first feature can line up horizontally and the second vertically, each expressed as how far the whole thing must slide
		let first_feature_track = Candidate::track(
			CandidateKey {
				kind: Kind::Alignment,
				feature: 0,
				identity: [1, 0, 0],
			},
			horizontal(4.),
		);
		let second_feature_track = Candidate::track(
			CandidateKey {
				kind: Kind::Alignment,
				feature: 1,
				identity: [2, 0, 0],
			},
			vertical(-5.),
		);
		let candidates = [first_feature_track, second_feature_track];

		let solution = solve(&Problem {
			features: &features,
			..problem(&view, &tunables, &candidates, DVec2::new(-2., 1.))
		});

		assert!(solution.parameters.abs_diff_eq(DVec2::new(-5., 4.), 1e-9));
		assert_eq!(engaged(&solution).candidates, vec![0, 1]);
	}

	#[test]
	fn reach_is_judged_on_screen_at_any_zoom() {
		let tunables = Tunables::default();
		let candidates = [spot(Kind::Anchor, 1, DVec2::new(1., 0.))];

		// One unit of the plane is a single pixel at this zoom and 20 pixels at the next
		let zoomed_out = ViewMapping::IDENTITY;
		assert_eq!(solve(&problem(&zoomed_out, &tunables, &candidates, DVec2::ZERO)).parameters, DVec2::new(1., 0.));

		let zoomed_in = ViewMapping::from_affine(DAffine2::from_scale(DVec2::splat(20.))).unwrap();
		assert_eq!(solve(&problem(&zoomed_in, &tunables, &candidates, DVec2::ZERO)).snap, Snap::Free);
		assert_eq!(solve(&problem(&zoomed_in, &tunables, &candidates, DVec2::new(0.7, 0.))).parameters, DVec2::new(1., 0.));
	}

	#[test]
	fn a_track_is_met_where_it_appears_nearest_in_a_view_that_stretches() {
		let tunables = Tunables::default();
		let diagonal = Locus::line(DVec2::ZERO, DVec2::ONE).unwrap();
		let candidates = [track(Kind::OnCurve, 1, diagonal)];

		// Heights appear four times as large as widths, so the place on the diagonal nearest in the plane appears over 12 pixels away
		let stretched = ViewMapping::from_affine(DAffine2::from_scale(DVec2::new(1., 4.))).unwrap();
		let free = DVec2::new(6., 0.);
		assert!(stretched.screen_distance(free, diagonal.nearest(free)) > tunables.release);

		let solution = solve(&problem(&stretched, &tunables, &candidates, free));

		// Staying low on the diagonal appears under 6 pixels away
		assert!(solution.parameters.abs_diff_eq(DVec2::splat(6. / 17.), 1e-9));
		assert!((engaged(&solution).distance - stretched.screen_distance(free, solution.parameters)).abs() < 1e-9);
		assert!(engaged(&solution).distance < 6.);
	}

	#[test]
	fn the_grid_point_landed_on_is_the_one_that_appears_nearest() {
		let tunables = Tunables::default();
		let grid = Grid::rectangular(DVec2::ZERO, DVec2::splat(10.)).unwrap();

		// A view that leans the plane over makes a grid point other than the nearest in the plane appear nearest
		let leaning = ViewMapping::from_affine(DAffine2::from_cols(DVec2::X, DVec2::new(1.5, 1.), DVec2::ZERO)).unwrap();
		let free = DVec2::new(4., 4.);

		let solution = solve(&Problem {
			grid: Some(&grid),
			..problem(&leaning, &tunables, &[], free)
		});

		let appears = |point: DVec2| leaning.screen_distance(free, point);
		assert_eq!(solution.snap, Snap::Grid);
		assert_ne!(solution.parameters, grid.nearest_point(free));

		// No grid point for a long way around appears nearer
		for x in -20..=20 {
			for y in -20..=20 {
				assert!(appears(solution.parameters) <= appears(DVec2::new(x as f64, y as f64) * 10.));
			}
		}
	}

	#[test]
	fn the_outcome_does_not_depend_on_the_order_of_the_candidates() {
		let (view, tunables) = (ViewMapping::IDENTITY, Tunables::default());
		let candidates = [
			spot(Kind::BoxCorner, 1, DVec2::new(3., 3.)),
			spot(Kind::BoxCorner, 2, DVec2::new(-3., 3.)),
			spot(Kind::Anchor, 3, DVec2::new(3., -3.)),
			track(Kind::OnCurve, 4, horizontal(1.)),
			track(Kind::Alignment, 5, vertical(2.)),
			track(Kind::Alignment, 6, vertical(-2.)),
		];
		let expected = solve(&problem(&view, &tunables, &candidates, DVec2::ZERO));

		// Rotating and reversing the list visits every candidate in several different orders
		for rotation in 0..candidates.len() {
			let mut reordered = candidates;
			reordered.rotate_left(rotation);
			for reversed in [false, true] {
				if reversed {
					reordered.reverse();
				}

				let solution = solve(&problem(&view, &tunables, &reordered, DVec2::ZERO));
				assert_eq!(solution.parameters, expected.parameters);

				let keys = |solution: &Solution, list: &[Candidate]| engaged(solution).candidates.iter().map(|&index| list[index].key).collect::<Vec<_>>();
				let mut found = keys(&solution, &reordered);
				let mut wanted = keys(&expected, &candidates);
				found.sort();
				wanted.sort();
				assert_eq!(found, wanted);
			}
		}
	}
}
