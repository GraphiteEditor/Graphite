<script lang="ts">
	import type { ActionShortcut } from "@graphite/messages";

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	export let url: string;
	export let width: string | undefined;
	export let height: string | undefined;
	export let tooltipLabel: string | undefined = undefined;
	export let tooltipDescription: string | undefined = undefined;
	export let tooltipShortcut: ActionShortcut | undefined = undefined;

	$: extraClasses = Object.entries(classes)
		.flatMap(([className, stateName]) => (stateName ? [className] : []))
		.join(" ");
</script>

<img
	src={url}
	style:width
	style:height
	class={`image-label ${className} ${extraClasses}`.trim()}
	data-tooltip-label={tooltipLabel}
	data-tooltip-description={tooltipDescription}
	data-tooltip-shortcut={tooltipShortcut?.shortcut ? JSON.stringify(tooltipShortcut.shortcut) : undefined}
	alt=""
/>

<style lang="scss" global>
	.image-label {
		width: auto;
		height: auto;
		border-radius: 2px;
		background-image: var(--color-transparent-checkered-background);
		background-size: var(--color-transparent-checkered-background-size);
		background-position: var(--color-transparent-checkered-background-position);
		background-repeat: var(--color-transparent-checkered-background-repeat);

		+ .image-label.image-label {
			margin-left: 8px;
		}
	}
</style>
