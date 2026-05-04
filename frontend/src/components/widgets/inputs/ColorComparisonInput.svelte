<script lang="ts">
	import { createEventDispatcher } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import IconButton from "/src/components/widgets/buttons/IconButton.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import { colorContrastingColor, colorEquals, colorToHexOptionalAlpha, contrastingOutlineFactor } from "/src/utility-functions/colors";
	import type { Color } from "/wrapper/pkg/graphite_wasm_wrapper";

	const dispatch = createEventDispatcher<{ swap: undefined }>();

	export let newColor: Color | undefined;
	export let oldColor: Color | undefined;
	export let isNone: boolean;
	export let oldIsNone: boolean;
	export let disabled = false;

	$: differs = !colorEquals(newColor, oldColor);
	$: outlineFactor = Math.max(
		contrastingOutlineFactor(newColor ? { Solid: newColor } : ("None" as const), "--color-2-mildblack", 0.01),
		contrastingOutlineFactor(oldColor ? { Solid: oldColor } : ("None" as const), "--color-2-mildblack", 0.01),
	);
	$: outlined = outlineFactor > 0.0001;
	$: transparency = (newColor?.alpha ?? 1) < 1 || (oldColor?.alpha ?? 1) < 1;
</script>

<LayoutRow
	class="color-comparison-input"
	classes={{ outlined, transparency, disabled }}
	styles={{
		"--outline-amount": outlineFactor,
		"--new-color": newColor ? colorToHexOptionalAlpha(newColor) : undefined,
		"--new-color-contrasting": colorContrastingColor(newColor),
		"--old-color": oldColor ? colorToHexOptionalAlpha(oldColor) : undefined,
		"--old-color-contrasting": colorContrastingColor(oldColor),
	}}
	tooltipDescription={differs ? "Comparison between the present color choice (left) and the color before it was changed (right)." : "The present color choice."}
>
	{#if differs && !disabled}
		<div class="swap-button-background"></div>
		<IconButton class="swap-button" icon="SwapHorizontal" size={16} action={() => dispatch("swap")} tooltipLabel="Swap" />
	{/if}
	<LayoutCol class="new-color" classes={{ none: isNone }}>
		{#if differs}
			<TextLabel>New</TextLabel>
		{/if}
	</LayoutCol>
	{#if differs}
		<LayoutCol class="old-color" classes={{ none: oldIsNone }}>
			<TextLabel>Old</TextLabel>
		</LayoutCol>
	{/if}
</LayoutRow>

<style lang="scss">
	.color-comparison-input {
		flex: 0 0 auto;
		width: 100%;
		height: 32px;
		border-radius: 2px;
		box-sizing: border-box;
		overflow: hidden;
		position: relative;

		&.outlined::after {
			content: "";
			pointer-events: none;
			position: absolute;
			top: 0;
			bottom: 0;
			left: 0;
			right: 0;
			box-shadow: inset 0 0 0 1px rgba(var(--color-0-black-rgb), var(--outline-amount));
		}

		&.transparency {
			background-image: var(--color-transparent-checkered-background);
			background-size: var(--color-transparent-checkered-background-size);
			background-position: var(--color-transparent-checkered-background-position);
			background-repeat: var(--color-transparent-checkered-background-repeat);
		}

		.swap-button-background {
			overflow: hidden;
			position: absolute;
			mix-blend-mode: multiply;
			opacity: 0.25;
			border-radius: 2px;
			width: 16px;
			height: 16px;
			top: 50%;
			left: 50%;
			transform: translate(-50%, -50%);

			&::before,
			&::after {
				content: "";
				position: absolute;
				width: 50%;
				height: 100%;
			}

			&::before {
				left: 0;
				background: var(--new-color-contrasting);
			}

			&::after {
				right: 0;
				background: var(--old-color-contrasting);
			}
		}

		.swap-button {
			position: absolute;
			transform: translate(-50%, -50%);
			top: 50%;
			left: 50%;
		}

		.new-color {
			background: var(--new-color);

			.text-label {
				text-align: left;
				margin: 2px 8px;
				color: var(--new-color-contrasting);
			}
		}

		.old-color {
			background: var(--old-color);

			.text-label {
				text-align: right;
				margin: 2px 8px;
				color: var(--old-color-contrasting);
			}
		}

		.new-color,
		.old-color {
			width: 50%;
			height: 100%;

			&.none {
				background: var(--color-none);
				background-repeat: var(--color-none-repeat);
				background-position: var(--color-none-position);
				background-size: var(--color-none-size-32px);
				background-image: var(--color-none-image-32px);

				.text-label {
					// Many stacked white shadows helps to increase the opacity and approximate shadow spread which does not exist for text shadows
					text-shadow:
						0 0 4px white,
						0 0 4px white,
						0 0 4px white,
						0 0 4px white,
						0 0 4px white,
						0 0 4px white,
						0 0 4px white,
						0 0 4px white,
						0 0 4px white,
						0 0 4px white;
				}
			}
		}

		&.disabled {
			transition: opacity 0.1s;

			&:hover {
				opacity: 0.5;
			}
		}
	}
</style>
