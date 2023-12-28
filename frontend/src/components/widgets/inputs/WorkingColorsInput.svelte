<script lang="ts">
	import { getContext } from "svelte";

	import type { Editor } from "@graphite/wasm-communication/editor";
	import type { Color } from "@graphite/wasm-communication/messages";

	import ColorPicker from "@graphite/components/floating-menus/ColorPicker.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";

	const editor = getContext<Editor>("editor");

	export let primary: Color;
	export let secondary: Color;

	let primaryOpen = false;
	let secondaryOpen = false;

	function clickPrimarySwatch() {
		primaryOpen = true;
		secondaryOpen = false;
	}

	function clickSecondarySwatch() {
		primaryOpen = false;
		secondaryOpen = true;
	}

	function primaryColorChanged(color: Color) {
		editor.instance.updatePrimaryColor(color.red, color.green, color.blue, color.alpha);
	}

	function secondaryColorChanged(color: Color) {
		editor.instance.updateSecondaryColor(color.red, color.green, color.blue, color.alpha);
	}
</script>

<LayoutCol class="working-colors-button">
	<LayoutRow class="primary swatch">
		<button on:click={clickPrimarySwatch} class:open={primaryOpen} style:--swatch-color={primary.toRgbaCSS()} data-floating-menu-spawner="no-hover-transfer" tabindex="0" />
		<ColorPicker open={primaryOpen} on:open={({ detail }) => (primaryOpen = detail)} color={primary} on:color={({ detail }) => primaryColorChanged(detail)} direction="Right" />
	</LayoutRow>
	<LayoutRow class="secondary swatch">
		<button on:click={clickSecondarySwatch} class:open={secondaryOpen} style:--swatch-color={secondary.toRgbaCSS()} data-floating-menu-spawner="no-hover-transfer" tabindex="0" />
		<ColorPicker open={secondaryOpen} on:open={({ detail }) => (secondaryOpen = detail)} color={secondary} on:color={({ detail }) => secondaryColorChanged(detail)} direction="Right" />
	</LayoutRow>
</LayoutCol>

<style lang="scss" global>
	.working-colors-button {
		flex: 0 0 auto;

		.swatch {
			width: 28px;
			height: 28px;
			margin: 0 2px;
			position: relative;

			> button {
				--swatch-color: #ffffff;
				width: 100%;
				height: 100%;
				border-radius: 50%;
				border: 2px var(--color-5-dullgray) solid;
				box-shadow: 0 0 0 2px var(--color-3-darkgray);
				margin: 0;
				padding: 0;
				box-sizing: border-box;
				background: linear-gradient(var(--swatch-color), var(--swatch-color)), var(--color-transparent-checkered-background);
				background-size: var(--color-transparent-checkered-background-size);
				background-position: var(--color-transparent-checkered-background-position);
				overflow: hidden;

				&:hover,
				&.open {
					border-color: var(--color-6-lowergray);
				}
			}

			.floating-menu {
				top: 50%;
				right: -2px;
			}

			&.primary {
				margin-bottom: -8px;
				z-index: 1;
			}
		}
	}
</style>
