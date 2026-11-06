<script lang="ts">
	import type { HistoryShot } from "/wrapper/pkg/graphite_wasm_wrapper";

	export let shot: HistoryShot;
	export let url: string | undefined;
	export let large = false;
</script>

<div class="history-shot" class:large>
	{#if url}<img src={url} alt="" />{/if}
	<svg viewBox={shot.frame.join(" ")} preserveAspectRatio="xMidYMid meet">
		{#each shot.outlines as outline}
			<rect class={outline.change.toLowerCase()} x={outline.x} y={outline.y} width={outline.width} height={outline.height} vector-effect="non-scaling-stroke" />
		{/each}
	</svg>
</div>

<style lang="scss">
	.history-shot {
		flex: 0 0 auto;
		position: relative;
		width: 48px;
		height: 32px;
		border-radius: 2px;
		overflow: hidden;
		background-image: var(--color-transparent-checkered-background);
		background-size: var(--color-transparent-checkered-background-size-mini);
		background-position: var(--color-transparent-checkered-background-position-mini);
		background-repeat: var(--color-transparent-checkered-background-repeat);

		&.large {
			width: 120px;
			height: 80px;
		}

		img,
		svg {
			position: absolute;
			inset: 0;
			width: 100%;
			height: 100%;
		}

		rect {
			fill: none;
			stroke-width: 1.5px;

			&.added {
				stroke: var(--color-session-green);
			}

			&.removed {
				stroke: var(--color-error-red);
			}

			&.changed {
				stroke: var(--color-warning-yellow);
			}
		}
	}
</style>
