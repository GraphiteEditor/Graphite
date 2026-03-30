<script lang="ts">
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import TextButton from "/src/components/widgets/buttons/TextButton.svelte";
	import type { ActionShortcut } from "/wrapper/pkg/graphite_wasm_wrapper";

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

			&:not(:first-of-type) {
				border-top-left-radius: 0;
				border-bottom-left-radius: 0;

				&::before {
					content: "";
					position: absolute;
					left: -4px;
					width: 8px;
					height: 100%;
					background: var(--button-background-color);
					clip-path: polygon(8px -1px, 0 -1px, 4px 12px, 0 25px, 8px 25px);
				}
			}

			&:not(:last-of-type) {
				border-top-right-radius: 0;
				border-bottom-right-radius: 0;

				&::after {
					content: "";
					position: absolute;
					right: -4px;
					width: 8px;
					height: 100%;
					background: var(--button-background-color);
					clip-path: polygon(0 -1px, 4px -1px, 8px 12px, 4px 25px, 0 25px);
				}
			}

			&:last-of-type {
				// Make this non-functional button not change color on hover
				pointer-events: none;
			}
		}
	}
</style>
