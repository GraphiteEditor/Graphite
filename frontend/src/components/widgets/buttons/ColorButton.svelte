<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import type { FillColorChoice } from "@graphite/wasm-communication/messages";
	import { Color, Gradient } from "@graphite/wasm-communication/messages";

	import ColorPicker from "@graphite/components/floating-menus/ColorPicker.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	const dispatch = createEventDispatcher<{ value: FillColorChoice; startHistoryTransaction: undefined }>();

	let open = false;

	export let value: FillColorChoice;
	export let disabled = false;
	export let allowNone = false;
	// export let allowTransparency = false; // TODO: Implement
	export let tooltip: string | undefined = undefined;

	$: chosenGradient = value instanceof Gradient ? value.toLinearGradientCSS() : `linear-gradient(${value.toHexOptionalAlpha()}, ${value.toHexOptionalAlpha()})`;
</script>

<LayoutCol class="color-button" classes={{ disabled, none: value instanceof Color ? value.none : false, open }} {tooltip}>
	<button {disabled} style:--chosen-gradient={chosenGradient} on:click={() => (open = true)} tabindex="0" data-floating-menu-spawner></button>
	{#if disabled && value instanceof Color && !value.none}
		<TextLabel>sRGB</TextLabel>
	{/if}
	<ColorPicker
		{open}
		on:open={({ detail }) => (open = detail)}
		colorOrGradient={value}
		on:colorOrGradient={({ detail }) => {
			value = detail;
			dispatch("value", detail);
		}}
		on:startHistoryTransaction={() => {
			// This event is sent to the backend so it knows to start a transaction for the history system. See discussion for some explanation:
			// <https://github.com/GraphiteEditor/Graphite/pull/1584#discussion_r1477592483>
			dispatch("startHistoryTransaction");
		}}
		{allowNone}
	/>
</LayoutCol>

<style lang="scss" global>
	.color-button {
		position: relative;
		min-width: 80px;
		border-radius: 2px;
		background: var(--color-5-dullgray);

		&:hover,
		&.open {
			&,
			> .text-label {
				background: rgba(var(--color-6-lowergray-rgb), 50%);
			}
		}

		&.disabled {
			&,
			> .text-label {
				background: var(--color-4-dimgray);
				color: var(--color-8-uppergray);
			}
		}

		> button {
			border: none;
			padding: 0;
			margin: 0;
			margin-left: 2px;
			margin-top: 2px;
			width: calc(100% - 4px);
			height: calc(100% - 4px);
			background-image: var(--chosen-gradient), var(--color-transparent-checkered-background);
			background-size:
				100% 100%,
				var(--color-transparent-checkered-background-size);
			background-position:
				0 0,
				var(--color-transparent-checkered-background-position);
			background-repeat: no-repeat, var(--color-transparent-checkered-background-repeat);
		}

		&.none {
			> button {
				background: var(--color-none);
				background-repeat: var(--color-none-repeat);
				background-position: var(--color-none-position);
				background-size: var(--color-none-size-24px);
				background-image: var(--color-none-image-24px);
			}

			&.disabled {
				> button::after {
					content: "";
					position: absolute;
					top: 0;
					bottom: 0;
					left: 0;
					right: 0;
					background: rgba(var(--color-4-dimgray-rgb), 50%);
				}
			}
		}

		> .text-label {
			background: rgba(var(--color-5-dullgray-rgb), 50%);
			font-size: 10px;
			line-height: 12px;
			height: 12px;
			border-radius: 6px 0 0 6px;
			padding-right: 2px;
			padding-left: 4px;
			margin: auto;
			position: absolute;
			right: 0;
			top: 0;
			bottom: 0;
		}

		> .floating-menu {
			left: 50%;
			bottom: 0;
		}
	}
</style>
