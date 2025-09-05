<script lang="ts">
	import { onDestroy, createEventDispatcher, getContext } from "svelte";

	import type { Editor } from "@graphite/editor";
	import type { HSV, RGB, FillChoice } from "@graphite/messages";
	import type { MenuDirection } from "@graphite/messages";
	import { Color, contrastingOutlineFactor, Gradient } from "@graphite/messages";
	import { clamp } from "@graphite/utility-functions/math";

	import FloatingMenu from "@graphite/components/layout/FloatingMenu.svelte";
	import { preventEscapeClosingParentFloatingMenu } from "@graphite/components/layout/FloatingMenu.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import NumberInput from "@graphite/components/widgets/inputs/NumberInput.svelte";
	import SpectrumInput from "@graphite/components/widgets/inputs/SpectrumInput.svelte";
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

	const dispatch = createEventDispatcher<{ colorOrGradient: FillChoice; startHistoryTransaction: undefined }>();

	export let colorOrGradient: FillChoice;
	export let allowNone = false;
	// export let allowTransparency = false; // TODO: Implement
	export let disabled = false;
	export let direction: MenuDirection = "Bottom";
	// TODO: See if this should be made to follow the pattern of DropdownInput.svelte so this could be removed
	export let open: boolean;

	const hsvaOrNone = colorOrGradient instanceof Color ? colorOrGradient.toHSVA() : colorOrGradient.firstColor()?.toHSVA();
	const hsva = hsvaOrNone || { h: 0, s: 0, v: 0, a: 1 };

	// Gradient color stops
	$: gradient = colorOrGradient instanceof Gradient ? colorOrGradient : undefined;
	let activeIndex = 0 as number | undefined;
	$: selectedGradientColor = (activeIndex !== undefined && gradient?.atIndex(activeIndex)?.color) || (Color.fromCSS("black") as Color);
	// Currently viewed color
	$: color = colorOrGradient instanceof Color ? colorOrGradient : selectedGradientColor;
	// New color components
	let hue = hsva.h;
	let saturation = hsva.s;
	let value = hsva.v;
	let alpha = hsva.a;
	let isNone = hsvaOrNone === undefined;
	// Old color components
	let oldHue = hsva.h;
	let oldSaturation = hsva.s;
	let oldValue = hsva.v;
	let oldAlpha = hsva.a;
	let oldIsNone = hsvaOrNone === undefined;
	// Transient state
	let draggingPickerTrack: HTMLDivElement | undefined = undefined;
	let strayCloses = true;
	let gradientSpectrumDragging = false;
	let shiftPressed = false;
	let alignedAxis: "saturation" | "value" | undefined = undefined;
	let hueBeforeDrag = 0;
	let saturationBeforeDrag = 0;
	let valueBeforeDrag = 0;
	let alphaBeforeDrag = 0;
	let saturationStartOfAxisAlign: number | undefined = undefined;
	let valueStartOfAxisAlign: number | undefined = undefined;
	let saturationRestoreWhenShiftReleased: number | undefined = undefined;
	let valueRestoreWhenShiftReleased: number | undefined = undefined;

	let self: FloatingMenu | undefined;
	let hexCodeInputWidget: TextInput | undefined;
	let gradientSpectrumInputWidget: SpectrumInput | undefined;

	$: watchOpen(open);
	$: watchColor(color);

	$: oldColor = generateColor(oldHue, oldSaturation, oldValue, oldAlpha, oldIsNone);
	$: newColor = generateColor(hue, saturation, value, alpha, isNone);
	$: rgbChannels = Object.entries(newColor.toRgb255() || { r: undefined, g: undefined, b: undefined }) as [keyof RGB, number | undefined][];
	$: hsvChannels = Object.entries(!isNone ? { h: hue * 360, s: saturation * 100, v: value * 100 } : { h: undefined, s: undefined, v: undefined }) as [keyof HSV, number | undefined][];
	$: opaqueHueColor = new Color({ h: hue, s: 1, v: 1, a: 1 });
	$: outlineFactor = Math.max(contrastingOutlineFactor(newColor, "--color-2-mildblack", 0.01), contrastingOutlineFactor(oldColor, "--color-2-mildblack", 0.01));
	$: outlined = outlineFactor > 0.0001;
	$: transparency = newColor.alpha < 1 || oldColor.alpha < 1;

	function generateColor(h: number, s: number, v: number, a: number, none: boolean) {
		if (none) return new Color("none");
		return new Color({ h, s, v, a });
	}

	function watchOpen(open: boolean) {
		if (open) {
			setTimeout(() => hexCodeInputWidget?.focus(), 0);
		} else {
			setOldHSVA(hue, saturation, value, alpha, isNone);
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
		if (disabled) return;

		const target = (e.target || undefined) as HTMLElement | undefined;
		draggingPickerTrack = target?.closest("[data-saturation-value-picker], [data-hue-picker], [data-alpha-picker]") || undefined;

		hueBeforeDrag = hue;
		saturationBeforeDrag = saturation;
		valueBeforeDrag = value;
		alphaBeforeDrag = alpha;

		saturationStartOfAxisAlign = undefined;
		valueStartOfAxisAlign = undefined;

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

			if (shiftPressed) updateAxisLock();
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

		if (!e.shiftKey) {
			shiftPressed = false;
			alignedAxis = undefined;
		} else if (!shiftPressed && draggingPickerTrack) {
			shiftPressed = true;
			saturationStartOfAxisAlign = saturation;
			valueStartOfAxisAlign = value;
		}
	}

	function onPointerUp() {
		removeEvents();
	}

	function onMouseDown(e: MouseEvent) {
		const BUTTONS_RIGHT = 0b0000_0010;
		if (e.buttons & BUTTONS_RIGHT) abortDrag();
	}

	function onKeyDown(e: KeyboardEvent) {
		if (e.key === "Escape") {
			const element = self?.div();
			if (element) preventEscapeClosingParentFloatingMenu(element);

			abortDrag();
		}
	}

	function onKeyUp(e: KeyboardEvent) {
		if (e.key === "Shift") {
			shiftPressed = false;
			alignedAxis = undefined;

			if (saturationRestoreWhenShiftReleased !== undefined && valueRestoreWhenShiftReleased !== undefined) {
				saturation = saturationRestoreWhenShiftReleased;
				value = valueRestoreWhenShiftReleased;

				const color = new Color({ h: hue, s: saturation, v: value, a: alpha });
				setColor(color);
			}
		}
	}

	function addEvents() {
		document.addEventListener("pointermove", onPointerMove);
		document.addEventListener("pointerup", onPointerUp);
		document.addEventListener("mousedown", onMouseDown);
		document.addEventListener("keydown", onKeyDown);
		document.addEventListener("keyup", onKeyUp);

		dispatch("startHistoryTransaction");
	}

	function removeEvents() {
		draggingPickerTrack = undefined;
		// The setTimeout is necessary to prevent the FloatingMenu's `escapeCloses` from becoming true immediately upon pressing the Escape key, and thus closing
		setTimeout(() => (strayCloses = true), 0);
		shiftPressed = false;
		alignedAxis = undefined;

		document.removeEventListener("pointermove", onPointerMove);
		document.removeEventListener("pointerup", onPointerUp);
		document.removeEventListener("mousedown", onMouseDown);
		document.removeEventListener("keydown", onKeyDown);
		document.removeEventListener("keyup", onKeyUp);
	}

	function updateAxisLock() {
		if (!saturationStartOfAxisAlign || !valueStartOfAxisAlign) return;

		const deltaSaturation = saturation - saturationStartOfAxisAlign;
		const deltaValue = value - valueStartOfAxisAlign;

		saturationRestoreWhenShiftReleased = saturation;
		valueRestoreWhenShiftReleased = value;

		if (Math.abs(deltaSaturation) < Math.abs(deltaValue)) {
			alignedAxis = "saturation";
			saturation = saturationStartOfAxisAlign;
		} else {
			alignedAxis = "value";
			value = valueStartOfAxisAlign;
		}
	}

	function abortDrag() {
		removeEvents();

		hue = hueBeforeDrag;
		saturation = saturationBeforeDrag;
		value = valueBeforeDrag;
		alpha = alphaBeforeDrag;

		const color = new Color({ h: hue, s: saturation, v: value, a: alpha });
		setColor(color);
	}

	function setColor(color?: Color) {
		const colorToEmit = color || new Color({ h: hue, s: saturation, v: value, a: alpha });

		const stop = gradientSpectrumInputWidget && activeIndex !== undefined && gradient?.atIndex(activeIndex);
		if (stop && gradientSpectrumInputWidget instanceof SpectrumInput) {
			stop.color = colorToEmit;
		}

		dispatch("colorOrGradient", gradient || colorToEmit);
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

		setColor(old);
	}

	function setColorCode(colorCode: string) {
		const color = Color.fromCSS(colorCode);
		if (color) setColor(color);
	}

	function setColorRGB(channel: keyof RGB, strength: number | undefined) {
		// Do nothing if the given value is undefined
		if (strength === undefined) return undefined;
		// Set the specified channel to the given value
		else if (channel === "r") setColor(new Color(strength / 255, newColor.green, newColor.blue, newColor.alpha));
		else if (channel === "g") setColor(new Color(newColor.red, strength / 255, newColor.blue, newColor.alpha));
		else if (channel === "b") setColor(new Color(newColor.red, newColor.green, strength / 255, newColor.alpha));
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

	function setColorPresetSubtile(e: MouseEvent) {
		const clickedTile = e.target as HTMLDivElement | undefined;
		const tileColor = clickedTile?.getAttribute("data-pure-tile") || undefined;

		if (tileColor) setColorPreset(tileColor as PresetColors);
	}

	function setColorPreset(preset: PresetColors) {
		dispatch("startHistoryTransaction");
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

	function setOldHSVA(h: number, s: number, v: number, a: number, none: boolean) {
		oldHue = h;
		oldSaturation = s;
		oldValue = v;
		oldAlpha = a;
		oldIsNone = none;
	}

	async function activateEyedropperSample() {
		// TODO: Replace this temporary solution that only works in Chromium-based browsers with the custom color sampler used by the Eyedropper tool
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		if (!(window as any).EyeDropper) {
			editor.handle.eyedropperSampleForColorPicker();
			return;
		}

		try {
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			const result = await new (window as any).EyeDropper().open();
			dispatch("startHistoryTransaction");
			setColorCode(result.sRGBHex);
		} catch {
			// Do nothing
		}
	}

	function gradientActiveMarkerIndexChange({ detail: index }: CustomEvent<number | undefined>) {
		activeIndex = index;
		const color = index === undefined ? undefined : gradient?.colorAtIndex(index);
		const hsva = color?.toHSVA();
		if (!color || !hsva) return;

		setColor(color);

		setNewHSVA(hsva.h, hsva.s, hsva.v, hsva.a, color.none);
		setOldHSVA(hsva.h, hsva.s, hsva.v, hsva.a, color.none);
	}

	onDestroy(() => {
		removeEvents();
	});
</script>

<FloatingMenu class="color-picker" classes={{ disabled }} {open} on:open {strayCloses} escapeCloses={strayCloses && !gradientSpectrumDragging} {direction} type="Popover" bind:this={self}>
	<LayoutRow
		styles={{
			"--new-color": newColor.toHexOptionalAlpha(),
			"--new-color-contrasting": newColor.contrastingColor(),
			"--old-color": oldColor.toHexOptionalAlpha(),
			"--old-color-contrasting": oldColor.contrastingColor(),
			"--hue-color": opaqueHueColor.toRgbCSS(),
			"--hue-color-contrasting": opaqueHueColor.contrastingColor(),
			"--opaque-color": (newColor.opaque() || new Color(0, 0, 0, 1)).toHexNoAlpha(),
			"--opaque-color-contrasting": (newColor.opaque() || new Color(0, 0, 0, 1)).contrastingColor(),
		}}
	>
		<LayoutCol class="pickers-and-gradient">
			<LayoutRow class="pickers">
				<LayoutCol class="saturation-value-picker" title={disabled ? "Saturation and value (disabled)" : "Saturation and value"} on:pointerdown={onPointerDown} data-saturation-value-picker>
					{#if !isNone}
						<div class="selection-circle" style:top={`${(1 - value) * 100}%`} style:left={`${saturation * 100}%`} />
					{/if}
					{#if alignedAxis}
						<div
							class="selection-circle-alignment"
							class:saturation={alignedAxis === "saturation"}
							class:value={alignedAxis === "value"}
							style:top={`${(1 - value) * 100}%`}
							style:left={`${saturation * 100}%`}
						/>
					{/if}
				</LayoutCol>
				<LayoutCol class="hue-picker" title={disabled ? "Hue (disabled)" : "Hue"} on:pointerdown={onPointerDown} data-hue-picker>
					{#if !isNone}
						<div class="selection-needle" style:top={`${(1 - hue) * 100}%`} />
					{/if}
				</LayoutCol>
				<LayoutCol class="alpha-picker" title={disabled ? "Alpha (disabled)" : "Alpha"} on:pointerdown={onPointerDown} data-alpha-picker>
					{#if !isNone}
						<div class="selection-needle" style:top={`${(1 - alpha) * 100}%`} />
					{/if}
				</LayoutCol>
			</LayoutRow>
			{#if gradient}
				<LayoutRow class="gradient">
					<SpectrumInput
						{gradient}
						{disabled}
						on:gradient={() => {
							if (gradient) dispatch("colorOrGradient", gradient);
						}}
						on:activeMarkerIndexChange={gradientActiveMarkerIndexChange}
						activeMarkerIndex={activeIndex}
						on:dragging={({ detail }) => (gradientSpectrumDragging = detail)}
						bind:this={gradientSpectrumInputWidget}
					/>
					{#if gradientSpectrumInputWidget && activeIndex !== undefined}
						<NumberInput
							value={(gradient.positionAtIndex(activeIndex) || 0) * 100}
							{disabled}
							on:value={({ detail }) => {
								if (gradientSpectrumInputWidget && activeIndex !== undefined && detail !== undefined) gradientSpectrumInputWidget.setPosition(activeIndex, detail / 100);
							}}
							displayDecimalPlaces={0}
							min={0}
							max={100}
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
				tooltip={!newColor.equals(oldColor) ? "Comparison between the present color choice (left) and the color before any change was made (right)" : "The present color choice"}
			>
				{#if !newColor.equals(oldColor) && !disabled}
					<div class="swap-button-background"></div>
					<IconButton class="swap-button" icon="SwapHorizontal" size={16} action={swapNewWithOld} tooltip="Swap" />
				{/if}
				<LayoutCol class="new-color" classes={{ none: isNone }}>
					{#if !newColor.equals(oldColor)}
						<TextLabel>New</TextLabel>
					{/if}
				</LayoutCol>
				{#if !newColor.equals(oldColor)}
					<LayoutCol class="old-color" classes={{ none: oldIsNone }}>
						<TextLabel>Old</TextLabel>
					</LayoutCol>
				{/if}
			</LayoutRow>
			<!-- <DropdownInput entries={[[{ label: "sRGB" }]]} selectedIndex={0} disabled={true} tooltip="Color model, color space, and HDR (coming soon)" /> -->
			<LayoutRow>
				<TextLabel tooltip={"Color code in hexadecimal format. 6 digits if opaque, 8 with alpha.\nAccepts input of CSS color values including named colors."}>Hex</TextLabel>
				<Separator type="Related" />
				<LayoutRow>
					<TextInput
						value={newColor.toHexOptionalAlpha() || "-"}
						{disabled}
						on:commitText={({ detail }) => {
							dispatch("startHistoryTransaction");
							setColorCode(detail);
						}}
						centered={true}
						tooltip={"Color code in hexadecimal format. 6 digits if opaque, 8 with alpha.\nAccepts input of CSS color values including named colors."}
						bind:this={hexCodeInputWidget}
					/>
				</LayoutRow>
			</LayoutRow>
			<LayoutRow>
				<TextLabel tooltip="Red/Green/Blue channels of the color, integers 0–255">RGB</TextLabel>
				<Separator type="Related" />
				<LayoutRow>
					{#each rgbChannels as [channel, strength], index}
						{#if index > 0}
							<Separator type="Related" />
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
							tooltip={`${{ r: "Red", g: "Green", b: "Blue" }[channel]} channel, integers 0–255`}
						/>
					{/each}
				</LayoutRow>
			</LayoutRow>
			<LayoutRow>
				<TextLabel tooltip={"Hue/Saturation/Value, also known as Hue/Saturation/Brightness (HSB).\nNot to be confused with Hue/Saturation/Lightness (HSL), a different color model."}>
					HSV
				</TextLabel>
				<Separator type="Related" />
				<LayoutRow>
					{#each hsvChannels as [channel, strength], index}
						{#if index > 0}
							<Separator type="Related" />
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
							tooltip={{
								h: `Hue component, the shade along the spectrum of the rainbow`,
								s: `Saturation component, the vividness from grayscale to full color`,
								v: "Value component, the brightness from black to full color",
							}[channel]}
						/>
					{/each}
				</LayoutRow>
			</LayoutRow>
			<LayoutRow>
				<TextLabel tooltip="Scale of translucency, from transparent (0%) to opaque (100%), for the color's alpha channel">Alpha</TextLabel>
				<Separator type="Related" />
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
					tooltip="Scale of translucency, from transparent (0%) to opaque (100%), for the color's alpha channel"
				/>
			</LayoutRow>
			<LayoutRow class="leftover-space" />
			<LayoutRow>
				{#if allowNone && !gradient}
					<button class="preset-color none" {disabled} on:click={() => setColorPreset("none")} title="Set to no color" tabindex="0"></button>
					<Separator type="Related" />
				{/if}
				<button class="preset-color black" {disabled} on:click={() => setColorPreset("black")} title="Set to black" tabindex="0"></button>
				<Separator type="Related" />
				<button class="preset-color white" {disabled} on:click={() => setColorPreset("white")} title="Set to white" tabindex="0"></button>
				<Separator type="Related" />
				<button class="preset-color pure" {disabled} on:click={setColorPresetSubtile} tabindex="-1">
					<div data-pure-tile="red" style="--pure-color: #ff0000; --pure-color-gray: #4c4c4c" title="Set to red" />
					<div data-pure-tile="yellow" style="--pure-color: #ffff00; --pure-color-gray: #e3e3e3" title="Set to yellow" />
					<div data-pure-tile="green" style="--pure-color: #00ff00; --pure-color-gray: #969696" title="Set to green" />
					<div data-pure-tile="cyan" style="--pure-color: #00ffff; --pure-color-gray: #b2b2b2" title="Set to cyan" />
					<div data-pure-tile="blue" style="--pure-color: #0000ff; --pure-color-gray: #1c1c1c" title="Set to blue" />
					<div data-pure-tile="magenta" style="--pure-color: #ff00ff; --pure-color-gray: #696969" title="Set to magenta" />
				</button>
				<Separator type="Related" />
				<IconButton icon="Eyedropper" size={24} {disabled} action={activateEyedropperSample} tooltip="Sample a pixel color from the document" />
			</LayoutRow>
		</LayoutCol>
	</LayoutRow>
</FloatingMenu>

<style lang="scss" global>
	.color-picker {
		--picker-size: 256px;
		--picker-circle-radius: 6px;

		.pickers-and-gradient {
			.pickers {
				.saturation-value-picker {
					width: var(--picker-size);
					background-blend-mode: multiply;
					background: linear-gradient(to bottom, #ffffff, #000000), linear-gradient(to right, #ffffff, var(--hue-color));
					position: relative;
				}

				.saturation-value-picker,
				.hue-picker,
				.alpha-picker {
					height: var(--picker-size);
					border-radius: 2px;
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
					--selection-needle-color: var(--hue-color-contrasting);
					background-blend-mode: screen;
					background:
						// Reds
						linear-gradient(to top, #ff0000ff calc(100% / 6), #ff000000 calc(200% / 6), #ff000000 calc(400% / 6), #ff0000ff calc(500% / 6)),
						// Greens
						linear-gradient(to top, #00ff0000 0%, #00ff00ff calc(100% / 6), #00ff00ff 50%, #00ff0000 calc(400% / 6)),
						// Blues
						linear-gradient(to top, #0000ff00 calc(200% / 6), #0000ffff 50%, #0000ffff calc(500% / 6), #0000ff00 100%);
				}

				.alpha-picker {
					--selection-needle-color: var(--new-color-contrasting);
					background-image: linear-gradient(to bottom, var(--opaque-color), transparent), var(--color-transparent-checkered-background);
					background-size:
						100% 100%,
						var(--color-transparent-checkered-background-size);
					background-position:
						0 0,
						var(--color-transparent-checkered-background-position);
					background-repeat: no-repeat, var(--color-transparent-checkered-background-repeat);
				}

				.selection-circle {
					position: absolute;
					left: 0;
					top: 0;
					width: 0;
					height: 0;
					pointer-events: none;

					&::after {
						content: "";
						display: block;
						position: relative;
						left: calc(-1 * var(--picker-circle-radius));
						top: calc(-1 * var(--picker-circle-radius));
						width: calc(var(--picker-circle-radius) * 2 + 1px);
						height: calc(var(--picker-circle-radius) * 2 + 1px);
						border-radius: 50%;
						border: 2px solid var(--opaque-color-contrasting);
						box-sizing: border-box;
					}
				}

				.selection-circle-alignment {
					position: absolute;
					pointer-events: none;

					&.saturation::before,
					&.saturation::after,
					&.value::before,
					&.value::after {
						content: "";
						position: absolute;
						background: var(--opaque-color-contrasting);
						width: 1px;
						height: 1px;
					}

					&.saturation {
						&::before {
							height: var(--picker-size);
							margin-top: calc(-1 * var(--picker-size) - var(--picker-circle-radius));
						}

						&::after {
							height: var(--picker-size);
							margin-top: var(--picker-circle-radius);
						}
					}

					&.value {
						&::before {
							width: var(--picker-size);
							margin-left: var(--picker-circle-radius);
						}

						&::after {
							width: var(--picker-size);
							margin-left: calc(-1 * var(--picker-size) - var(--picker-circle-radius));
						}
					}
				}

				.selection-needle {
					position: absolute;
					top: 0;
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
			}

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
					position: absolute;
					top: 0;
					bottom: 0;
					left: 0;
					right: 0;
					box-shadow: inset 0 0 0 1px rgba(var(--color-0-black-rgb), var(--outline-amount));
					pointer-events: none;
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

		&.disabled .pickers-and-gradient .pickers :is(.saturation-value-picker, .hue-picker, .alpha-picker),
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

	// paddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpadding
</style>
