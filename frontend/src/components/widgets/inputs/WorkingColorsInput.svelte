<script lang="ts">
	import { getContext } from "svelte";

	import type { Editor } from "@graphite/editor";
	import { Color } from "@graphite/messages";

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
		editor.handle.updatePrimaryColor(color.red, color.green, color.blue, color.alpha);
	}

	function secondaryColorChanged(color: Color) {
		editor.handle.updateSecondaryColor(color.red, color.green, color.blue, color.alpha);
	}
</script>

<LayoutCol class="working-colors-button">
	<LayoutRow class="primary swatch">
		<button on:click={clickPrimarySwatch} class:open={primaryOpen} style:--swatch-color={primary.toRgbaCSS()} data-floating-menu-spawner="no-hover-transfer" tabindex="0"></button>
		<ColorPicker
			open={primaryOpen}
			on:open={({ detail }) => (primaryOpen = detail)}
			colorOrGradient={primary}
			on:colorOrGradient={({ detail }) => detail instanceof Color && primaryColorChanged(detail)}
			direction="Right"
		/>
	</LayoutRow>
	<LayoutRow class="secondary swatch">
		<button on:click={clickSecondarySwatch} class:open={secondaryOpen} style:--swatch-color={secondary.toRgbaCSS()} data-floating-menu-spawner="no-hover-transfer" tabindex="0"></button>
		<ColorPicker
			open={secondaryOpen}
			on:open={({ detail }) => (secondaryOpen = detail)}
			colorOrGradient={secondary}
			on:colorOrGradient={({ detail }) => detail instanceof Color && secondaryColorChanged(detail)}
			direction="Right"
		/>
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
				--ring-color: var(--color-5-dullgray);
				width: 100%;
				height: 100%;
				margin: 0;
				padding: 0;
				border: none;
				outline: none;
				border-radius: 50%;
				box-sizing: border-box;
				overflow: hidden;
				position: relative;
				// Color of the panel background, used to extend outside the ring and appear to cut out a crescent from the lower circle (by covering it up with the panel background color)
				box-shadow: 0 0 0 2px var(--color-3-darkgray);
				background: var(--color-3-darkgray);

				// Main color and checked transparency pattern (inset by 1px to begin inside/below the ring to avoid antialiasing artifacts)
				&::before {
					content: "";
					position: absolute;
					top: 1px;
					bottom: 1px;
					left: 1px;
					right: 1px;
					border-radius: 50%;
					background: linear-gradient(var(--swatch-color), var(--swatch-color)), var(--color-transparent-checkered-background);
					background-size:
						100% 100%,
						var(--color-transparent-checkered-background-size);
					background-position:
						0 0,
						var(--color-transparent-checkered-background-position-plus-one);
					background-repeat: no-repeat, var(--color-transparent-checkered-background-repeat);
				}

				// Gray ring outline
				&::after {
					content: "";
					position: absolute;
					top: 0;
					bottom: 0;
					left: 0;
					right: 0;
					border-radius: 50%;
					box-shadow: inset 0 0 0 2px var(--ring-color);
				}

				&:hover,
				&.open {
					--ring-color: var(--color-6-lowergray);
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
