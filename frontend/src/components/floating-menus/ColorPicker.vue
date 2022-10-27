<template>
	<FloatingMenu class="color-picker" :open="open" @update:open="(isOpen) => emitOpenState(isOpen)" :direction="direction" :type="'Popover'">
		<LayoutRow
			:style="{
				'--new-color': colorFromHSV.toHexOptionalAlpha(),
				'--new-color-contrasting': colorFromHSV.contrastingColor(),
				'--initial-color': initialColor.toHexOptionalAlpha(),
				'--initial-color-contrasting': initialColor.contrastingColor(),
				'--hue-color': opaqueHueColor.toRgbCSS(),
				'--hue-color-contrasting': opaqueHueColor.contrastingColor(),
				'--opaque-color': (colorFromHSV.opaque() || black).toHexNoAlpha(),
				'--opaque-color-contrasting': (colorFromHSV.opaque() || black).contrastingColor(),
			}"
		>
			<LayoutCol class="saturation-value-picker" @pointerdown="(e: PointerEvent) => beginDrag(e)" data-saturation-value-picker>
				<div class="selection-circle" :style="{ top: `${(1 - value) * 100}%`, left: `${saturation * 100}%` }" v-if="!isNone"></div>
			</LayoutCol>
			<LayoutCol class="hue-picker" @pointerdown="(e: PointerEvent) => beginDrag(e)" data-hue-picker>
				<div class="selection-pincers" :style="{ top: `${(1 - hue) * 100}%` }" v-if="!isNone"></div>
			</LayoutCol>
			<LayoutCol class="opacity-picker" @pointerdown="(e: PointerEvent) => beginDrag(e)" data-opacity-picker>
				<div class="selection-pincers" :style="{ top: `${(1 - opacity) * 100}%` }" v-if="!isNone"></div>
			</LayoutCol>
			<LayoutCol class="details">
				<LayoutRow class="choice-preview" @click="() => swapColorWithInitial()">
					<LayoutCol class="new-color" :class="{ none: isNone }">
						<TextLabel>New</TextLabel>
					</LayoutCol>
					<LayoutCol class="initial-color" :class="{ none: initialIsNone }">
						<TextLabel>Initial</TextLabel>
					</LayoutCol>
				</LayoutRow>
				<DropdownInput :entries="colorSpaceChoices" :selectedIndex="0" :disabled="true" :tooltip="'Color Space and HDR (coming soon)'" />
				<LayoutRow>
					<TextLabel>Hex</TextLabel>
					<Separator />
					<LayoutRow>
						<TextInput :value="colorFromHSV.toHexOptionalAlpha() || '-'" @commitText="(value: string) => setColorCode(value)" :centered="true" />
					</LayoutRow>
				</LayoutRow>
				<LayoutRow>
					<TextLabel>RGB</TextLabel>
					<Separator />
					<LayoutRow>
						<template v-for="([channel, strength], index) in Object.entries(colorFromHSV.toRgb255() || { r: undefined, g: undefined, b: undefined })" :key="channel">
							<Separator :type="'Related'" v-if="index > 0" />
							<NumberInput :value="strength" @update:value="(value: number) => setColorRGB(channel as keyof RGB, value)" :min="0" :max="255" :centered="true" />
						</template>
					</LayoutRow>
				</LayoutRow>
				<LayoutRow>
					<TextLabel>HSV</TextLabel>
					<Separator />
					<LayoutRow>
						<template
							v-for="([channel, strength], index) in !isNone
								? Object.entries({ h: hue * 360, s: saturation * 100, v: value * 100 })
								: Object.entries({ h: undefined, s: undefined, v: undefined })"
							:key="channel"
						>
							<Separator :type="'Related'" v-if="index > 0" />
							<NumberInput
								:value="strength"
								@update:value="(value: number) => setColorHSV(channel as keyof HSV, value)"
								:min="0"
								:max="channel === 'h' ? 360 : 100"
								:unit="channel === 'h' ? '°' : '%'"
								:centered="true"
							/>
						</template>
					</LayoutRow>
				</LayoutRow>
				<NumberInput :label="'Opacity'" :value="!isNone ? opacity * 100 : undefined" @update:value="(value: number) => setColorOpacityPercent(value)" :min="0" :max="100" :unit="'%'" />
				<LayoutRow class="leftover-space"></LayoutRow>
				<LayoutRow>
					<button class="preset-color none" @click="() => setColorPreset('none')" v-if="allowNone"></button>
					<Separator :type="'Related'" v-if="allowNone" />
					<button class="preset-color black" @click="() => setColorPreset('black')"></button>
					<Separator :type="'Related'" />
					<button class="preset-color white" @click="() => setColorPreset('white')"></button>
					<Separator :type="'Related'" />
					<button class="preset-color pure" @click="(e: MouseEvent) => setColorPresetSubtile(e)">
						<div data-pure-tile="red" style="--pure-color: #ff0000; --pure-color-gray: #4c4c4c"></div>
						<div data-pure-tile="yellow" style="--pure-color: #ffff00; --pure-color-gray: #e3e3e3"></div>
						<div data-pure-tile="green" style="--pure-color: #00ff00; --pure-color-gray: #969696"></div>
						<div data-pure-tile="cyan" style="--pure-color: #00ffff; --pure-color-gray: #b2b2b2"></div>
						<div data-pure-tile="blue" style="--pure-color: #0000ff; --pure-color-gray: #1c1c1c"></div>
						<div data-pure-tile="magenta" style="--pure-color: #ff00ff; --pure-color-gray: #696969"></div>
					</button>
				</LayoutRow>
			</LayoutCol>
		</LayoutRow>
	</FloatingMenu>
