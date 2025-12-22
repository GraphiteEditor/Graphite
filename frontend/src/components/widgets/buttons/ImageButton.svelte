<script lang="ts">
	import type { ActionShortcut } from "@graphite/messages";
	import { IMAGE_BASE64_STRINGS } from "@graphite/utility-functions/images";

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	export let image: string;
	export let width: string | undefined;
	export let height: string | undefined;
	export let tooltipLabel: string | undefined = undefined;
	export let tooltipDescription: string | undefined = undefined;
	export let tooltipShortcut: ActionShortcut | undefined = undefined;
	// Callbacks
	export let action: (e?: MouseEvent) => void;

	$: extraClasses = Object.entries(classes)
		.flatMap(([className, stateName]) => (stateName ? [className] : []))
		.join(" ");
</script>

<img
	src={IMAGE_BASE64_STRINGS[image]}
	style:width
	style:height
	class={`image-button ${className} ${extraClasses}`.trim()}
	data-tooltip-label={tooltipLabel}
	data-tooltip-description={tooltipDescription}
	data-tooltip-shortcut={tooltipShortcut?.shortcut ? JSON.stringify(tooltipShortcut.shortcut) : undefined}
	alt=""
	on:click={action}
/>

<style lang="scss" global>
	.image-button {
		width: auto;
		height: auto;
		border-radius: 2px;

		+ .image-button.image-button {
			margin-left: 8px;
		}
	}
</style>
