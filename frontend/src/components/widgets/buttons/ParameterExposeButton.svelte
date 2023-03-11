<script lang="ts">
	import LayoutRow from "~/src/components/layout/LayoutRow.svelte";

	export let exposed: boolean;
	export let dataType: string;
	export let tooltip: string | undefined = undefined;
	// Callbacks
	export let action: (e?: MouseEvent) => void;
</script>

<LayoutRow class="parameter-expose-button">
	<button class:exposed style:--data-type-color={`var(--color-data-${dataType})`} on:click={action} title={tooltip} tabindex="-1" />
</LayoutRow>

<style lang="scss" global>
	.parameter-expose-button {
		display: flex;
		align-items: center;
		flex: 0 0 auto;
		max-height: 24px;

		button {
			flex: 0 0 auto;
			width: 8px;
			height: 8px;
			margin: 0;
			padding: 0;
			border: none;
			border-radius: 50%;

			&:not(.exposed) {
				background: none;
				border: 1px solid var(--data-type-color);

				&:hover {
					background: var(--color-6-lowergray);
				}
			}

			&.exposed {
				background: var(--data-type-color);

				&:hover {
					border: 1px solid var(--color-f-white);
				}
			}
		}
	}
</style>
