<script lang="ts">
	import { getContext, tick } from "svelte";
	import HistoryShot from "/src/components/views/HistoryShot.svelte";
	import IconButton from "/src/components/widgets/buttons/IconButton.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import type { HistoryStore } from "/src/stores/history";
	import type { EditorWrapper, HistoryGraphNode } from "/wrapper/pkg/graphite_wasm_wrapper";

	const editor = getContext<EditorWrapper>("editor");
	const history = getContext<HistoryStore>("history");
	const graph = history.graph;
	const thumbnails = history.thumbnails;

	// Each step's card and the room around it
	const NODE_WIDTH = 168;
	const NODE_HEIGHT = 44;
	const COLUMN = 184;
	const ROW = 72;
	const PADDING = 24;

	let body: HTMLDivElement | undefined;
	let scrolledToHead = false;

	$: byId = new Map($graph.nodes.map((node) => [node.id, node]));
	$: head = $graph.nodes.find((node) => node.head);
	$: if (head && !scrolledToHead) scrollToHead();

	function left(node: HistoryGraphNode): number {
		return PADDING + node.x * COLUMN;
	}

	function top(node: HistoryGraphNode): number {
		return PADDING + node.depth * ROW;
	}

	// From the bottom of the parent's card to the top of the child's
	function edge(parent: HistoryGraphNode, child: HistoryGraphNode): string {
		const [x1, y1] = [left(parent) + NODE_WIDTH / 2, top(parent) + NODE_HEIGHT];
		const [x2, y2] = [left(child) + NODE_WIDTH / 2, top(child)];
		const middle = (y1 + y2) / 2;
		return `M ${x1} ${y1} C ${x1} ${middle}, ${x2} ${middle}, ${x2} ${y2}`;
	}

	function onHeadLine(node: HistoryGraphNode): boolean {
		return !node.undone && !node.abandoned;
	}

	function tooltip(node: HistoryGraphNode): string {
		if (node.head) return "Where the document is now";
		if (node.undone) return "Redo up to here";
		if (node.abandoned) return "Show this branch in the History panel";
		return "Go back to here";
	}

	function choose(node: HistoryGraphNode) {
		if (node.abandoned) editor.followHistoryBranch(node.id);
		else if (!node.head) editor.goBackToHistoryInteraction(node.id);
	}

	async function scrollToHead() {
		scrolledToHead = true;
		await tick();
		body?.querySelector("[data-head]")?.scrollIntoView({ block: "center", inline: "center" });
	}
</script>

<!-- Pointer presses and wheel scrolls stay here rather than reaching the viewport's tools beneath -->
<div class="history-graph" on:pointerdown|stopPropagation on:wheel|stopPropagation>
	<div class="header">
		<TextLabel>History</TextLabel>
		<IconButton icon="CloseX" size={16} tooltipLabel="Hide the history graph" action={() => editor.setHistoryGraphOpen(false)} />
	</div>
	<div class="body" bind:this={body}>
		{#if $graph.nodes.length === 0}
			<TextLabel italic={true}>Nothing in the history yet.</TextLabel>
		{/if}
		<div class="canvas" style:width={`${PADDING * 2 + Math.max(0, $graph.width - 1) * COLUMN + NODE_WIDTH}px`} style:height={`${PADDING * 2 + $graph.depth * ROW}px`}>
			<svg class="edges">
				{#each $graph.nodes as node (node.id)}
					{@const parent = node.parent ? byId.get(node.parent) : undefined}
					{#if parent}
						<path d={edge(parent, node)} class:head-line={onHeadLine(node)} class:undone={node.undone} />
					{/if}
					{#each node.merged as merged}
						{@const other = byId.get(merged)}
						{#if other}<path d={edge(other, node)} class="merge" />{/if}
					{/each}
				{/each}
			</svg>
			{#each $graph.nodes as node (node.id)}
				<div
					class="node"
					class:head={node.head}
					class:head-line={onHeadLine(node)}
					class:undone={node.undone}
					class:abandoned={node.abandoned}
					style:left={`${left(node)}px`}
					style:top={`${top(node)}px`}
					style:width={`${NODE_WIDTH}px`}
					style:height={`${NODE_HEIGHT}px`}
					data-head={node.head || undefined}
				>
					<button class="step" data-tooltip-label={tooltip(node)} on:click={() => choose(node)}>
						{#if node.thumbnail}<HistoryShot thumbnail={node.thumbnail} images={$thumbnails} />{/if}
						<span class="label">{node.label}</span>
					</button>
					{#if node.abandoned}
						<IconButton
							icon="HistoryRedo"
							size={16}
							tooltipLabel="Bring back"
							tooltipDescription="Do this step again on top of the current line."
							action={() => editor.bringBackHistoryInteraction(node.id)}
						/>
					{/if}
					{#if node.head}<span class="marker">Now</span>{/if}
				</div>
			{/each}
		</div>
	</div>
</div>

<style lang="scss">
	.history-graph {
		position: absolute;
		inset: 0;
		display: flex;
		flex-direction: column;
		background: var(--color-2-mildblack);

		.header {
			flex: 0 0 auto;
			display: flex;
			align-items: center;
			justify-content: space-between;
			padding: 4px 8px;
			border-bottom: 1px solid var(--color-4-dimgray);
		}

		.body {
			flex: 1 1 0;
			overflow: auto;
			position: relative;

			> .text-label {
				position: absolute;
				padding: 16px;
			}
		}

		.canvas {
			position: relative;
		}

		.edges {
			position: absolute;
			inset: 0;
			width: 100%;
			height: 100%;
			pointer-events: none;

			path {
				fill: none;
				stroke: var(--color-5-dullgray);
				stroke-width: 2;

				&.head-line {
					stroke: var(--color-e-nearwhite);
				}

				&.undone {
					stroke-dasharray: 4 3;
				}

				&.merge {
					stroke-dasharray: 2 4;
				}
			}
		}

		.node {
			position: absolute;
			box-sizing: border-box;
			display: flex;
			align-items: center;
			gap: 2px;
			padding: 0 4px 0 0;
			border-radius: 4px;
			border: 1px solid var(--color-4-dimgray);
			background: var(--color-3-darkgray);

			&.head-line {
				border-color: var(--color-8-uppergray);
			}

			&.head {
				border-color: var(--color-e-nearwhite);
				background: var(--color-4-dimgray);
			}

			&.undone,
			&.abandoned {
				.step {
					opacity: 0.6;
				}
			}

			.step {
				flex: 1 1 0;
				min-width: 0;
				height: 100%;
				display: flex;
				align-items: center;
				gap: 6px;
				padding: 0 0 0 6px;
				border: none;
				background: none;
				color: var(--color-e-nearwhite);
				text-align: left;
				cursor: pointer;

				.label {
					overflow: hidden;
					text-overflow: ellipsis;
					white-space: nowrap;
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
	}
</style>
