<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import type { Color } from "@/wasm-communication/messages";

	import ColorPicker from "@/components/floating-menus/ColorPicker.svelte";
	import LayoutRow from "@/components/layout/LayoutRow.svelte";
	import TextLabel from "@/components/widgets/labels/TextLabel.svelte";

	// emits: ["update:value"],
	const dispatch = createEventDispatcher<{ value: Color }>();

	let open = false;

	export let value: Color;
	export let noTransparency = false; // TODO: Rename to allowTransparency, also implement allowNone
	export let disabled = false; // TODO: Design and implement
	export let tooltip: string | undefined = undefined;
	export let sharpRightCorners = false;

	// TODO: Implement
	$: chip = undefined;
</script>

<LayoutRow class="color-input" classes={{ "sharp-right-corners": sharpRightCorners }} {tooltip}>
	<button
		class:none={value.none}
		class:sharp-right-corners={sharpRightCorners}
		style:--chosen-color={value.toHexOptionalAlpha()}
		on:click={() => (open = true)}
		tabindex="0"
		data-floating-menu-spawner
	>
		{#if chip}
			<TextLabel class="chip" bold={true}>{chip}</TextLabel>
		{/if}
	</button>
	<ColorPicker {open} on:open={({ detail }) => (open = detail)} color={value} on:color={({ detail }) => dispatch("value", detail)} allowNone={true} />
</LayoutRow>

<style lang="scss" global>
	.color-input {
		box-sizing: border-box;
		position: relative;
		border: 1px solid var(--color-5-dullgray);
		border-radius: 2px;
		padding: 1px;

		> button {
			position: relative;
			overflow: hidden;
			border: none;
			margin: 0;
			padding: 0;
			width: 100%;
			height: 100%;
			border-radius: 1px;

			&::before {
				content: "";
				position: absolute;
				width: 100%;
				height: 100%;
				padding: 2px;
				top: -2px;
				left: -2px;
				background: linear-gradient(var(--chosen-color), var(--chosen-color)), var(--color-transparent-checkered-background);
				background-size: var(--color-transparent-checkered-background-size);
				background-position: var(--color-transparent-checkered-background-position);
			}

			&.none {
				background: var(--color-none);
				background-repeat: var(--color-none-repeat);
				background-position: var(--color-none-position);
				background-size: var(--color-none-size-24px);
				background-image: var(--color-none-image-24px);
			}

			.chip {
				position: absolute;
				bottom: -1px;
				right: 0;
				height: 13px;
				line-height: 13px;
				background: var(--color-f-white);
				color: var(--color-2-mildblack);
				border-radius: 4px 0 0 0;
				padding: 0 4px;
				font-size: 10px;
				box-shadow: 0 0 2px var(--color-3-darkgray);
			}
		}

		&.color-input.color-input > button {
			outline-offset: 0;
		}

		> .floating-menu {
			left: 50%;
			bottom: 0;
		}
	}
</style>