</template>

<style lang="scss">
.color-picker {
	.saturation-value-picker {
		width: 256px;
		background-blend-mode: multiply;
		background: linear-gradient(to bottom, #ffffff, #000000), linear-gradient(to right, #ffffff, var(--hue-color));
		position: relative;
	}

	.saturation-value-picker,
	.hue-picker,
	.opacity-picker {
		height: 256px;
		position: relative;
		overflow: hidden;
	}

	.hue-picker,
	.opacity-picker {
		width: 24px;
		margin-left: 8px;
		position: relative;
	}

	.hue-picker {
		background-blend-mode: screen;
		background: linear-gradient(to top, #ff0000ff 16.666%, #ff000000 33.333%, #ff000000 66.666%, #ff0000ff 83.333%),
			linear-gradient(to top, #00ff0000 0%, #00ff00ff 16.666%, #00ff00ff 50%, #00ff0000 66.666%), linear-gradient(to top, #0000ff00 33.333%, #0000ffff 50%, #0000ffff 83.333%, #0000ff00 100%);
		--selection-pincers-color: var(--hue-color-contrasting);
	}

	.opacity-picker {
		background: linear-gradient(to bottom, var(--opaque-color), transparent);

		&::before {
			content: "";
			width: 100%;
			height: 100%;
			z-index: -1;
			position: relative;
			background: var(--transparent-checkered-background);
			background-size: var(--transparent-checkered-background-size);
			background-position: var(--transparent-checkered-background-position);
		}
		--selection-pincers-color: var(--new-color-contrasting);
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

	.selection-pincers {
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
			border-color: transparent transparent transparent var(--selection-pincers-color);
		}

		&::after {
			content: "";
			position: absolute;
			top: -4px;
			right: 0;
			border-style: solid;
			border-width: 4px 4px 4px 0;
			border-color: transparent var(--selection-pincers-color) transparent transparent;
		}
	}

	.details {
		margin-left: 16px;
		gap: 8px;

		> .layout-row {
			height: 24px;
			flex: 0 0 auto;

			&.leftover-space {
				flex: 1 1 100%;
			}
		}

		.choice-preview {
			flex: 0 0 auto;
			width: 208px;
			height: 32px;
			border-radius: 2px;
			border: 1px solid var(--color-0-black);
			box-sizing: border-box;
			overflow: hidden;

			.new-color {
				background: linear-gradient(var(--new-color), var(--new-color)), var(--transparent-checkered-background);

				.text-label {
					margin: 2px 8px;
					color: var(--new-color-contrasting);
				}
			}

			.initial-color {
				background: linear-gradient(var(--initial-color), var(--initial-color)), var(--transparent-checkered-background);

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
				background-size: var(--transparent-checkered-background-size);
				background-position: var(--transparent-checkered-background-position);

				&.none {
					background: var(--color-none-slash);
				}
			}
		}

		.preset-color {
			border: none;
			outline: none;
			padding: 0;
			border-radius: 2px;
			width: calc(48px + (48px + 4px) / 2);
			height: 24px;

			&.none {
				background: var(--color-none-slash);

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
				transition: background-color 0.5s ease;

				div {
					display: inline-block;
					width: calc(100% / 3);
					height: 50%;
					// For the least jarring luminance conversion, these colors are derived by placing a black layer with the "desaturate" blend mode over the colors.
					// We don't use the CSS `filter: grayscale(1);` property because it produces overly dark tones for bright colors with a noticeable jump on hover.
					background: var(--pure-color-gray);
				}
			}

			&:hover.pure div,
			&:hover ~ .pure div,
			& + .separator:hover ~ .pure div {
				background: var(--pure-color);
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

import { clamp } from "@/utility-functions/math";
import type { HSV, RGB } from "@/wasm-communication/messages";
import { Color } from "@/wasm-communication/messages";

import FloatingMenu, { type MenuDirection } from "@/components/layout/FloatingMenu.vue";
import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import DropdownInput from "@/components/widgets/inputs/DropdownInput.vue";
import NumberInput from "@/components/widgets/inputs/NumberInput.vue";
import TextInput from "@/components/widgets/inputs/TextInput.vue";
import Separator from "@/components/widgets/labels/Separator.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";

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

const COLOR_SPACE_CHOICES = [[{ label: "sRGB" }]];

export default defineComponent({
	emits: ["update:color", "update:open"],
	props: {
		color: { type: Object as PropType<Color>, required: true },
		allowNone: { type: Boolean as PropType<boolean>, default: false },
		allowTransparency: { type: Boolean as PropType<boolean>, default: false }, // TODO: Implement this
		direction: { type: String as PropType<MenuDirection>, default: "Bottom" },
		// TODO: See if this should be made to follow the pattern of DropdownInput.vue so this could be removed
		open: { type: Boolean as PropType<boolean>, required: true },
	},
	data() {
		const hsvaOrNone = this.color.toHSVA();
		const hsva = hsvaOrNone || { h: 0, s: 0, v: 0, a: 1 };

		return {
			hue: hsva.h,
			saturation: hsva.s,
			value: hsva.v,
			opacity: hsva.a,
			isNone: hsvaOrNone === undefined,
			initialHue: hsva.h,
			initialSaturation: hsva.s,
			initialValue: hsva.v,
			initialOpacity: hsva.a,
			initialIsNone: hsvaOrNone === undefined,
			draggingPickerTrack: undefined as HTMLDivElement | undefined,
			colorSpaceChoices: COLOR_SPACE_CHOICES,
		};
	},
	computed: {
		opaqueHueColor(): Color {
			return new Color({ h: this.hue, s: 1, v: 1, a: 1 });
		},
		colorFromHSV(): Color {
			return new Color({ h: this.hue, s: this.saturation, v: this.value, a: this.opacity });
		},
		initialColor(): Color {
			return new Color({ h: this.initialHue, s: this.initialSaturation, v: this.initialValue, a: this.initialOpacity });
		},
		black(): Color {
			return new Color(0, 0, 0, 1);
		},
	},
	watch: {
		// Called only when `open` is changed from outside this component (with v-model)
		open(isOpen: boolean) {
			if (isOpen) this.setInitialHsvAndOpacity(this.hue, this.saturation, this.value, this.opacity, this.isNone);
		},
		// Called only when `color` is changed from outside this component (with v-model)
		color(newColor: Color) {
			const hsva = newColor.toHSVA();

			if (hsva !== undefined) {
				if (hsva.h !== 0 && hsva.s !== 0 && hsva.v !== 0) this.hue = hsva.h;
				if (hsva.v !== 0) this.saturation = hsva.s;
				this.value = hsva.v;
				this.opacity = hsva.a;
				this.isNone = false;
			} else {
				this.setNewHsvAndOpacity(0, 0, 0, 0, true);
			}
		},
	},
	methods: {
		beginDrag(e: PointerEvent) {
			const target = (e.target || undefined) as HTMLElement | undefined;
			this.draggingPickerTrack = target?.closest("[data-saturation-value-picker], [data-hue-picker], [data-opacity-picker]") || undefined;

			this.addEvents();
			this.onPointerMove(e);
		},
		onPointerMove(e: PointerEvent) {
			if (this.draggingPickerTrack?.hasAttribute("data-saturation-value-picker")) {
				const rectangle = this.draggingPickerTrack.getBoundingClientRect();

				this.saturation = clamp((e.clientX - rectangle.left) / rectangle.width, 0, 1);
				this.value = clamp(1 - (e.clientY - rectangle.top) / rectangle.height, 0, 1);
			} else if (this.draggingPickerTrack?.hasAttribute("data-hue-picker")) {
				const rectangle = this.draggingPickerTrack.getBoundingClientRect();

				this.hue = clamp(1 - (e.clientY - rectangle.top) / rectangle.height, 0, 1);
			} else if (this.draggingPickerTrack?.hasAttribute("data-opacity-picker")) {
				const rectangle = this.draggingPickerTrack.getBoundingClientRect();

				this.opacity = clamp(1 - (e.clientY - rectangle.top) / rectangle.height, 0, 1);
			}

			// Just in case the mouseup event is lost
			if (e.buttons === 0) this.removeEvents();

			const newColor = new Color({ h: this.hue, s: this.saturation, v: this.value, a: this.opacity });
			this.setColor(newColor);
		},
		onPointerUp() {
			this.removeEvents();
		},
		emitOpenState(isOpen: boolean) {
			this.$emit("update:open", isOpen);
		},
		addEvents() {
			document.addEventListener("pointermove", this.onPointerMove);
			document.addEventListener("pointerup", this.onPointerUp);
		},
		removeEvents() {
			document.removeEventListener("pointermove", this.onPointerMove);
			document.removeEventListener("pointerup", this.onPointerUp);
		},
		setColor(newColor: Color) {
			this.$emit("update:color", newColor);
		},
		swapColorWithInitial() {
			const initial = this.initialColor;

			const tempHue = this.hue;
			const tempSaturation = this.saturation;
			const tempValue = this.value;
			const tempOpacity = this.opacity;
			const tempIsNone = this.isNone;

			this.setNewHsvAndOpacity(this.initialHue, this.initialSaturation, this.initialValue, this.initialOpacity, this.initialIsNone);
			this.setInitialHsvAndOpacity(tempHue, tempSaturation, tempValue, tempOpacity, tempIsNone);

			this.setColor(initial);
		},
		setColorCode(colorCode: string) {
			const newColor = Color.fromCSS(colorCode);
			if (newColor) this.setColor(newColor);
		},
		setColorRGB(channel: keyof RGB, strength: number) {
			if (channel === "r") this.setColor(new Color(strength / 255, this.colorFromHSV.green, this.colorFromHSV.blue, this.colorFromHSV.alpha));
			else if (channel === "g") this.setColor(new Color(this.colorFromHSV.red, strength / 255, this.colorFromHSV.blue, this.colorFromHSV.alpha));
			else if (channel === "b") this.setColor(new Color(this.colorFromHSV.red, this.colorFromHSV.green, strength / 255, this.colorFromHSV.alpha));
		},
		setColorHSV(channel: keyof HSV, strength: number) {
			if (channel === "h") this.setColor(new Color({ h: strength / 360, s: this.saturation, v: this.value, a: this.opacity }));
			if (channel === "s") this.setColor(new Color({ h: this.hue, s: strength / 100, v: this.value, a: this.opacity }));
			if (channel === "v") this.setColor(new Color({ h: this.hue, s: this.saturation, v: strength / 100, a: this.opacity }));
		},
		setColorOpacityPercent(opacity: number) {
			this.setColor(new Color({ h: this.hue, s: this.saturation, v: this.value, a: opacity / 100 }));
		},
		setColorPresetSubtile(e: MouseEvent) {
			const clickedTile = e.target as HTMLDivElement | undefined;
			const tileColor = clickedTile?.getAttribute("data-pure-tile") || undefined;

			if (tileColor) this.setColorPreset(tileColor as PresetColors);
		},
		setColorPreset(preset: PresetColors) {
			if (preset === "none") {
				this.setNewHsvAndOpacity(0, 0, 0, 0, true);
				this.setColor(new Color("none"));
				return;
			}

			const presetColor = new Color(...PURE_COLORS[preset], 1);
			const hsva = presetColor.toHSVA() || { h: 0, s: 0, v: 0, a: 0 };

			this.setNewHsvAndOpacity(hsva.h, hsva.s, hsva.v, hsva.a, false);
			this.setColor(presetColor);
		},
		setNewHsvAndOpacity(hue: number, saturation: number, value: number, opacity: number, isNone: boolean) {
			this.hue = hue;
			this.saturation = saturation;
			this.value = value;
			this.opacity = opacity;
			this.isNone = isNone;
		},
		setInitialHsvAndOpacity(hue: number, saturation: number, value: number, opacity: number, isNone: boolean) {
			this.initialHue = hue;
			this.initialSaturation = saturation;
			this.initialValue = value;
			this.initialOpacity = opacity;
			this.initialIsNone = isNone;
		},
	},
	unmounted() {
		this.removeEvents();
	},
	components: {
		FloatingMenu,
		LayoutCol,
		LayoutRow,
		TextLabel,
		DropdownInput,
		NumberInput,
		TextInput,
		Separator,
	},
});
</script>
