<script lang="ts">
	import { getContext, onDestroy } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import HistoryShot from "/src/components/views/HistoryShot.svelte";
	import IconButton from "/src/components/widgets/buttons/IconButton.svelte";
	import TextButton from "/src/components/widgets/buttons/TextButton.svelte";
	import TextInput from "/src/components/widgets/inputs/TextInput.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import type { HistoryStore } from "/src/stores/history";
	import type { EditorWrapper } from "/wrapper/pkg/graphite_wasm_wrapper";

	const editor = getContext<EditorWrapper>("editor");
	const history = getContext<HistoryStore>("history");
	const thumbnails = history.thumbnails;

	// Relative times age while the panel stays open
	let now = Date.now();
	const clock = setInterval(() => (now = Date.now()), 15_000);
	onDestroy(() => clearInterval(clock));

	// Relative for the past week, a date before that
	function when(ms: number | undefined, now: number): string {
		if (ms === undefined) return "";
		const seconds = Math.max(0, Math.round((now - ms) / 1000));
		if (seconds < 45) return "just now";
		const minutes = Math.round(seconds / 60);
		if (minutes < 60) return `${minutes} min ago`;
		const hours = Math.round(minutes / 60);
		if (hours < 24) return `${hours} h ago`;
		const days = Math.round(hours / 24);
		if (days < 7) return days === 1 ? "yesterday" : `${days} days ago`;
		return new Date(ms).toLocaleDateString();
	}

	// Alone, the newest step not yet in history is where the document is
	$: pendingHead = !$history.collaborating && $history.progress.length > 0;

	function count(n: number, one: string, many: string): string {
		return `${n} ${n === 1 ? one : many}`;
	}

	function authorLabel(row: { author: string; mine: boolean }): string {
		return row.mine ? `${row.author} (you)` : row.author;
	}

	let renamingId: string | undefined = undefined;
	let taggingId: string | undefined = undefined;

	function commitName(id: string, label: string) {
		editor.renameHistoryInteraction(id, label);
		renamingId = undefined;
	}

	function commitTag(id: string, tag: string) {
		if (tag.trim()) editor.tagHistoryInteraction(id, tag, true);
		taggingId = undefined;
	}

	// The inputs open focused and close once focus leaves them, committed or not
	function focusInput(node: HTMLElement) {
		const input = node.querySelector("[data-input-element]");
		if (input instanceof HTMLElement) input.focus();
	}
</script>

