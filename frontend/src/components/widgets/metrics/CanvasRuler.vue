<template>
	<div class="canvas-ruler" :class="direction.toLowerCase()" ref="rulerRef">
		<svg :style="svgBounds">
			<path :d="svgPath" />
			<text v-for="(svgText, index) in svgTexts" :key="index" :transform="svgText.transform">{{ svgText.text }}</text>
		</svg>
	</div>
</template>

<style lang="scss">
.canvas-ruler {
	flex: 1 1 100%;
	background: var(--color-4-dimgray);
	overflow: hidden;
	position: relative;

	&.horizontal {
		height: 16px;
	}

	&.vertical {
		width: 16px;

		svg text {
			text-anchor: end;
		}
	}

	svg {
		position: absolute;

		path {
			stroke-width: 1px;
			stroke: var(--color-7-middlegray);
		}

		text {
			font-size: 12px;
			fill: var(--color-8-uppergray);
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

const RULER_THICKNESS = 16;
const MAJOR_MARK_THICKNESS = 16;
const MEDIUM_MARK_THICKNESS = 6;
const MINOR_MARK_THICKNESS = 3;

export type RulerDirection = "Horizontal" | "Vertical";

// Modulo function that works for negative numbers, unlike the JS % operator
const mod = (n: number, m: number): number => {
	const remainder = n % m;
	return Math.floor(remainder >= 0 ? remainder : remainder + m);
};

export default defineComponent({
	props: {
		direction: { type: String as PropType<RulerDirection>, default: "Vertical" },
		origin: { type: Number as PropType<number>, required: true },
		numberInterval: { type: Number as PropType<number>, required: true },
		majorMarkSpacing: { type: Number as PropType<number>, required: true },
		mediumDivisions: { type: Number as PropType<number>, default: 5 },
		minorDivisions: { type: Number as PropType<number>, default: 2 },
	},
	computed: {
		svgPath(): string {
			const isVertical = this.direction === "Vertical";
			const lineDirection = isVertical ? "H" : "V";

			const offsetStart = mod(this.origin, this.majorMarkSpacing);
			const shiftedOffsetStart = offsetStart - this.majorMarkSpacing;

			const divisions = this.majorMarkSpacing / this.mediumDivisions / this.minorDivisions;
			const majorMarksFrequency = this.mediumDivisions * this.minorDivisions;

			let dPathAttribute = "";
			let i = 0;
			for (let location = shiftedOffsetStart; location < this.rulerLength; location += divisions) {
				let length;
				if (i % majorMarksFrequency === 0) length = MAJOR_MARK_THICKNESS;
				else if (i % this.minorDivisions === 0) length = MEDIUM_MARK_THICKNESS;
				else length = MINOR_MARK_THICKNESS;
				i += 1;

				const destination = Math.round(location) + 0.5;
				const startPoint = isVertical ? `${RULER_THICKNESS - length},${destination}` : `${destination},${RULER_THICKNESS - length}`;
				dPathAttribute += `M${startPoint}${lineDirection}${RULER_THICKNESS} `;
			}

			return dPathAttribute;
		},
		svgTexts(): { transform: string; text: number }[] {
			const isVertical = this.direction === "Vertical";

			const offsetStart = mod(this.origin, this.majorMarkSpacing);
			const shiftedOffsetStart = offsetStart - this.majorMarkSpacing;

			const svgTextCoordinates = [];

			let text = (Math.ceil(-this.origin / this.majorMarkSpacing) - 1) * this.numberInterval;

			for (let location = shiftedOffsetStart; location < this.rulerLength; location += this.majorMarkSpacing) {
				const destination = Math.round(location);
				const x = isVertical ? 9 : destination + 2;
				const y = isVertical ? destination + 1 : 9;

				let transform = `translate(${x} ${y})`;
				if (isVertical) transform += " rotate(270)";

				svgTextCoordinates.push({ transform, text });

				text += this.numberInterval;
			}

			return svgTextCoordinates;
		},
	},
	methods: {
		resize() {
			if (!this.$refs.rulerRef) return;

			const rulerElement = this.$refs.rulerRef as HTMLElement;
			const isVertical = this.direction === "Vertical";

			const newLength = isVertical ? rulerElement.clientHeight : rulerElement.clientWidth;
			const roundedUp = (Math.floor(newLength / this.majorMarkSpacing) + 1) * this.majorMarkSpacing;

			if (roundedUp !== this.rulerLength) {
				this.rulerLength = roundedUp;
				const thickness = `${RULER_THICKNESS}px`;
				const length = `${roundedUp}px`;
				this.svgBounds = isVertical ? { width: thickness, height: length } : { width: length, height: thickness };
			}
		},
	},
	data() {
		return {
			rulerLength: 0,
			svgBounds: { width: "0px", height: "0px" },
		};
	},
});
</script>
