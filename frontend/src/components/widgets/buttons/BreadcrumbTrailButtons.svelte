<script lang="ts">
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import TextButton from "@graphite/components/widgets/buttons/TextButton.svelte";

	export let labels: string[];
	export let disabled = false;
	export let tooltip: string | undefined = undefined;
	// Callbacks
	export let action: (index: number) => void;
</script>

<LayoutRow class="breadcrumb-trail-buttons" {tooltip}>
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
					top: 0;
					left: -4px;
					width: 0;
					height: 0;
					border-style: solid;
					border-width: 12px 0 12px 4px;
					border-color: var(--button-background-color) var(--button-background-color) var(--button-background-color) transparent;
				}
			}

			&:not(:last-of-type) {
				border-top-right-radius: 0;
				border-bottom-right-radius: 0;

				&::after {
					content: "";
					position: absolute;
					top: 0;
					right: -4px;
					width: 0;
					height: 0;
					border-style: solid;
					border-width: 12px 0 12px 4px;
					border-color: transparent transparent transparent var(--button-background-color);
				}
			}

			&:last-of-type {
				// Make this non-functional button not change color on hover
				pointer-events: none;
			}
		}
	}
</style>
