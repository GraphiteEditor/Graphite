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
				'--opaque-color': colorFromHSV.opaque().toHexNoAlpha(),
				'--opaque-color-contrasting': colorFromHSV.opaque().contrastingColor(),
			}"
		>
			<LayoutCol class="saturation-value-picker" @pointerdown="(e: PointerEvent) => beginDrag(e)" data-saturation-value-picker>
				<div class="selection-circle" :style="{ top: `${(1 - value) * 100}%`, left: `${saturation * 100}%` }"></div>
			</LayoutCol>
			<LayoutCol class="hue-picker" @pointerdown="(e: PointerEvent) => beginDrag(e)" data-hue-picker>
				<div class="selection-pincers" :style="{ top: `${(1 - hue) * 100}%` }"></div>
			</LayoutCol>
			<LayoutCol class="opacity-picker" @pointerdown="(e: PointerEvent) => beginDrag(e)" data-opacity-picker>
				<div class="selection-pincers" :style="{ top: `${(1 - opacity) * 100}%` }"></div>
			</LayoutCol>
			<LayoutCol class="details">
				<LayoutRow class="choice-preview" @click="() => swapColorWithInitial()">
					<LayoutCol class="new-color">
						<TextLabel>New</TextLabel>
					</LayoutCol>
					<LayoutCol class="initial-color">
						<TextLabel>Initial</TextLabel>
					</LayoutCol>
				</LayoutRow>
				<DropdownInput :entries="colorSpaceChoices" :selectedIndex="0" :disabled="true" :tooltip="'Color Space and HDR (coming soon)'" />
				<LayoutRow>
					<TextLabel>Hex</TextLabel>
					<Separator />
					<LayoutRow>
						<TextInput :value="colorFromHSV.toHexOptionalAlpha()" @commitText="(value: string) => setColorCode(value)" :centered="true" />
					</LayoutRow>
				</LayoutRow>
				<LayoutRow>
					<TextLabel>RGB</TextLabel>
					<Separator />
					<LayoutRow>
						<template v-for="([channel, strength], index) in Object.entries(colorFromHSV.toRgb255())" :key="channel">
							<Separator :type="'Related'" v-if="index > 0" />
							<NumberInput :value="strength" @update:value="(value: number) => setColorRGB(channel as keyof RGB, value)" :min="0" :max="255" :centered="true" />
						</template>
					</LayoutRow>
				</LayoutRow>
				<LayoutRow>
					<TextLabel>HSV</TextLabel>
					<Separator />
					<LayoutRow>
						<template v-for="([channel, strength], index) in Object.entries({ h: hue * 360, s: saturation * 100, v: value * 100 })" :key="channel">
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
				<NumberInput :label="'Opacity'" :value="opacity * 100" @update:value="(value: number) => setColorOpacityPercent(value)" :min="0" :max="100" :unit="'%'" />
				<LayoutRow class="leftover-space"></LayoutRow>
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
			}
		}

		> .layout-row {
			height: 24px;
			flex: 0 0 auto;

			&.leftover-space {
				flex: 1 1 100%;
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

const COLOR_SPACE_CHOICES = [[{ label: "sRGB" }]];

export default defineComponent({
	emits: ["update:color", "update:open"],
	props: {
		color: { type: Object as PropType<Color>, required: true },
		open: { type: Boolean as PropType<boolean>, required: true },
		direction: { type: String as PropType<MenuDirection>, default: "Bottom" },
	},
	data() {
		const hsva = this.color.toHSVA();

		return {
			hue: hsva.h,
			saturation: hsva.s,
			value: hsva.v,
			opacity: hsva.a,
			initialHue: hsva.h,
			initialSaturation: hsva.s,
			initialValue: hsva.v,
			initialOpacity: hsva.a,
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
	},
	watch: {
		// Called only when `open` is changed from outside this component (with v-model)
		open(state) {
			if (state) {
				this.initialHue = this.hue;
				this.initialSaturation = this.saturation;
				this.initialValue = this.value;
				this.initialOpacity = this.opacity;
			}
		},
		// Called only when `color` is changed from outside this component (with v-model)
		color(newColor) {
			const hsva = newColor.toHSVA();

			if (hsva.h !== 0 && hsva.s !== 0 && hsva.v !== 0) this.hue = hsva.h;
			if (hsva.v !== 0) this.saturation = hsva.s;
			this.value = hsva.v;
			this.opacity = hsva.a;
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

			this.hue = this.initialHue;
			this.saturation = this.initialSaturation;
			this.value = this.initialValue;
			this.opacity = this.initialOpacity;

			this.initialHue = tempHue;
			this.initialSaturation = tempSaturation;
			this.initialValue = tempValue;
			this.initialOpacity = tempOpacity;

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
