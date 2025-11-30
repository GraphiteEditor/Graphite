<script lang="ts">
	import { getContext } from "svelte";

	import type { TooltipState } from "@graphite/state-providers/tooltip";

	import FloatingMenu from "@graphite/components/layout/FloatingMenu.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	const tooltip = getContext<TooltipState>("tooltip");

	let self: FloatingMenu | undefined;
</script>

<div class="tooltip" style:top={`${$tooltip.position.y}px`} style:left={`${$tooltip.position.x}px`}>
	<FloatingMenu open={true} type="Tooltip" direction="Bottom" bind:this={self}>
		{@const text = $tooltip.element?.getAttribute("data-tooltip")}
		{#if text}
			<TextLabel>{text}</TextLabel>
		{/if}
	</FloatingMenu>
</div>

<style lang="scss" global>
	.tooltip {
		position: absolute;
		pointer-events: none;
		width: 0;
		height: 0;

		.floating-menu-content {
			max-width: Min(400px, 50vw);

			.text-label {
				white-space: pre-wrap;
			}
		}
	}
</style>
