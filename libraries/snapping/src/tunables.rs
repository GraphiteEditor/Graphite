/// The numbers that decide how snapping feels, gathered in one place so they can be tuned together. Distances are in screen pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tunables {
	/// How close a snap must be before it can engage.
	pub reach: f64,
	/// How far the pointer must stray from an engaged snap before it lets go, which is further than `reach` so snaps do not flicker at their edge.
	pub release: f64,
	/// How much nearer a rival of equal standing must be before it takes over from the engaged snap.
	pub switch_margin: f64,
	/// How far a position may lie from a track and still count as being on it.
	pub on_track: f64,
	/// How much further away than the nearest candidate a more specific kind of snap may be and still be preferred to it.
	pub tie: f64,
}

impl Tunables {
	/// How far from the pointer candidates are gathered, which takes in anything at the release distance along a track that is itself at the release distance.
	pub fn gathering_distance(&self) -> f64 {
		self.release * std::f64::consts::SQRT_2
	}
}

impl Default for Tunables {
	fn default() -> Self {
		Self {
			reach: 8.,
			release: 12.,
			switch_margin: 2.,
			on_track: 0.5,
			tie: 0.5,
		}
	}
}
