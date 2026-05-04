<script lang="ts">
	import { createEventDispatcher } from "svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import IconButton from "/src/components/widgets/buttons/IconButton.svelte";
	import Separator from "/src/components/widgets/labels/Separator.svelte";
	import { createColor } from "/src/utility-functions/colors";
	import type { Color } from "/wrapper/pkg/graphite_wasm_wrapper";

	type PresetColor = "Black" | "White" | "Red" | "Yellow" | "Green" | "Cyan" | "Blue" | "Magenta";

	const PURE_COLORS: Record<PresetColor, [number, number, number]> = {
		Black: [0, 0, 0],
		White: [1, 1, 1],
		Red: [1, 0, 0],
		Yellow: [1, 1, 0],
		Green: [0, 1, 0],
		Cyan: [0, 1, 1],
		Blue: [0, 0, 1],
		Magenta: [1, 0, 1],
	};
	const PURE_COLORS_GRAYABLE: [PresetColor, string, string][] = [
		["Red", "#ff0000", "#4c4c4c"],
		["Yellow", "#ffff00", "#e3e3e3"],
		["Green", "#00ff00", "#969696"],
		["Cyan", "#00ffff", "#b2b2b2"],
		["Blue", "#0000ff", "#1c1c1c"],
		["Magenta", "#ff00ff", "#696969"],
	];

	const dispatch = createEventDispatcher<{
		preset: Color | "None";
		eyedropperColorCode: string;
	}>();

	export let disabled = false;
	export let showNoneOption = false;

	function pickPreset(preset: PresetColor | "None") {
		if (disabled) return;
		dispatch("preset", preset === "None" ? "None" : createColor(...PURE_COLORS[preset], 1));
	}

	// TODO: Replace this temporary usage of the browser eyedropper API, that only works in Chromium-based browsers, with the custom color sampler system used by the Eyedropper tool
	function eyedropperSupported(): boolean {
		// TODO: Implement support in the desktop app for OS-level color picking
		if (import.meta.env.MODE === "native") return false;

		return window.EyeDropper !== undefined;
	}

	async function activateEyedropperSample() {
		if (!eyedropperSupported()) return;

		try {
			const result = await new EyeDropper().open();
			dispatch("eyedropperColorCode", result.sRGBHex);
		} catch {
			// Do nothing
		}
	}
</script>

<LayoutRow class="color-presets-input" classes={{ disabled }}>
	{#if showNoneOption}
		<button
			class="preset-color none"
			{disabled}
			on:click={() => pickPreset("None")}
			data-tooltip-label="Set to No Color"
			data-tooltip-description={disabled ? "Disabled (read-only)." : ""}
			tabindex="0"
		></button>
		<Separator style="Related" />
	{/if}
	<button class="preset-color black" {disabled} on:click={() => pickPreset("Black")} data-tooltip-label="Set to Black" data-tooltip-description={disabled ? "Disabled (read-only)." : ""} tabindex="0"
	></button>
	<Separator style="Related" />
	<button class="preset-color white" {disabled} on:click={() => pickPreset("White")} data-tooltip-label="Set to White" data-tooltip-description={disabled ? "Disabled (read-only)." : ""} tabindex="0"
	></button>
	<Separator style="Related" />
	<button class="preset-color pure" {disabled} tabindex="-1">
		{#each PURE_COLORS_GRAYABLE as [preset, color, gray]}
			<div
				on:click={() => pickPreset(preset)}
				style:--pure-color={color}
				style:--pure-color-gray={gray}
				data-tooltip-label={`Set to ${preset}`}
				data-tooltip-description={disabled ? "Disabled (read-only)." : ""}
			></div>
		{/each}
	</button>
	{#if eyedropperSupported()}
		<Separator style="Related" />
		<IconButton icon="Eyedropper" size={24} {disabled} action={activateEyedropperSample} tooltipLabel="Eyedropper" tooltipDescription="Sample a pixel color from the document." />
	{/if}
</LayoutRow>

<style lang="scss">
	.color-presets-input {
		flex: 0 0 auto;
		width: 100%;

		.preset-color {
			border: none;
			margin: 0;
			padding: 0;
			border-radius: 2px;
			height: 24px;
			flex: 1 1 100%;

			&.none {
				background: var(--color-none);
				background-repeat: var(--color-none-repeat);
				background-position: var(--color-none-position);
				background-size: var(--color-none-size-24px);
				background-image: var(--color-none-image-24px);

				&,
				& ~ .black,
				& ~ .white {
					width: 48px;
				}
			}

			&.black {
				background: black;
			}

			&.white {
				background: white;
			}

			&.pure {
				width: 24px;
				font-size: 0;
				overflow: hidden;
				flex: 0 0 auto;

				div {
					display: inline-block;
					width: calc(100% / 3);
					height: 50%;
					// For the least jarring luminance conversion, these colors are derived by placing a black layer with the "desaturate" blend mode over the colors.
					// We don't use the CSS `filter: grayscale(1);` property because it produces overly dark tones for bright colors with a noticeable jump on hover.
					background: var(--pure-color-gray);
					transition: background-color 0.1s;
				}

				&:hover div {
					background: var(--pure-color);
				}
			}
		}

		&.disabled {
			.preset-color {
				transition: opacity 0.1s;

				&:hover {
					opacity: 0.5;
				}
			}

			.preset-color.pure:hover div {
				background: var(--pure-color-gray);
			}
		}
	}
</style>
