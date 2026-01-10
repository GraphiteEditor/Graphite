<script lang="ts">
	import type { ActionShortcut } from "@graphite/messages";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import TextButton from "@graphite/components/widgets/buttons/TextButton.svelte";

	// Content
	export let labels: string[];
	export let disabled = false;
	// Tooltips
	export let tooltipLabel: string | undefined = undefined;
	export let tooltipDescription: string | undefined = undefined;
	export let tooltipShortcut: ActionShortcut | undefined = undefined;
	// Callbacks
	export let action: (index: number) => void;
</script>

<LayoutRow class="breadcrumb-trail-buttons" {tooltipLabel} {tooltipDescription} {tooltipShortcut}>
	{#each labels as label, index}
		<TextButton {label} emphasized={index === labels.length - 1} {disabled} action={() => !disabled && index !== labels.length - 1 && action(index)} />
	{/each}
</LayoutRow>

<style lang="scss" global>
	.breadcrumb-trail-buttons {
		.text-button {
			position: relative;

&:not(:first-child) {
				margin-left: -4px;
			}

			clip-path: polygon(0% 0%, calc(100% - 4px) 0%, 100% 50%, calc(100% - 4px) 100%, 0% 100%, 4px 50%);
			padding-left: 12px;
			padding-right: 12px;

			&:first-of-type {
				clip-path: polygon(0% 0%, calc(100% - 4px) 0%, 100% 50%, calc(100% - 4px) 100%, 0% 100%);
				padding-left: 8px;
				padding-right: 12px;
			}

			&:last-of-type {
				clip-path: polygon(0% 0%, 100% 0%, 100% 100%, 0% 100%, 4px 50%);
				padding-left: 12px;
				padding-right: 8px;

				pointer-events: none;
			}

			&:first-of-type:last-of-type {
				clip-path: none;
				padding-left: 8px;
				padding-right: 8px;
			}
		}
	}
</style>
