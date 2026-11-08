<script lang="ts">
	import type { ActionShortcut } from "/wrapper/pkg/graphite_wasm_wrapper";

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	// Content
	export let fraction: number;
	export let label = "";
	// Tooltips
	export let tooltipLabel: string | undefined = undefined;
	export let tooltipDescription: string | undefined = undefined;
	export let tooltipShortcut: ActionShortcut | undefined = undefined;

	$: extraClasses = Object.entries(classes)
		.flatMap(([className, stateName]) => (stateName ? [className] : []))
		.join(" ");
	$: percent = `${Math.min(Math.max(fraction, 0), 1) * 100}%`;
</script>

<div
	class={`progress-bar ${className} ${extraClasses}`.trim()}
	role="progressbar"
	aria-valuemin={0}
	aria-valuemax={1}
	aria-valuenow={fraction}
	data-tooltip-label={tooltipLabel}
	data-tooltip-description={tooltipDescription}
	data-tooltip-shortcut={tooltipShortcut?.shortcut ? JSON.stringify(tooltipShortcut.shortcut) : undefined}
>
	<div class="track">
		<div class="fill" style:width={percent}></div>
	</div>
	{#if label}
		<span class="label">{label}</span>
	{/if}
</div>

<style lang="scss">
	.progress-bar {
		display: flex;
		align-items: center;
		gap: 8px;
		flex: 1 1 100%;
		height: 24px;

		.track {
			position: relative;
			flex: 1 1 100%;
			height: 4px;
			border-radius: 2px;
			overflow: hidden;
			background: var(--color-1-nearblack);
		}

		.fill {
			position: absolute;
			inset: 0 auto 0 0;
			background: var(--color-e-nearwhite);
			transition: width 0.1s linear;
		}

		.label {
			flex: 0 0 auto;
			font-size: 12px;
			white-space: nowrap;
			color: var(--color-a-softgray);
		}
	}
</style>
