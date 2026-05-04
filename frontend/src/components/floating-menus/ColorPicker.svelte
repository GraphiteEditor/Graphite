<script lang="ts">
	import { createEventDispatcher, tick } from "svelte";
	import FloatingMenu from "/src/components/layout/FloatingMenu.svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import IconButton from "/src/components/widgets/buttons/IconButton.svelte";
	import NumberInput from "/src/components/widgets/inputs/NumberInput.svelte";
	import SpectrumInput, { MAX_MIDPOINT, MIN_MIDPOINT } from "/src/components/widgets/inputs/SpectrumInput.svelte";
	import TextInput from "/src/components/widgets/inputs/TextInput.svelte";
	import VisualColorPickersInput from "/src/components/widgets/inputs/VisualColorPickersInput.svelte";
	import Separator from "/src/components/widgets/labels/Separator.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import type { HSV, RGB } from "/src/utility-functions/colors";
	import {
		contrastingOutlineFactor,
		fillChoiceColor,
		fillChoiceGradientStops,
		createColor,
		createColorFromHSVA,
		colorFromCSS,
		colorToRgb255,
		colorToHSV,
		colorToHexOptionalAlpha,
		colorToHexNoAlpha,
		colorToRgbCSS,
		colorContrastingColor,
		colorOpaque,
		colorEquals,
		gradientFirstColor,
	} from "/src/utility-functions/colors";
	import type { FillChoice, MenuDirection, Color } from "/wrapper/pkg/graphite_wasm_wrapper";

	type PresetColors = "None" | "Black" | "White" | "Red" | "Yellow" | "Green" | "Cyan" | "Blue" | "Magenta";

	const PURE_COLORS: Record<PresetColors, [number, number, number]> = {
		None: [0, 0, 0],
		Black: [0, 0, 0],
		White: [1, 1, 1],
		Red: [1, 0, 0],
		Yellow: [1, 1, 0],
		Green: [0, 1, 0],
		Cyan: [0, 1, 1],
		Blue: [0, 0, 1],
		Magenta: [1, 0, 1],
	};
	const PURE_COLORS_GRAYABLE: [PresetColors, string, string][] = [
		["Red", "#ff0000", "#4c4c4c"],
		["Yellow", "#ffff00", "#e3e3e3"],
		["Green", "#00ff00", "#969696"],
		["Cyan", "#00ffff", "#b2b2b2"],
		["Blue", "#0000ff", "#1c1c1c"],
		["Magenta", "#ff00ff", "#696969"],
	];

	const dispatch = createEventDispatcher<{ colorOrGradient: FillChoice; startHistoryTransaction: undefined; commitHistoryTransaction: undefined }>();

	export let colorOrGradient: FillChoice;
	export let allowNone = false;
	// export let allowTransparency = false; // TODO: Implement
	export let disabled = false;
	export let direction: MenuDirection = "Bottom";
	// TODO: See if this should be made to follow the pattern of DropdownInput.svelte so this could be removed
	export let open: boolean;

	const initSolidColor = fillChoiceColor(colorOrGradient);
	const initGradientStops = fillChoiceGradientStops(colorOrGradient);
	const colorForHSVA = initSolidColor || (initGradientStops ? gradientFirstColor(initGradientStops) : undefined);
	const hsvOrNone = colorForHSVA ? colorToHSV(colorForHSVA) : undefined;
	const hsv = hsvOrNone || { h: 0, s: 0, v: 0 };

	// Gradient color stops
	$: gradient = fillChoiceGradientStops(colorOrGradient);
	let activeIndex: number | undefined = 0;
	let activeIndexIsMidpoint = false;
	$: selectedGradientColor = (activeIndex !== undefined && gradient?.color[activeIndex]) || colorFromCSS("black") || createColor(0, 0, 0, 1);
	// Currently viewed color
	$: color = fillChoiceColor(colorOrGradient) || selectedGradientColor;
	// New color components
	let hue = hsv.h;
	let saturation = hsv.s;
	let value = hsv.v;
	let alpha = colorForHSVA ? colorForHSVA.alpha : 1;
	let isNone = hsvOrNone === undefined;
	// Old color components
	let oldHue = hsv.h;
	let oldSaturation = hsv.s;
	let oldValue = hsv.v;
	let oldAlpha = colorForHSVA ? colorForHSVA.alpha : 1;
	let oldIsNone = hsvOrNone === undefined;
	// Transient state
	let strayCloses = true;
	let gradientSpectrumDragging = false;

	let self: FloatingMenu | undefined;
	let hexCodeInputWidget: TextInput | undefined;
	let gradientSpectrumInputWidget: SpectrumInput | undefined;

	$: watchOpen(open);
	$: watchColor(color);

	$: oldColor = oldIsNone ? undefined : createColorFromHSVA(oldHue, oldSaturation, oldValue, oldAlpha);
	$: newColor = isNone ? undefined : createColorFromHSVA(hue, saturation, value, alpha);
	$: rgbChannels = ((): [keyof RGB, number | undefined][] => {
		const rgb = newColor ? colorToRgb255(newColor) : undefined;
		return [
			["r", rgb?.r],
			["g", rgb?.g],
			["b", rgb?.b],
		];
	})();
	$: hsvChannels = ((): [keyof HSV, number | undefined][] => {
		return [
			["h", isNone ? undefined : hue * 360],
			["s", isNone ? undefined : saturation * 100],
			["v", isNone ? undefined : value * 100],
		];
	})();
	$: opaqueHueColor = createColorFromHSVA(hue, 1, 1, 1);
	$: outlineFactor = Math.max(
		contrastingOutlineFactor(newColor ? { Solid: newColor } : ("None" as const), "--color-2-mildblack", 0.01),
		contrastingOutlineFactor(oldColor ? { Solid: oldColor } : ("None" as const), "--color-2-mildblack", 0.01),
	);
	$: outlined = outlineFactor > 0.0001;
	$: transparency = (newColor?.alpha ?? 1) < 1 || (oldColor?.alpha ?? 1) < 1;

	async function watchOpen(open: boolean) {
		if (open) {
			setTimeout(() => hexCodeInputWidget?.focus(), 0);

			await tick();
			setOldHSVA(hue, saturation, value, alpha, isNone);
		}
	}

	function watchColor(color: Color) {
		const hsv = colorToHSV(color);

		// Update the hue, but only if it is necessary so we don't:
		// - ...jump the user's hue from 360° (top) to the equivalent 0° (bottom)
		// - ...reset the hue to 0° if the color is fully desaturated, where all hues are equivalent
		// - ...reset the hue to 0° if the color's value is black, where all hues are equivalent
		if (!(hsv.h === 0 && hue === 1) && hsv.s > 0 && hsv.v > 0) hue = hsv.h;
		// Update the saturation, but only if it is necessary so we don't:
		// - ...reset the saturation to the left if the color's value is black along the bottom edge, where all saturations are equivalent
		if (hsv.v !== 0) saturation = hsv.s;
		// Update the value
		value = hsv.v;
		// Update the alpha
		alpha = color.alpha;
		// Update the status of this not being a color
		isNone = false;
	}

	function onVisualUpdate({ detail }: CustomEvent<{ hue: number; saturation: number; value: number; alpha: number }>) {
		hue = detail.hue;
		saturation = detail.saturation;
		value = detail.value;
		alpha = detail.alpha;

		const color = createColorFromHSVA(hue, saturation, value, alpha);
		setColor(color);
	}

	function setColor(color?: Color | "None") {
		if (color === "None") {
			dispatch("colorOrGradient", "None");
			return;
		}

		const colorToEmit = color || createColorFromHSVA(hue, saturation, value, alpha);

		if (gradientSpectrumInputWidget && activeIndex !== undefined && gradient && gradient.position[activeIndex] !== undefined) {
			const gradientStops = fillChoiceGradientStops(colorOrGradient);
			if (gradientStops) gradientStops.color[activeIndex] = colorToEmit;
		}

		dispatch("colorOrGradient", gradient ? { Gradient: gradient } : { Solid: colorToEmit });
	}

	function swapNewWithOld() {
		const old = oldColor;

		const tempHue = hue;
		const tempSaturation = saturation;
		const tempValue = value;
		const tempAlpha = alpha;
		const tempIsNone = isNone;

		setNewHSVA(oldHue, oldSaturation, oldValue, oldAlpha, oldIsNone);
		setOldHSVA(tempHue, tempSaturation, tempValue, tempAlpha, tempIsNone);

		setColor(old || "None");
	}

	function setColorCode(colorCode: string) {
		const color = colorFromCSS(colorCode);
		if (color) setColor(color);
	}

	function setColorRGB(channel: keyof RGB, strength: number | undefined) {
		// Do nothing if the given value is undefined
		if (strength === undefined || !newColor) return undefined;
		// Set the specified channel to the given value
		else if (channel === "r") setColor(createColor(strength / 255, newColor.green, newColor.blue, newColor.alpha));
		else if (channel === "g") setColor(createColor(newColor.red, strength / 255, newColor.blue, newColor.alpha));
		else if (channel === "b") setColor(createColor(newColor.red, newColor.green, strength / 255, newColor.alpha));
	}

	function setColorHSV(channel: keyof HSV, strength: number | undefined) {
		// Do nothing if the given value is undefined
		if (strength === undefined) return undefined;
		// Set the specified channel to the given value
		else if (channel === "h") hue = strength / 360;
		else if (channel === "s") saturation = strength / 100;
		else if (channel === "v") value = strength / 100;

		setColor();
	}

	function setColorAlphaPercent(strength: number | undefined) {
		if (strength !== undefined) alpha = strength / 100;
		setColor();
	}

	function setColorPreset(preset: PresetColors) {
		dispatch("startHistoryTransaction");

		if (preset === "None") {
			setNewHSVA(0, 0, 0, 1, true);
			setColor("None");
		} else {
			const presetColor = createColor(...PURE_COLORS[preset], 1);
			const hsv = colorToHSV(presetColor);
			if (!hsv) return;

			setNewHSVA(hsv.h, hsv.s, hsv.v, presetColor.alpha, false);
			setColor(presetColor);
		}
	}

	function setNewHSVA(h: number, s: number, v: number, a: number, none: boolean) {
		hue = h;
		saturation = s;
		value = v;
		alpha = a;
		isNone = none;
	}

	function setOldHSVA(h: number, s: number, v: number, a: number, none: boolean) {
		oldHue = h;
		oldSaturation = s;
		oldValue = v;
		oldAlpha = a;
		oldIsNone = none;
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
			dispatch("startHistoryTransaction");
			setColorCode(result.sRGBHex);
		} catch {
			// Do nothing
		}
	}

	function gradientActiveMarkerIndexChange({ detail: { activeMarkerIndex, activeMarkerIsMidpoint } }: CustomEvent<{ activeMarkerIndex: number | undefined; activeMarkerIsMidpoint: boolean }>) {
		activeIndex = activeMarkerIndex;
		activeIndexIsMidpoint = activeMarkerIsMidpoint;

		const color = activeMarkerIndex === undefined ? undefined : gradient?.color[activeMarkerIndex];
		const hsv = color ? colorToHSV(color) : undefined;
		if (!color || !hsv) return;

		setColor(color);

		setNewHSVA(hsv.h, hsv.s, hsv.v, color.alpha, false);
		setOldHSVA(hsv.h, hsv.s, hsv.v, color.alpha, false);
	}

	export function div(): HTMLDivElement | undefined {
		return self?.div();
	}
