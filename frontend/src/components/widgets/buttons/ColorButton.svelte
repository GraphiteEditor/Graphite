<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import type { Color } from "@graphite/wasm-communication/messages";

	import ColorPicker from "@graphite/components/floating-menus/ColorPicker.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	const dispatch = createEventDispatcher<{ value: Color; startHistoryTransaction: undefined }>();

	let open = false;

	export let value: Color;
	export let disabled = false;
	export let allowNone = false;
	// export let allowTransparency = false; // TODO: Implement
	export let tooltip: string | undefined = undefined;
</script>

<LayoutCol class="color-button" classes={{ disabled, none: value.none, open }} {tooltip}>
	<button {disabled} style:--chosen-color={value.toHexOptionalAlpha()} on:click={() => (open = true)} tabindex="0" data-floating-menu-spawner></button>
	{#if disabled && !value.none}
		<TextLabel>sRGB</TextLabel>
	{/if}
	<ColorPicker
		{open}
		on:open={({ detail }) => (open = detail)}
		color={value}
		on:color={({ detail }) => {
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
			background: linear-gradient(var(--chosen-color), var(--chosen-color)), var(--color-transparent-checkered-background);
			background-size: var(--color-transparent-checkered-background-size);
			background-position: var(--color-transparent-checkered-background-position);
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
