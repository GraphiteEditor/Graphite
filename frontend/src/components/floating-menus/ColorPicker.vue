<template>
	<FloatingMenu class="color-picker" :open="open" @update:open="(isOpen) => emitOpenState(isOpen)" :direction="direction" :type="'Popover'">
		<LayoutRow
			:style="{
				'--new-color': color.toRgbaCSS(),
				'--new-color-contrasting': color.contrastingColor(),
				'--initial-color': initialColor.toRgbaCSS(),
				'--initial-color-contrasting': initialColor.contrastingColor(),
				'--hue-color': opaqueHueColor.toRgbCSS(),
				'--hue-color-contrasting': opaqueHueColor.contrastingColor(),
				'--opaque-color': color.toRgbaCSS(),
				'--opaque-color-contrasting': color.opaque().contrastingColor(),
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
				<LayoutRow class="choice-preview">
					<LayoutCol class="new-color">
						<TextLabel>New</TextLabel>
					</LayoutCol>
					<LayoutCol class="initial-color">
						<TextLabel>Initial</TextLabel>
					</LayoutCol>
				</LayoutRow>
				<DropdownInput :entries="colorSpaceChoices" :selectedIndex="0" :disabled="true" :tooltip="'Color spaces and HDR (coming soon)'" />
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
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

import { clamp } from "@/utility-functions/math";
import { Color } from "@/wasm-communication/messages";

import FloatingMenu, { type MenuDirection } from "@/components/layout/FloatingMenu.vue";
import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import DropdownInput from "@/components/widgets/inputs/DropdownInput.vue";
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
			initialColor: this.color,
			draggingPickerTrack: undefined as HTMLDivElement | undefined,
			hue: hsva.h,
			saturation: hsva.s,
			value: hsva.v,
			opacity: hsva.a,
			colorSpaceChoices: COLOR_SPACE_CHOICES,
		};
	},
	computed: {
		opaqueHueColor(): Color {
			return new Color({ h: this.hue, s: 1, v: 1, a: 1 });
		},
	},
	watch: {
		open(state) {
			if (state) this.initialColor = this.color;
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

			// The `color` prop's watcher calls `this.updateColor()`
			this.$emit("update:color", new Color({ h: this.hue, s: this.saturation, v: this.value, a: this.opacity }));
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
	},
});
</script>
