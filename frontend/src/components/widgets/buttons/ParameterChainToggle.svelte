<script lang="ts">
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import type { ActionShortcut } from "/wrapper/pkg/graphite_wasm_wrapper";

	// Content
	export let name: string;
	export let expanded: boolean;
	// Tooltips
	export let tooltipLabel: string | undefined = undefined;
	export let tooltipDescription: string | undefined = undefined;
	export let tooltipShortcut: ActionShortcut | undefined = undefined;
	// Callbacks
	export let action: () => void;
</script>

<!-- Only the name is underlined, not the marker of whether its input's chain is shown -->
<TextLabel class="parameter-chain-toggle" classes={{ expanded }} {tooltipLabel} {tooltipDescription} {tooltipShortcut} on:click={action}>
	<span class="name">{name}</span>
	<span class="marker">{expanded ? " [+]" : " [−]"}</span>
</TextLabel>

<style lang="scss">
	.text-label.parameter-chain-toggle {
		display: flex;
		cursor: pointer;

		// Shrinks to an ellipsis when space runs out, so the marker after it isn't cut off
		.name {
			min-width: 0;
			overflow: hidden;
			text-overflow: ellipsis;
			text-decoration: underline;
		}

		// Keeping its leading space
		.marker {
			flex: 0 0 auto;
			white-space: pre;
		}
	}
</style>
