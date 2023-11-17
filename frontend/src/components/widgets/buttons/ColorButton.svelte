<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import type { Color } from "@graphite/wasm-communication/messages";

	import ColorPicker from "@graphite/components/floating-menus/ColorPicker.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	// emits: ["update:value"],
	const dispatch = createEventDispatcher<{ value: Color }>();

	let open = false;

	export let value: Color;
	// TODO: Implement
	// export let allowTransparency = false;
	// export let disabled = false;
	export let allowNone = false;
	export let tooltip: string | undefined = undefined;
	export let sharpRightCorners = false;

	function colorLabel(value: Color): string {
		if (value.none) return "No Color";
		const type = "Color"; // TODO: Add "Gradient" type
		const hex = value.toHexNoAlpha();
		const alpha = value.alpha === 1 ? undefined : `${Math.floor(value.alpha * 100)}%`;
		return [type, hex, alpha].filter((x) => x).join(" — ");
	}
</script>

<LayoutCol class="color-button" classes={{ "sharp-right-corners": sharpRightCorners }} {tooltip}>
	<button
		class:none={value.none}
		class:sharp-right-corners={sharpRightCorners}
		style:--chosen-color={value.toHexOptionalAlpha()}
		on:click={() => (open = true)}
		tabindex="0"
		data-floating-menu-spawner
	></button>
	<ColorPicker
		{open}
		on:open={({ detail }) => (open = detail)}
		color={value}
		on:color={({ detail }) => {
			value = detail;
			dispatch("value", detail);
		}}
		{allowNone}
	/>
	<TextLabel>{colorLabel(value)}</TextLabel>
</LayoutCol>

<style lang="scss" global>
	.color-button {
		position: relative;
		min-width: 80px;

		> button {
			position: relative;
			overflow: hidden;
			border: none;
			margin: 0;
			padding: 0;
			width: 100%;
			height: 16px;
			// TODO: Find a way to work around Chrome's light-colored antialiasing artifacts around the rounded parts of the pill border most visible when the color is dark colored
			border: 1px solid var(--color-5-dullgray);
			border-radius: 10000px;

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
		}

		> .floating-menu {
			left: 50%;
			bottom: 0;
		}

		> .text-label {
			margin-top: 1px;
			height: 24px - 16px - 1px;
			line-height: 24px - 16px - 1px;
			font-size: 10px;
			text-align: center;
		}
	}
</style>
