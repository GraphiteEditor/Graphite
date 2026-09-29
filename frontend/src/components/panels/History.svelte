<script lang="ts">
	import { getContext } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import TextButton from "/src/components/widgets/buttons/TextButton.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import type { HistoryStore } from "/src/stores/history";
	import type { EditorWrapper } from "/wrapper/pkg/graphite_wasm_wrapper";

	const editor = getContext<EditorWrapper>("editor");
	const history = getContext<HistoryStore>("history");

	// "3 min ago" for the recent past, a date for anything older; nothing when the retirer recorded no time.
	function when(ms: number | undefined): string {
		if (ms === undefined) return "";
		const seconds = Math.max(0, Math.round((Date.now() - ms) / 1000));
		if (seconds < 45) return "just now";
		const minutes = Math.round(seconds / 60);
		if (minutes < 60) return `${minutes} min ago`;
		const hours = Math.round(minutes / 60);
		if (hours < 24) return `${hours} h ago`;
		const days = Math.round(hours / 24);
		if (days < 7) return days === 1 ? "yesterday" : `${days} days ago`;
		return new Date(ms).toLocaleDateString();
	}

	function count(n: number, one: string, many: string): string {
		return `${n} ${n === 1 ? one : many}`;
	}
</script>

<LayoutCol class="history-panel">
	<LayoutCol class="body" scrollableY={true}>
		{#each $history.progress as row}
			<div class="row progress">
				<span class="rail"><span class="dot hollow" style:border-color={row.color}></span></span>
				<div class="text">
					<span class="label">In progress</span>
					<span class="meta">
						<span class="author" class:anonymous={row.anonymous}>{row.mine ? `${row.author} (you)` : row.author}</span>
						· {count(row.ops, "change", "changes")} not yet in history
					</span>
				</div>
			</div>
		{/each}
		{#each $history.rows as row (row.id)}
			<div class="row" class:head={row.head} class:undone={row.undone}>
				<button
					class="expand"
					class:expanded={row.expanded}
					title=""
					data-tooltip-label={row.expanded ? "Hide the changes" : "Show the changes"}
					on:click={() => editor.expandHistoryInteraction(row.id, !row.expanded)}
				></button>
				<span class="rail"><span class="dot" style:background={row.color}></span></span>
				<div class="text">
					<span class="label">{row.label}</span>
					<span class="meta">
						<span class="author" class:anonymous={row.anonymous}>{row.mine ? `${row.author} (you)` : row.author}</span>
						{#if row.time}· {when(row.time)}{/if}
						· {count(row.deltas, "change", "changes")}
						{#if row.branches > 0}· {count(row.branches, "branch", "branches")}{/if}
					</span>
				</div>
				{#if row.head}<span class="marker">Now</span>{/if}
				{#if row.undone}<span class="marker">Undone</span>{/if}
			</div>
			{#if row.expanded}
				{#each row.details as delta (delta.id)}
					<div class="row delta" class:undone={row.undone}>
						<span class="rail"><span class="tick"></span></span>
						<div class="text">
							<span class="label">{delta.label}</span>
							<span class="meta">
								<span class="author">{delta.author}</span>
								{#if delta.time}· {when(delta.time)}{/if}
							</span>
						</div>
					</div>
				{/each}
			{/if}
		{/each}
		{#if $history.more}
			<div class="more">
				<TextButton label="Show older" action={() => editor.loadMoreHistory()} />
			</div>
		{/if}
		{#if $history.rows.length === 0 && $history.progress.length === 0}
			<TextLabel italic={true}>Nothing in the history yet.</TextLabel>
		{/if}
	</LayoutCol>
</LayoutCol>

<style lang="scss">
	.history-panel {
		flex-grow: 1;
		padding: 4px 0;

		.row {
			display: flex;
			align-items: center;
			min-height: 32px;
			padding: 2px 8px 2px 4px;
			gap: 6px;

			&.head {
				background: var(--color-3-darkgray);
			}

			&.undone {
				opacity: 0.5;
			}

			&.delta {
				min-height: 24px;
				padding-left: 28px;
				font-size: 11px;
			}

			.expand {
				flex: 0 0 auto;
				width: 16px;
				height: 16px;
				padding: 0;
				border: none;
				background: none;
				position: relative;
				cursor: pointer;

				&::after {
					content: "";
					position: absolute;
					left: 5px;
					top: 4px;
					border: 4px solid transparent;
					border-left: 5px solid var(--color-8-uppergray);
				}

				&.expanded::after {
					left: 3px;
					top: 6px;
					border: 4px solid transparent;
					border-top: 5px solid var(--color-8-uppergray);
				}

				&:hover::after {
					border-left-color: var(--color-e-nearwhite);
				}

				&.expanded:hover::after {
					border-left-color: transparent;
					border-top-color: var(--color-e-nearwhite);
				}
			}

			.rail {
				flex: 0 0 12px;
				display: flex;
				justify-content: center;

				.dot {
					width: 8px;
					height: 8px;
					border-radius: 50%;

					&.hollow {
						background: none;
						border: 2px solid;
						width: 6px;
						height: 6px;
					}
				}

				.tick {
					width: 4px;
					height: 4px;
					border-radius: 50%;
					background: var(--color-6-lowergray);
				}
			}

			.text {
				flex: 1 1 0;
				min-width: 0;
				display: flex;
				flex-direction: column;
				line-height: 16px;

				.label {
					white-space: nowrap;
					overflow: hidden;
					text-overflow: ellipsis;
				}

				.meta {
					color: var(--color-a-softgray);
					font-size: 11px;
					white-space: nowrap;
					overflow: hidden;
					text-overflow: ellipsis;

					.anonymous {
						font-style: italic;
					}
				}
			}

			.marker {
				flex: 0 0 auto;
				font-size: 10px;
				line-height: 14px;
				padding: 0 4px;
				border-radius: 2px;
				background: var(--color-5-dullgray);
				color: var(--color-e-nearwhite);
			}
		}

		.more {
			display: flex;
			justify-content: center;
			padding: 8px;
		}
	}
</style>
