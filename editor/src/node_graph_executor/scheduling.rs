use graphene_std::runtime::{Clock, Scheduling};
use std::time::Duration;

/// The frame the host marked last, which the evaluations finished tasks ask for fill until its budget is spent.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Frame {
	start: Duration,
}

impl Frame {
	pub fn begin(clock: &dyn Clock) -> Self {
		Self { start: clock.now() }
	}

	/// Whether an evaluation asked for now still starts within the frame's budget.
	pub fn has_room(&self, scheduling: &Scheduling, clock: &dyn Clock) -> bool {
		scheduling.frame_budget.is_none_or(|budget| clock.now().saturating_sub(self.start) < budget)
	}

	/// How long until the next frame starts, counting frames of the budget's length from this one's start.
	pub fn until_next(&self, scheduling: &Scheduling, clock: &dyn Clock) -> Duration {
		let Some(budget) = scheduling.frame_budget.filter(|budget| !budget.is_zero()) else {
			return Duration::ZERO;
		};
		let (elapsed, budget) = (clock.now().saturating_sub(self.start).as_nanos(), budget.as_nanos());
		Duration::from_nanos(((budget - elapsed % budget) % budget) as u64)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::test_utils::FakeClock;

	const MILLISECOND: Duration = Duration::from_millis(1);

	/// The evaluations each of `frames` frames runs, each taking `evaluation` and asking for the next.
	fn evaluations_per_frame(scheduling: Scheduling, evaluation: Duration, frames: usize) -> Vec<usize> {
		let clock = FakeClock::default();
		(0..frames)
			.map(|_| {
				let frame = Frame::begin(&clock);
				let mut evaluations = 0;
				while frame.has_room(&scheduling, &clock) && evaluations < 100 {
					clock.advance(evaluation);
					evaluations += 1;
				}
				clock.advance(frame.until_next(&scheduling, &clock));
				evaluations
			})
			.collect()
	}

	#[test]
	fn a_long_evaluation_runs_once_per_frame() {
		assert_eq!(evaluations_per_frame(Scheduling::default(), 30 * MILLISECOND, 4), [1; 4]);
	}

	#[test]
	fn short_evaluations_chain_within_a_frame() {
		assert_eq!(evaluations_per_frame(Scheduling::default(), 2 * MILLISECOND, 4), [8; 4]);
		assert_eq!(evaluations_per_frame(Scheduling::default(), 5 * MILLISECOND, 2), [4; 2]);
	}

	#[test]
	fn no_budget_never_waits() {
		assert_eq!(evaluations_per_frame(Scheduling::UNLIMITED, 30 * MILLISECOND, 2), [100; 2]);
	}

	#[test]
	fn the_next_frame_starts_a_budget_after_the_last() {
		let (clock, scheduling) = (FakeClock::default(), Scheduling::default());
		let frame = Frame::begin(&clock);
		clock.advance(20 * MILLISECOND);
		assert!(!frame.has_room(&scheduling, &clock));
		assert_eq!(frame.until_next(&scheduling, &clock), 12 * MILLISECOND);
		clock.advance(30 * MILLISECOND);
		assert_eq!(frame.until_next(&scheduling, &clock), 14 * MILLISECOND);
	}

	mod runtime {
		use super::*;
		use crate::messages::prelude::*;
		use crate::task::{Spawn, Task};
		use crate::test_utils::EditorTestUtils;
		use glam::{DVec2, UVec2};
		use graphene_std::SourceId;
		use graphene_std::runtime::Runtime;
		use std::sync::atomic::{AtomicBool, Ordering};
		use std::sync::{Arc, Mutex};

		const SOURCE: SourceId = SourceId(1);

		/// Runs each task to its end as it is spawned, unless told to hold the tasks spawned meanwhile.
		#[derive(Clone, Default)]
		struct HoldingSpawner {
			hold: Arc<AtomicBool>,
			held: Arc<Mutex<Vec<Task>>>,
		}

		impl Spawn for HoldingSpawner {
			fn spawn(&self, task: Task) {
				match self.hold.load(Ordering::Relaxed) {
					true => self.held.lock().unwrap().push(task),
					false => futures::executor::block_on(task),
				}
			}
		}

		/// An editor with a drawn document, its runtime in a frame of the default budget on `clock`, with `SOURCE` live.
		async fn editor(clock: &Arc<FakeClock>, spawner: HoldingSpawner) -> EditorTestUtils {
			let mut editor = EditorTestUtils::create_with(Scheduling::default(), clock.clone(), spawner);
			editor.new_document().await;
			editor.draw_rect(0., 0., 100., 100.).await;
			editor.runtime.async_runtime().retain_sources(&[SOURCE]);
			editor.runtime.begin_frame();
			editor
		}

		/// Ends `count` tasks of `SOURCE`, each bumping its generation.
		fn finish_tasks(editor: &EditorTestUtils, count: usize) {
			for _ in 0..count {
				editor.runtime.async_runtime().spawn(SOURCE, Box::pin(async {}));
			}
		}

		/// Runs the runtime once, telling whether it evaluated the graph, as its output reaching the editor shows.
		async fn evaluates(editor: &mut EditorTestUtils) -> bool {
			editor.runtime.run().await;
			let messages = editor.editor.handle_message(Message::Wake);
			messages.iter().any(|message| matches!(message, FrontendMessage::UpdateDocumentArtwork { .. }))
		}

		#[tokio::test]
		async fn a_finished_task_waits_for_the_next_frame_once_the_budget_is_spent() {
			let clock = Arc::new(FakeClock::default());
			let mut editor = editor(&clock, HoldingSpawner::default()).await;

			finish_tasks(&editor, 1);
			assert!(evaluates(&mut editor).await, "the frame had room");
			assert_eq!(editor.runtime.until_next_frame(), None);

			clock.advance(20 * MILLISECOND);
			finish_tasks(&editor, 1);
			assert!(!evaluates(&mut editor).await, "the spent frame evaluated");
			assert_eq!(editor.runtime.until_next_frame(), Some(12 * MILLISECOND));
			assert!(!evaluates(&mut editor).await, "a second run of the spent frame evaluated");

			clock.advance(12 * MILLISECOND);
			editor.runtime.begin_frame();
			assert!(evaluates(&mut editor).await, "the deferred evaluation was lost");
			assert_eq!(editor.runtime.until_next_frame(), None);
			assert!(!evaluates(&mut editor).await, "the deferred evaluation ran twice");
		}

		#[tokio::test]
		async fn finished_tasks_ask_for_one_evaluation() {
			let clock = Arc::new(FakeClock::default());
			let mut editor = editor(&clock, HoldingSpawner::default()).await;

			finish_tasks(&editor, 3);
			assert!(evaluates(&mut editor).await);
			assert!(!evaluates(&mut editor).await, "three finished tasks evaluated more than once");
		}

		#[tokio::test]
		async fn an_explicit_request_runs_past_the_budget_and_takes_the_finished_tasks_along() {
			let clock = Arc::new(FakeClock::default());
			let mut editor = editor(&clock, HoldingSpawner::default()).await;

			clock.advance(100 * MILLISECOND);
			finish_tasks(&editor, 1);
			let portfolio = &mut editor.editor.dispatcher.message_handlers.portfolio_message_handler;
			let document_id = portfolio.active_document_id.unwrap();
			let document = portfolio.documents.get_mut(&document_id).unwrap();
			portfolio
				.executor
				.submit_current_node_graph_evaluation(document, document_id, UVec2::ONE, 1., Default::default(), DVec2::ZERO)
				.unwrap();

			assert!(evaluates(&mut editor).await, "the explicit request waited for the next frame");
			assert_eq!(editor.runtime.until_next_frame(), None);
			assert!(!evaluates(&mut editor).await, "the finished task asked for another evaluation");
		}

		#[tokio::test]
		async fn a_graph_update_drops_the_tasks_of_a_removed_source() {
			let clock = Arc::new(FakeClock::default());
			let spawner = HoldingSpawner::default();
			let mut editor = editor(&clock, spawner.clone()).await;

			spawner.hold.store(true, Ordering::Relaxed);
			finish_tasks(&editor, 1);
			spawner.hold.store(false, Ordering::Relaxed);
			// The graph update lists the document's sources, which `SOURCE` is not one of
			editor.draw_rect(10., 10., 50., 50.).await;

			for task in std::mem::take(&mut *spawner.held.lock().unwrap()) {
				futures::executor::block_on(task);
			}
			assert!(!editor.runtime.async_runtime().bumped(), "the removed source's task bumped a generation");
			assert!(!evaluates(&mut editor).await, "the removed source's task asked for an evaluation");
		}
	}
}