</script>

<FloatingMenu class="color-picker" classes={{ disabled }} {open} on:open {strayCloses} escapeCloses={strayCloses && !gradientSpectrumDragging} {direction} type="Popover" bind:this={self}>
	<LayoutRow
		styles={{
			"--new-color": newColor ? colorToHexOptionalAlpha(newColor) : undefined,
			"--new-color-contrasting": colorContrastingColor(newColor),
			"--old-color": oldColor ? colorToHexOptionalAlpha(oldColor) : undefined,
			"--old-color-contrasting": colorContrastingColor(oldColor),
			"--hue-color": colorToRgbCSS(opaqueHueColor),
			"--hue-color-contrasting": colorContrastingColor(opaqueHueColor),
			"--opaque-color": colorToHexNoAlpha(newColor ? colorOpaque(newColor) : createColor(0, 0, 0, 1)),
			"--opaque-color-contrasting": colorContrastingColor(newColor ? colorOpaque(newColor) : createColor(0, 0, 0, 1)),
		}}
	>
		{@const hueDescription = "The shade along the spectrum of the rainbow."}
		{@const saturationDescription = "The vividness from grayscale to full color."}
		{@const valueDescription = "The brightness from black to full color."}
		<LayoutCol class="pickers-and-gradient">
			<VisualColorPickersInput
				{hue}
				{saturation}
				{value}
				{alpha}
				{isNone}
				{disabled}
				getFloatingMenuElement={() => self?.div()}
				on:update={onVisualUpdate}
				on:startHistoryTransaction={() => dispatch("startHistoryTransaction")}
				on:commitHistoryTransaction={() => dispatch("commitHistoryTransaction")}
				on:dragStateChange={({ detail }) => (strayCloses = !detail)}
			/>
			{#if gradient}
				<LayoutRow class="gradient">
					<SpectrumInput
						{gradient}
						{disabled}
						on:gradient={() => dispatch("colorOrGradient", gradient ? { Gradient: gradient } : "None")}
						on:activeMarkerIndexChange={gradientActiveMarkerIndexChange}
						activeMarkerIndex={activeIndex}
						activeMarkerIsMidpoint={activeIndexIsMidpoint}
						on:dragging={({ detail }) => (gradientSpectrumDragging = detail)}
						bind:this={gradientSpectrumInputWidget}
					/>
					{#if gradientSpectrumInputWidget && activeIndex !== undefined}
						<NumberInput
							value={(activeIndexIsMidpoint ? gradient.midpoint[activeIndex] : gradient.position[activeIndex] || 0) * 100}
							{disabled}
							on:value={({ detail: position }) => {
								if (gradientSpectrumInputWidget && activeIndex !== undefined && position !== undefined) {
									gradientSpectrumInputWidget.setPosition(activeIndex, position / 100, activeIndexIsMidpoint);
								}
							}}
							displayDecimalPlaces={0}
							min={activeIndexIsMidpoint ? MIN_MIDPOINT * 100 : 0}
							max={activeIndexIsMidpoint ? MAX_MIDPOINT * 100 : 100}
							unit="%"
						/>
					{/if}
				</LayoutRow>
			{/if}
		</LayoutCol>
		<LayoutCol class="details">
			<LayoutRow
				class="choice-preview"
				classes={{ outlined, transparency }}
				styles={{ "--outline-amount": outlineFactor }}
				tooltipDescription={!colorEquals(newColor, oldColor) ? "Comparison between the present color choice (left) and the color before it was changed (right)." : "The present color choice."}
			>
				{#if !colorEquals(newColor, oldColor) && !disabled}
					<div class="swap-button-background"></div>
					<IconButton class="swap-button" icon="SwapHorizontal" size={16} action={swapNewWithOld} tooltipLabel="Swap" />
				{/if}
				<LayoutCol class="new-color" classes={{ none: isNone }}>
					{#if !colorEquals(newColor, oldColor)}
						<TextLabel>New</TextLabel>
					{/if}
				</LayoutCol>
				{#if !colorEquals(newColor, oldColor)}
					<LayoutCol class="old-color" classes={{ none: oldIsNone }}>
						<TextLabel>Old</TextLabel>
					</LayoutCol>
				{/if}
			</LayoutRow>
			<!-- <DropdownInput entries={[[{ label: "sRGB" }]]} selectedIndex={0} disabled={true} tooltipDescription="Color model, color space, and HDR (coming soon)." /> -->
			<LayoutRow>
				{@const hexDescription = "Color code in hexadecimal format. 6 digits if opaque, 8 with alpha. Accepts input of CSS color values including named colors."}
				<TextLabel tooltipLabel="Hex Color Code" tooltipDescription={hexDescription}>Hex</TextLabel>
				<Separator style="Related" />
				<LayoutRow>
					<TextInput
						value={newColor ? colorToHexOptionalAlpha(newColor) : "-"}
						{disabled}
						on:commitText={({ detail }) => {
							dispatch("startHistoryTransaction");
							setColorCode(detail);
						}}
						centered={true}
						tooltipLabel="Hex Color Code"
						tooltipDescription={hexDescription}
						bind:this={hexCodeInputWidget}
					/>
				</LayoutRow>
			</LayoutRow>
			<LayoutRow>
				<TextLabel tooltipLabel="Red/Green/Blue" tooltipDescription="Integers 0–255.">RGB</TextLabel>
				<Separator style="Related" />
				<LayoutRow>
					{#each rgbChannels as [channel, strength], index}
						{#if index > 0}
							<Separator style="Related" />
						{/if}
						<NumberInput
							value={strength}
							{disabled}
							on:value={({ detail }) => {
								strength = detail;
								setColorRGB(channel, detail);
							}}
							on:startHistoryTransaction={() => {
								dispatch("startHistoryTransaction");
							}}
							min={0}
							max={255}
							minWidth={1}
							displayDecimalPlaces={0}
							tooltipLabel={{ r: "Red Channel", g: "Green Channel", b: "Blue Channel" }[channel]}
							tooltipDescription="Integers 0–255."
						/>
					{/each}
				</LayoutRow>
			</LayoutRow>
			<LayoutRow>
				<TextLabel
					tooltipLabel="Hue/Saturation/Value"
					tooltipDescription="Also known as Hue/Saturation/Brightness (HSB). Not to be confused with Hue/Saturation/Lightness (HSL), a different color model.">HSV</TextLabel
				>
				<Separator style="Related" />
				<LayoutRow>
					{#each hsvChannels as [channel, strength], index}
						{#if index > 0}
							<Separator style="Related" />
						{/if}
						<NumberInput
							value={strength}
							{disabled}
							on:value={({ detail }) => {
								strength = detail;
								setColorHSV(channel, detail);
							}}
							on:startHistoryTransaction={() => {
								dispatch("startHistoryTransaction");
							}}
							min={0}
							max={channel === "h" ? 360 : 100}
							unit={channel === "h" ? "°" : "%"}
							minWidth={1}
							displayDecimalPlaces={1}
							tooltipLabel={{
								h: "Hue Component",
								s: "Saturation Component",
								v: "Value Component",
							}[channel]}
							tooltipDescription={{
								h: hueDescription,
								s: saturationDescription,
								v: valueDescription,
							}[channel]}
						/>
					{/each}
				</LayoutRow>
			</LayoutRow>
			<LayoutRow>
				{@const alphaDescription = "The level of translucency, from transparent (0%) to opaque (100%)."}
				<TextLabel tooltipLabel="Alpha" tooltipDescription={alphaDescription}>Alpha</TextLabel>
				<Separator style="Related" />
				<NumberInput
					value={!isNone ? alpha * 100 : undefined}
					{disabled}
					on:value={({ detail }) => {
						if (detail !== undefined) alpha = detail / 100;
						setColorAlphaPercent(detail);
					}}
					on:startHistoryTransaction={() => {
						dispatch("startHistoryTransaction");
					}}
					min={0}
					max={100}
					rangeMin={0}
					rangeMax={100}
					unit="%"
					mode="Range"
					displayDecimalPlaces={1}
					tooltipLabel="Alpha"
					tooltipDescription={alphaDescription}
				/>
			</LayoutRow>
			<LayoutRow class="leftover-space" />
			<LayoutRow>
				{#if allowNone && !gradient}
					<button
						class="preset-color none"
						{disabled}
						on:click={() => setColorPreset("None")}
						data-tooltip-label="Set to No Color"
						data-tooltip-description={disabled ? "Disabled (read-only)." : ""}
						tabindex="0"
					></button>
					<Separator style="Related" />
				{/if}
				<button
					class="preset-color black"
					{disabled}
					on:click={() => setColorPreset("Black")}
					data-tooltip-label="Set to Black"
					data-tooltip-description={disabled ? "Disabled (read-only)." : ""}
					tabindex="0"
				></button>
				<Separator style="Related" />
				<button
					class="preset-color white"
					{disabled}
					on:click={() => setColorPreset("White")}
					data-tooltip-label="Set to White"
					data-tooltip-description={disabled ? "Disabled (read-only)." : ""}
					tabindex="0"
				></button>
				<Separator style="Related" />
				<button class="preset-color pure" {disabled} tabindex="-1">
					{#each PURE_COLORS_GRAYABLE as [preset, color, gray]}
						<div
							on:click={() => setColorPreset(preset)}
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
		</LayoutCol>
	</LayoutRow>
</FloatingMenu>

<style lang="scss">
	.color-picker {
		--widget-height: 24px;

		.pickers-and-gradient {
			.gradient {
				margin-top: 16px;

				.spectrum-input {
					flex: 1 1 100%;
				}

				.number-input {
					margin-left: 8px;
					min-width: 0;
					width: calc(24px + 8px + 24px);
					flex: 0 0 auto;
				}
			}
		}

		.details {
			margin-left: 16px;
			width: 200px;
			gap: 8px;

			> .layout-row {
				height: 24px;
				flex: 0 0 auto;

				> .text-label {
					// TODO: Use a table or grid layout for this width to match the widest label. Hard-coding it won't work when we add translation/localization.
					flex: 0 0 34px;
					line-height: 24px;
				}

				&.leftover-space {
					flex: 1 1 100%;
				}
			}

			.choice-preview {
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
			}

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
		}

		&.disabled .details .preset-color,
		&.disabled .details .choice-preview {
			transition: opacity 0.1s;

			&:hover {
				opacity: 0.5;
			}
		}

		&.disabled .details .preset-color.pure:hover div {
			background: var(--pure-color-gray);
		}
	}
</style>