<LayoutCol class="history-panel">
	{#if $history.following}
		<LayoutRow class="following">
			<TextLabel>Viewing a branch</TextLabel>
			<TextButton label="Back to now" action={() => editor.followHistoryBranch(undefined)} />
		</LayoutRow>
	{/if}
	<LayoutCol class="body" scrollableY={true}>
		{#each $history.progress as row, index}
			<!-- Alone, work not yet in history is shown as an ordinary step -->
			<div class="row" class:head={pendingHead && index === 0}>
				<span class="gutter"></span>
				{#if $history.collaborating}
					<span class="rail"><span class="dot hollow" style:border-color={row.color}></span></span>
				{:else}
					<span class="rail"><span class="dot"></span></span>
				{/if}
				{#if row.thumbnail}<HistoryShot shot={row.thumbnail.after} url={$thumbnails[row.thumbnail.after.id]} />{/if}
				<div class="text">
					<span class="label">{row.label || (row.open ? "In progress" : "Waiting to enter history")}</span>
					<span class="meta">
						{#if $history.collaborating}<span class="author" class:anonymous={row.anonymous}>{authorLabel(row)}</span> ·{/if}
						{count(row.ops, "change", "changes")}
						{#if $history.collaborating}· {row.open ? "in progress" : "not yet in history"}{/if}
					</span>
				</div>
				{#if pendingHead && index === 0}<span class="marker">Now</span>{/if}
			</div>
		{/each}
		{#each $history.rows as row (row.id)}
			<div class="row" class:head={row.head && !pendingHead} class:undone={row.undone} class:abandoned={row.abandoned}>
				{#if row.deltas > 1 || row.thumbnail?.before}
					<button
						class="expand"
						class:expanded={row.expanded}
						data-tooltip-label={row.expanded ? "Hide the changes" : "Show the changes"}
						on:click={() => editor.expandHistoryInteraction(row.id, !row.expanded)}
					></button>
				{:else}
					<span class="gutter"></span>
				{/if}
				<span class="rail"><span class="dot" style:background={$history.collaborating || !row.mine ? row.color : undefined}></span></span>
				{#if row.thumbnail}<HistoryShot shot={row.thumbnail.after} url={$thumbnails[row.thumbnail.after.id]} />{/if}
				<div class="text">
					{#if renamingId === row.id}
						<span class="editing" use:focusInput on:focusout={() => (renamingId = undefined)}>
							<TextInput value={row.named ? row.label : ""} placeholder={row.label} on:commitText={({ detail }) => commitName(row.id, detail)} />
						</span>
					{:else}
						<span class="label" class:named={row.named}>{row.label}</span>
					{/if}
					<span class="meta">
						{#if $history.collaborating || !row.mine}<span class="author" class:anonymous={row.anonymous}>{authorLabel(row)}</span> ·{/if}
						{#if row.time}{when(row.time, now)} ·{/if}
						{count(row.deltas, "change", "changes")}
					</span>
					{#if row.tags.length > 0 || taggingId === row.id}
						<span class="tags">
							{#each row.tags as tag (tag)}
								<span class="tag">{tag}<button class="remove" data-tooltip-label="Remove the tag" on:click={() => editor.tagHistoryInteraction(row.id, tag, false)}>×</button></span>
							{/each}
							{#if taggingId === row.id}
								<span class="editing" use:focusInput on:focusout={() => (taggingId = undefined)}>
									<TextInput value="" placeholder="Tag name" narrow={true} on:commitText={({ detail }) => commitTag(row.id, detail)} />
								</span>
							{/if}
						</span>
					{/if}
					{#if row.branches.length > 0}
						<span class="branches">
							{#each row.branches as branch (branch.id)}
								<button class="chip branch" data-tooltip-label="Show this branch" on:click={() => editor.followHistoryBranch(branch.id)}>↳ {branch.label}</button>
							{/each}
						</span>
					{/if}
				</div>
				<span class="actions">
					<button class="chip" data-tooltip-label="Name this step" on:click={() => (renamingId = row.id)}>Name</button>
					<button class="chip" data-tooltip-label="Tag this step" on:click={() => (taggingId = row.id)}>Tag</button>
					{#if row.undone || row.abandoned}
						<IconButton
							icon="HistoryRedo"
							size={16}
							tooltipLabel="Bring back"
							tooltipDescription={row.undone ? "Redo up to and including this step." : "Do this step again on top of the current line."}
							action={() => editor.bringBackHistoryInteraction(row.id)}
						/>
					{:else}
						{#if !row.head}
							<IconButton
								icon="HistoryUndo"
								size={16}
								tooltipLabel="Go back to here"
								tooltipDescription={$history.collaborating
									? "Move everyone's document back to this step. The steps since stay as a branch."
									: "Move the document back to this step. The steps since stay as a branch."}
								action={() => editor.goBackToHistoryInteraction(row.id)}
							/>
						{/if}
						<IconButton
							icon="Trash"
							size={16}
							tooltipLabel="Remove this step"
							tooltipDescription="Take this step out and keep the ones after it."
							action={() => editor.removeHistoryInteraction(row.id)}
						/>
					{/if}
				</span>
				{#if row.head && !pendingHead}<span class="marker">Now</span>{/if}
				{#if row.undone}<span class="marker">Undone</span>{/if}
				{#if row.abandoned}<span class="marker">Branch</span>{/if}
			</div>
			{#if row.expanded}
				{#if row.thumbnail?.before}
					<div class="row comparison" class:undone={row.undone}>
						<span class="gutter"></span>
						<span class="rail"></span>
						<HistoryShot shot={row.thumbnail.before} url={$thumbnails[row.thumbnail.before.id]} large={true} />
						<span class="arrow">→</span>
						<HistoryShot shot={row.thumbnail.after} url={$thumbnails[row.thumbnail.after.id]} large={true} />
					</div>
				{/if}
				{#each row.details as delta (delta.id)}
					<div class="row delta" class:undone={row.undone}>
						<span class="gutter"></span>
						<span class="rail"><span class="tick"></span></span>
						<div class="text">
							<span class="label">{delta.label}</span>
							<span class="meta">
								{#if $history.collaborating || !delta.mine}<span class="author">{delta.author}</span>{/if}
								{#if delta.time}{$history.collaborating || !delta.mine ? "· " : ""}{when(delta.time, now)}{/if}
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

		.following {
			flex: 0 0 auto;
			align-items: center;
			justify-content: space-between;
			padding: 0 8px 4px 8px;
		}

		.row {
			display: flex;
			align-items: center;
			min-height: 32px;
			padding: 2px 8px 2px 4px;
			gap: 6px;

			&.head {
				background: var(--color-3-darkgray);
			}

			&.undone,
			&.abandoned {
				.rail,
				.text {
					opacity: 0.6;
				}
			}

			&.delta {
				min-height: 24px;
				font-size: 11px;
			}

			&.comparison {
				padding-top: 4px;
				padding-bottom: 4px;

				.arrow {
					color: var(--color-8-uppergray);
				}
			}

			.actions {
				flex: 0 0 auto;
				display: flex;
				align-items: center;
				gap: 2px;
				visibility: hidden;
			}

			&:hover .actions,
			&:focus-within .actions {
				visibility: visible;
			}

			.chip {
				font-size: 11px;
				line-height: 14px;
				padding: 0 4px;
				border: none;
				border-radius: 2px;
				background: var(--color-4-dimgray);
				color: var(--color-c-brightgray);
				cursor: pointer;

				&:hover {
					background: var(--color-5-dullgray);
					color: var(--color-e-nearwhite);
				}
			}

			.gutter {
				flex: 0 0 16px;
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
				color: var(--color-8-uppergray);

				&:hover {
					color: var(--color-e-nearwhite);
				}

				&::after {
					content: "";
					position: absolute;
					left: 5px;
					top: 4px;
					border: 4px solid transparent;
					border-left: 5px solid currentColor;
				}

				&.expanded::after {
					transform: rotate(90deg);
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
					background: var(--color-8-uppergray);

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

				.label.named {
					font-weight: 600;
				}

				.tags {
					display: flex;
					flex-wrap: wrap;
					align-items: center;
					gap: 4px;
					margin-top: 2px;

					.tag {
						display: inline-flex;
						align-items: center;
						gap: 2px;
						font-size: 11px;
						line-height: 14px;
						padding: 0 2px 0 4px;
						border-radius: 2px;
						background: var(--color-accent-hover);
						color: var(--color-f-white);

						.remove {
							border: none;
							background: none;
							color: inherit;
							padding: 0 2px;
							cursor: pointer;
							line-height: 14px;
						}
					}
				}

				.branches {
					display: flex;
					flex-wrap: wrap;
					gap: 4px;
					margin-top: 2px;

					.branch {
						max-width: 100%;
						white-space: nowrap;
						overflow: hidden;
						text-overflow: ellipsis;
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

		.editing {
			display: contents;
		}

		.more {
			display: flex;
			justify-content: center;
			padding: 8px;
		}
	}
</style>
