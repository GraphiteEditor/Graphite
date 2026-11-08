/// The top-level stages of a trace in the order ollev runs them, each with its share of the trace's time.
/// Measured with ollev 0.1.2 on 1024 by 1024 pixel renders of the demo artwork with the node's default stages, averaged over the images.
/// The unnamed stage is the setup before the first checkpoint names one.
const STAGES: [(&str, f64); 8] = [
	("", 0.03),
	("analyze", 0.21),
	("plan", 0.09),
	("draw", 0.18),
	("redraw", 0.09),
	("shapes", 0.16),
	("families", 0.),
	("passes", 0.24),
];

const FAMILIES: usize = 6;

/// The share of the trace's time the translucent families take with translucency on, which ranged from 6 to 93% on the same images.
/// The other stages split the rest in proportion to their shares.
const FAMILIES_SHARE: f64 = 0.35;

/// How many nested traces of the families stage fill half of its share, since how many there will be isn't known up front.
const HALF_FILLING_NESTED_TRACES: f64 = 4.;

/// Folds the stages ollev reports into one fraction of the whole trace that never goes down.
#[derive(Clone, Debug)]
pub struct TraceProgress {
	translucency: bool,
	stage: usize,
	/// The fraction of the current stage, or for the families stage how many traces beneath its layers have started.
	within_stage: f64,
	previous_stage: &'static str,
	fraction: f64,
}

impl TraceProgress {
	pub fn new(translucency: bool) -> Self {
		Self {
			translucency,
			stage: 0,
			within_stage: 0.,
			previous_stage: "",
			fraction: 0.,
		}
	}

	/// Takes in a checkpoint of the trace, where it is in `stage` with `fraction` of the stage done.
	pub fn observe(&mut self, stage: &'static str, fraction: f64) {
		let entered = stage != self.previous_stage;
		self.previous_stage = stage;

		// The traces beneath translucent layers repeat the stages before the families stage, so they count as part of it
		if self.stage == FAMILIES && stage != "passes" {
			if entered && stage == "analyze" {
				self.within_stage += 1.;
			}
		} else if let Some(index) = STAGES.iter().position(|(name, _)| *name == stage)
			&& index >= self.stage
		{
			if index > self.stage {
				self.stage = index;
				self.within_stage = 0.;
			}
			if index != FAMILIES {
				self.within_stage = fraction.clamp(0., 1.);
			}
		}

		let within_share = match self.stage {
			FAMILIES => self.within_stage / (self.within_stage + HALF_FILLING_NESTED_TRACES),
			_ => self.within_stage,
		};
		let before: f64 = (0..self.stage).map(|index| self.share(index)).sum();
		self.fraction = self.fraction.max(before + self.share(self.stage) * within_share);
	}

	/// How much of the trace is done, from 0 to 1.
	pub fn fraction(&self) -> f64 {
		self.fraction.min(1.)
	}

	fn share(&self, index: usize) -> f64 {
		match (self.translucency, index) {
			(false, _) => STAGES[index].1,
			(true, FAMILIES) => FAMILIES_SHARE,
			(true, _) => STAGES[index].1 * (1. - FAMILIES_SHARE),
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn the_stages_fill_the_whole_trace() {
		for translucency in [false, true] {
			let progress = TraceProgress::new(translucency);
			let total: f64 = (0..STAGES.len()).map(|index| progress.share(index)).sum();
			assert!((total - 1.).abs() < 1e-9, "the shares add up to {total}");
		}
	}

	#[test]
	fn progress_moves_through_the_stages_and_within_them() {
		let mut progress = TraceProgress::new(false);
		progress.observe("analyze", 0.);
		assert!((progress.fraction() - 0.03).abs() < 1e-9);

		progress.observe("draw", 0.5);
		assert!((progress.fraction() - (0.03 + 0.21 + 0.09 + 0.09)).abs() < 1e-9);

		progress.observe("passes", 0.);
		assert!((progress.fraction() - 0.76).abs() < 1e-9);
	}

	#[test]
	fn the_traces_beneath_translucent_layers_never_move_progress_back() {
		let mut progress = TraceProgress::new(true);
		progress.observe("shapes", 0.);
		progress.observe("families", 0.);
		let families_start = progress.fraction();

		let mut previous = families_start;
		for _ in 0..3 {
			for stage in ["analyze", "plan", "draw", "redraw", "shapes"] {
				progress.observe(stage, 0.5);
				assert!(progress.fraction() >= previous, "{stage} moved progress back");
				previous = progress.fraction();
			}
		}
		assert!(previous > families_start, "the nested traces didn't move progress");
		assert!(previous < families_start + FAMILIES_SHARE, "the nested traces filled the families stage");

		progress.observe("passes", 0.);
		assert!((progress.fraction() - (1. - 0.24 * (1. - FAMILIES_SHARE))).abs() < 1e-9);
	}
}
