<script lang="ts">
	import { onDestroy, createEventDispatcher, getContext } from "svelte";

	import { clamp } from "@graphite/utility-functions/math";
	import type { Editor } from "@graphite/wasm-communication/editor";
	import { type HSV, type RGB } from "@graphite/wasm-communication/messages";
	import { Color } from "@graphite/wasm-communication/messages";

	import FloatingMenu, { type MenuDirection } from "@graphite/components/layout/FloatingMenu.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import NumberInput from "@graphite/components/widgets/inputs/NumberInput.svelte";
	import TextInput from "@graphite/components/widgets/inputs/TextInput.svelte";
	import Separator from "@graphite/components/widgets/labels/Separator.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	type PresetColors = "none" | "black" | "white" | "red" | "yellow" | "green" | "cyan" | "blue" | "magenta";

	const PURE_COLORS: Record<PresetColors, [number, number, number]> = {
		none: [0, 0, 0],
		black: [0, 0, 0],
		white: [1, 1, 1],
		red: [1, 0, 0],
		yellow: [1, 1, 0],
		green: [0, 1, 0],
		cyan: [0, 1, 1],
		blue: [0, 0, 1],
		magenta: [1, 0, 1],
	};

	const editor = getContext<Editor>("editor");

	const dispatch = createEventDispatcher<{ color: Color; start: undefined }>();

	export let color: Color;
	export let allowNone = false;
	// export let allowTransparency = false; // TODO: Implement
	export let direction: MenuDirection = "Bottom";
	// TODO: See if this should be made to follow the pattern of DropdownInput.svelte so this could be removed
	export let open: boolean;

	const hsvaOrNone = color.toHSVA();
	const hsva = hsvaOrNone || { h: 0, s: 0, v: 0, a: 1 };

	// New color components
	let hue = hsva.h;
	let saturation = hsva.s;
	let value = hsva.v;
	let alpha = hsva.a;
	let isNone = hsvaOrNone === undefined;
	// Initial color components
	let initialHue = hsva.h;
	let initialSaturation = hsva.s;
	let initialValue = hsva.v;
	let initialAlpha = hsva.a;
	let initialIsNone = hsvaOrNone === undefined;
	// Transient state
	let draggingPickerTrack: HTMLDivElement | undefined = undefined;
	let strayCloses = true;

	let hexCodeInputWidget: TextInput | undefined;

	$: watchOpen(open);
	$: watchColor(color);

	$: initialColor = generateColor(initialHue, initialSaturation, initialValue, initialAlpha, initialIsNone);
	$: newColor = generateColor(hue, saturation, value, alpha, isNone);
	$: rgbChannels = Object.entries(newColor.toRgb255() || { r: undefined, g: undefined, b: undefined }) as [keyof RGB, number | undefined][];
	$: hsvChannels = Object.entries(!isNone ? { h: hue * 360, s: saturation * 100, v: value * 100 } : { h: undefined, s: undefined, v: undefined }) as [keyof HSV, number | undefined][];
	$: opaqueHueColor = new Color({ h: hue, s: 1, v: 1, a: 1 });

	function generateColor(h: number, s: number, v: number, a: number, none: boolean) {
		if (none) return new Color("none");
		return new Color({ h, s, v, a });
	}

	function watchOpen(open: boolean) {
		if (open) {
			setTimeout(() => hexCodeInputWidget?.focus(), 0);
		} else {
			setInitialHSVA(hue, saturation, value, alpha, isNone);
		}
	}

	function watchColor(color: Color) {
		const hsva = color.toHSVA();

		if (hsva === undefined) {
			setNewHSVA(0, 0, 0, 1, true);
			return;
		}

		// Update the hue, but only if it is necessary so we don't:
		// - ...jump the user's hue from 360° (top) to the equivalent 0° (bottom)
		// - ...reset the hue to 0° if the color is fully desaturated, where all hues are equivalent
		// - ...reset the hue to 0° if the color's value is black, where all hues are equivalent
		if (!(hsva.h === 0 && hue === 1) && hsva.s > 0 && hsva.v > 0) hue = hsva.h;
		// Update the saturation, but only if it is necessary so we don't:
		// - ...reset the saturation to the left if the color's value is black along the bottom edge, where all saturations are equivalent
		if (hsva.v !== 0) saturation = hsva.s;
		// Update the value
		value = hsva.v;
		// Update the alpha
		alpha = hsva.a;
		// Update the status of this not being a color
		isNone = false;
	}

	function onPointerDown(e: PointerEvent) {
		const target = (e.target || undefined) as HTMLElement | undefined;
		draggingPickerTrack = target?.closest("[data-saturation-value-picker], [data-hue-picker], [data-alpha-picker]") || undefined;

		addEvents();

		onPointerMove(e);
	}

	function onPointerMove(e: PointerEvent) {
		// Just in case the mouseup event is lost
		if (e.buttons === 0) removeEvents();

		if (draggingPickerTrack?.hasAttribute("data-saturation-value-picker")) {
			const rectangle = draggingPickerTrack.getBoundingClientRect();

			saturation = clamp((e.clientX - rectangle.left) / rectangle.width, 0, 1);
			value = clamp(1 - (e.clientY - rectangle.top) / rectangle.height, 0, 1);
			strayCloses = false;
		} else if (draggingPickerTrack?.hasAttribute("data-hue-picker")) {
			const rectangle = draggingPickerTrack.getBoundingClientRect();

			hue = clamp(1 - (e.clientY - rectangle.top) / rectangle.height, 0, 1);
			strayCloses = false;
		} else if (draggingPickerTrack?.hasAttribute("data-alpha-picker")) {
			const rectangle = draggingPickerTrack.getBoundingClientRect();

			alpha = clamp(1 - (e.clientY - rectangle.top) / rectangle.height, 0, 1);
			strayCloses = false;
		}

		const color = new Color({ h: hue, s: saturation, v: value, a: alpha });
		setColor(color);
	}

	function onPointerUp() {
		removeEvents();
	}

	function addEvents() {
		document.addEventListener("pointermove", onPointerMove);
		document.addEventListener("pointerup", onPointerUp);

		dispatch("start");
	}

	function removeEvents() {
		draggingPickerTrack = undefined;
		strayCloses = true;

		document.removeEventListener("pointermove", onPointerMove);
		document.removeEventListener("pointerup", onPointerUp);
	}

	function setColor(color?: Color) {
		const colorToEmit = color || new Color({ h: hue, s: saturation, v: value, a: alpha });
		dispatch("color", colorToEmit);
	}

	function swapNewWithInitial() {
		const initial = initialColor;

		const tempHue = hue;
		const tempSaturation = saturation;
		const tempValue = value;
		const tempAlpha = alpha;
		const tempIsNone = isNone;

		setNewHSVA(initialHue, initialSaturation, initialValue, initialAlpha, initialIsNone);
		setInitialHSVA(tempHue, tempSaturation, tempValue, tempAlpha, tempIsNone);

		setColor(initial);
	}

	function setColorCode(colorCode: string) {
		const color = Color.fromCSS(colorCode);
		if (color) setColor(color);
	}

	function setColorRGB(channel: keyof RGB, strength: number | undefined) {
		// Do nothing if the given value is undefined
		if (strength === undefined) undefined;
		// Set the specified channel to the given value
		else if (channel === "r") setColor(new Color(strength / 255, newColor.green, newColor.blue, newColor.alpha));
		else if (channel === "g") setColor(new Color(newColor.red, strength / 255, newColor.blue, newColor.alpha));
		else if (channel === "b") setColor(new Color(newColor.red, newColor.green, strength / 255, newColor.alpha));
	}

	function setColorHSV(channel: keyof HSV, strength: number | undefined) {
		// Do nothing if the given value is undefined
		if (strength === undefined) undefined;
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

	function setColorPresetSubtile(e: MouseEvent) {
		const clickedTile = e.target as HTMLDivElement | undefined;
		const tileColor = clickedTile?.getAttribute("data-pure-tile") || undefined;

		if (tileColor) setColorPreset(tileColor as PresetColors);
	}

	function setColorPreset(preset: PresetColors) {
		if (preset === "none") {
			setNewHSVA(0, 0, 0, 1, true);
			setColor(new Color("none"));
			return;
		}

		const presetColor = new Color(...PURE_COLORS[preset], 1);
		const hsva = presetColor.toHSVA() || { h: 0, s: 0, v: 0, a: 0 };

		setNewHSVA(hsva.h, hsva.s, hsva.v, hsva.a, false);
		setColor(presetColor);
	}

	function setNewHSVA(h: number, s: number, v: number, a: number, none: boolean) {
		hue = h;
		saturation = s;
		value = v;
		alpha = a;
		isNone = none;
	}

	function setInitialHSVA(h: number, s: number, v: number, a: number, none: boolean) {
		initialHue = h;
		initialSaturation = s;
		initialValue = v;
		initialAlpha = a;
		initialIsNone = none;
	}

	async function activateEyedropperSample() {
		// TODO: Replace this temporary solution that only works in Chromium-based browsers with the custom color sampler used by the Eyedropper tool
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		if (!(window as any).EyeDropper) {
			editor.instance.eyedropperSampleForColorPicker();
			return;
		}

		try {
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			const result = await new (window as any).EyeDropper().open();
			setColorCode(result.sRGBHex);
		} catch {
			// Do nothing
		}
	}

	onDestroy(() => {
		removeEvents();
	});
</script>

<FloatingMenu class="color-picker" {open} on:open {strayCloses} {direction} type="Popover">
	<LayoutRow
		styles={{
			"--new-color": newColor.toHexOptionalAlpha(),
			"--new-color-contrasting": newColor.contrastingColor(),
			"--initial-color": initialColor.toHexOptionalAlpha(),
			"--initial-color-contrasting": initialColor.contrastingColor(),
			"--hue-color": opaqueHueColor.toRgbCSS(),
			"--hue-color-contrasting": opaqueHueColor.contrastingColor(),
			"--opaque-color": (newColor.opaque() || new Color(0, 0, 0, 1)).toHexNoAlpha(),
			"--opaque-color-contrasting": (newColor.opaque() || new Color(0, 0, 0, 1)).contrastingColor(),
		}}
	>
		<LayoutCol class="saturation-value-picker" on:pointerdown={onPointerDown} data-saturation-value-picker>
			{#if !isNone}
				<div class="selection-circle" style:top={`${(1 - value) * 100}%`} style:left={`${saturation * 100}%`} />
			{/if}
		</LayoutCol>
		<LayoutCol class="hue-picker" on:pointerdown={onPointerDown} data-hue-picker>
			{#if !isNone}
				<div class="selection-needle" style:top={`${(1 - hue) * 100}%`} />
			{/if}
		</LayoutCol>
		<LayoutCol class="alpha-picker" on:pointerdown={onPointerDown} data-alpha-picker>
			{#if !isNone}
				<div class="selection-needle" style:top={`${(1 - alpha) * 100}%`} />
			{/if}
		</LayoutCol>
		<LayoutCol class="details">
			<LayoutRow class="choice-preview" on:click={swapNewWithInitial} tooltip="Comparison views of the present color choice (left) and the color before any change (right). Click to swap sides.">
				<LayoutCol class="new-color" classes={{ none: isNone }}>
					<TextLabel>New</TextLabel>
				</LayoutCol>
				<LayoutCol class="initial-color" classes={{ none: initialIsNone }}>
					<TextLabel>Initial</TextLabel>
				</LayoutCol>
			</LayoutRow>
			<!-- <DropdownInput entries={[[{ label: "sRGB" }]]} selectedIndex={0} disabled={true} tooltip="Color model, color space, and HDR (coming soon)" /> -->
			<LayoutRow>
				<TextLabel tooltip={"Color code in hexadecimal format. 6 digits if opaque, 8 with alpha.\nAccepts input of CSS color values including named colors."}>Hex</TextLabel>
				<Separator />
				<LayoutRow>
					<TextInput
						value={newColor.toHexOptionalAlpha() || "-"}
						on:commitText={({ detail }) => setColorCode(detail)}
						centered={true}
						tooltip={"Color code in hexadecimal format. 6 digits if opaque, 8 with alpha.\nAccepts input of CSS color values including named colors."}
						bind:this={hexCodeInputWidget}
					/>
				</LayoutRow>
			</LayoutRow>
			<LayoutRow>
				<TextLabel tooltip="Red/Green/Blue channels of the color, integers 0–255">RGB</TextLabel>
				<Separator />
				<LayoutRow>
					{#each rgbChannels as [channel, strength], index}
						{#if index > 0}
							<Separator type="Related" />
						{/if}
						<NumberInput
							value={strength}
							on:value={({ detail }) => {
								strength = detail;
								setColorRGB(channel, detail);
							}}
							min={0}
							max={255}
							minWidth={56}
							tooltip={`${{ r: "Red", g: "Green", b: "Blue" }[channel]} channel, integers 0–255`}
						/>
					{/each}
				</LayoutRow>
			</LayoutRow>
			<LayoutRow>
				<TextLabel tooltip={"Hue/Saturation/Value, also known as Hue/Saturation/Brightness (HSB).\nNot to be confused with Hue/Saturation/Lightness (HSL), a different color model."}>
					HSV
				</TextLabel>
				<Separator />
				<LayoutRow>
					{#each hsvChannels as [channel, strength], index}
						{#if index > 0}
							<Separator type="Related" />
						{/if}
						<NumberInput
							value={strength}
							on:value={({ detail }) => {
								strength = detail;
								setColorHSV(channel, detail);
							}}
							min={0}
							max={channel === "h" ? 360 : 100}
							unit={channel === "h" ? "°" : "%"}
							minWidth={56}
							tooltip={{
								h: `Hue component, the shade along the spectrum of the rainbow`,
								s: `Saturation component, the vividness from grayscale to full color`,
								v: "Value component, the brightness from black to full color",
							}[channel]}
						/>
					{/each}
				</LayoutRow>
			</LayoutRow>
			<NumberInput
				label="Alpha"
				value={!isNone ? alpha * 100 : undefined}
				on:value={({ detail }) => {
					if (detail !== undefined) alpha = detail / 100;
					setColorAlphaPercent(detail);
				}}
				min={0}
				max={100}
				rangeMin={0}
				rangeMax={100}
				unit="%"
				mode="Range"
				displayDecimalPlaces={1}
				tooltip={`Scale from transparent (0%) to opaque (100%) for the color's alpha channel`}
			/>
			<LayoutRow class="leftover-space" />
			<LayoutRow>
				{#if allowNone}
					<button class="preset-color none" on:click={() => setColorPreset("none")} title="Set No Color" tabindex="0" />
					<Separator type="Related" />
				{/if}
				<button class="preset-color black" on:click={() => setColorPreset("black")} title="Set Black" tabindex="0" />
				<Separator type="Related" />
				<button class="preset-color white" on:click={() => setColorPreset("white")} title="Set White" tabindex="0" />
				<Separator type="Related" />
				<button class="preset-color pure" on:click={setColorPresetSubtile} tabindex="-1">
					<div data-pure-tile="red" style="--pure-color: #ff0000; --pure-color-gray: #4c4c4c" title="Set Red" />
					<div data-pure-tile="yellow" style="--pure-color: #ffff00; --pure-color-gray: #e3e3e3" title="Set Yellow" />
					<div data-pure-tile="green" style="--pure-color: #00ff00; --pure-color-gray: #969696" title="Set Green" />
					<div data-pure-tile="cyan" style="--pure-color: #00ffff; --pure-color-gray: #b2b2b2" title="Set Cyan" />
					<div data-pure-tile="blue" style="--pure-color: #0000ff; --pure-color-gray: #1c1c1c" title="Set Blue" />
					<div data-pure-tile="magenta" style="--pure-color: #ff00ff; --pure-color-gray: #696969" title="Set Magenta" />
				</button>
				<Separator type="Related" />
				<IconButton icon="Eyedropper" size={24} action={activateEyedropperSample} tooltip="Sample a pixel color from the document" />
			</LayoutRow>
		</LayoutCol>
	</LayoutRow>
</FloatingMenu>

<style lang="scss" global>
	.color-picker {
		.saturation-value-picker {
			width: 256px;
			background-blend-mode: multiply;
			background: linear-gradient(to bottom, #ffffff, #000000), linear-gradient(to right, #ffffff, var(--hue-color));
			position: relative;
		}

		.saturation-value-picker,
		.hue-picker,
		.alpha-picker {
			height: 256px;
			position: relative;
			overflow: hidden;
		}

		.hue-picker,
		.alpha-picker {
			width: 24px;
			margin-left: 8px;
			position: relative;
		}

		.hue-picker {
			background-blend-mode: screen;
			background:
				// Reds
				linear-gradient(to top, #ff0000ff 16.666%, #ff000000 33.333%, #ff000000 66.666%, #ff0000ff 83.333%),
				// Greens
				linear-gradient(to top, #00ff0000 0%, #00ff00ff 16.666%, #00ff00ff 50%, #00ff0000 66.666%),
				// Blues
				linear-gradient(to top, #0000ff00 33.333%, #0000ffff 50%, #0000ffff 83.333%, #0000ff00 100%);
			--selection-needle-color: var(--hue-color-contrasting);
		}

		.alpha-picker {
			background: linear-gradient(to bottom, var(--opaque-color), transparent);
			--selection-needle-color: var(--new-color-contrasting);

			&::before {
				content: "";
				width: 100%;
				height: 100%;
				z-index: -1;
				position: relative;
				background: var(--color-transparent-checkered-background);
				background-size: var(--color-transparent-checkered-background-size);
				background-position: var(--color-transparent-checkered-background-position);
			}
		}

		.selection-circle {
			position: absolute;
			left: 0%;
			top: 0%;
			width: 0;
			height: 0;
			pointer-events: none;

			&::after {
				content: "";
				display: block;
				position: relative;
				left: -6px;
				top: -6px;
				width: 12px;
				height: 12px;
				border-radius: 50%;
				border: 2px solid var(--opaque-color-contrasting);
				box-sizing: border-box;
			}
		}

		.selection-needle {
			position: absolute;
			top: 0%;
			width: 100%;
			height: 0;
			pointer-events: none;

			&::before {
				content: "";
				position: absolute;
				top: -4px;
				left: 0;
				border-style: solid;
				border-width: 4px 0 4px 4px;
				border-color: transparent transparent transparent var(--selection-needle-color);
			}

			&::after {
				content: "";
				position: absolute;
				top: -4px;
				right: 0;
				border-style: solid;
				border-width: 4px 4px 4px 0;
				border-color: transparent var(--selection-needle-color) transparent transparent;
			}
		}

		.details {
			margin-left: 16px;
			width: 208px;
			gap: 8px;

			> .layout-row {
				height: 24px;
				flex: 0 0 auto;

				> .text-label {
					width: 24px;
					flex: 0 0 auto;
				}

				&.leftover-space {
					flex: 1 1 100%;
				}
			}

			.choice-preview {
				flex: 0 0 auto;
				width: 208px;
				height: 32px;
				border-radius: 2px;
				border: 1px solid var(--color-1-nearblack);
				box-sizing: border-box;
				overflow: hidden;

				.new-color {
					background: linear-gradient(var(--new-color), var(--new-color)), var(--color-transparent-checkered-background);

					.text-label {
						text-align: left;
						margin: 2px 8px;
						color: var(--new-color-contrasting);
					}
				}

				.initial-color {
					background: linear-gradient(var(--initial-color), var(--initial-color)), var(--color-transparent-checkered-background);

					.text-label {
						text-align: right;
						margin: 2px 8px;
						color: var(--initial-color-contrasting);
					}
				}

				.new-color,
				.initial-color {
					width: 50%;
					height: 100%;
					background-size: var(--color-transparent-checkered-background-size);
					background-position: var(--color-transparent-checkered-background-position);

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
				width: calc(48px + (48px + 4px) / 2);
				height: 24px;

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

					div {
						display: inline-block;
						width: calc(100% / 3);
						height: 50%;
						// For the least jarring luminance conversion, these colors are derived by placing a black layer with the "desaturate" blend mode over the colors.
						// We don't use the CSS `filter: grayscale(1);` property because it produces overly dark tones for bright colors with a noticeable jump on hover.
						background: var(--pure-color-gray);
						transition: background-color 0.2s ease;
					}

					&:hover div,
					&:focus div {
						background: var(--pure-color);
					}
				}
			}
		}
	}
</style>
