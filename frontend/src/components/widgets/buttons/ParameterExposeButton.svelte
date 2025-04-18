<script lang="ts">
	import type { FrontendGraphDataType } from "@graphite/messages";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";

	export let exposed: boolean;
	export let dataType: FrontendGraphDataType;
	export let tooltip: string | undefined = undefined;
	// Callbacks
	export let action: (e?: MouseEvent) => void;
</script>

<LayoutRow class="parameter-expose-button">
	<button
		class:exposed
		style:--data-type-color={`var(--color-data-${dataType.toLowerCase()})`}
		style:--data-type-color-dim={`var(--color-data-${dataType.toLowerCase()}-dim)`}
		on:click={action}
		title={tooltip}
		tabindex="-1"
	>
		{#if !exposed}
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8">
				<circle class="interior" r="3" cx="4" cy="4" />
				<path
					class="outline"
					d="M4,1C5.656,1 7,2.344 7,4C7,5.656 5.656,7 4,7C2.344,7 1,5.656 1,4C1,2.344 2.344,1 4,1ZM4,2C2.896,2 2,2.896 2,4C2,5.104 2.896,6 4,6C5.104,6 6,5.104 6,4C6,2.896 5.104,2 4,2z"
				/>
			</svg>
		{:else}
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8">
				<path class="interior" d="M0,6.306L0,1.694C0,0.228 1.06,-0.41 2.356,0.276L7.028,2.752C8.324,3.438 8.324,4.562 7.028,5.248L2.356,7.723C1.06,8.41 0,7.771 0,6.306z" />
				<path
					class="outline"
					d="M1.364,8C0.56,8 0,7.372 0,6.306L0,1.694C0,0.228 1.06,-0.41 2.356,0.276L7.028,2.752C8.324,3.439 8.324,4.561 7.028,5.248L2.356,7.724C2.002,7.911 1.666,8 1.364,8ZM1.364,7.2C1.542,7.2 1.756,7.137 1.981,7.017L6.653,4.541C7.031,4.341 7.2,4.125 7.2,4C7.2,3.875 7.031,3.659 6.653,3.459L1.982,0.983C1.756,0.863 1.542,0.8 1.364,0.8C0.873,0.8 0.8,1.36 0.8,1.694L0.8,6.306C0.8,6.64 0.873,7.2 1.364,7.2z"
				/>
			</svg>
		{/if}
	</button>
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
			background: none;
			fill: none;
			stroke: none;

			.outline {
				fill: none;
			}

			.interior {
				fill: var(--data-type-color);
			}

			&:hover {
				.outline {
					fill: var(--data-type-color);
				}

				.interior {
					fill: var(--data-type-color-dim);
				}
			}
		}
	}
</style>
