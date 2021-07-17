<template>
	<div class="canvas-ruler" :class="direction.toLowerCase()" ref="rulerRef">
		<svg :style="svgBounds">
			<path :d="svgPath" />
		</svg>
	</div>
</template>

<style lang="scss">
.canvas-ruler {
	flex: 1 1 100%;
	background: var(--color-5-dullgray);
	overflow: hidden;
	position: relative;

	&.vertical {
		width: 16px;
	}

	&.horizontal {
		height: 16px;
	}

	svg {
		position: absolute;

		path {
			stroke-width: 1px;
			stroke: var(--color-7-middlegray);
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

const RULER_THICKNESS = 16;

export enum RulerDirection {
	"Horizontal" = "Horizontal",
	"Vertical" = "Vertical",
}

export default defineComponent({
	props: {
		direction: { type: String as PropType<RulerDirection>, default: RulerDirection.Vertical },
		origin: { type: Number, required: true },
		majorMarkSpacing: { type: Number, required: true },
		mediumDivisions: { type: Number, default: 5 },
		minorDivisions: { type: Number, default: 2 },
	},
	computed: {
		svgPath(): string {
			const isVertical = this.direction === RulerDirection.Vertical;
			const lineDirection = isVertical ? "H" : "V";

			let offsetStart = this.origin % this.majorMarkSpacing;
			if (offsetStart < this.majorMarkSpacing) offsetStart -= this.majorMarkSpacing;

			const divisions = this.majorMarkSpacing / this.mediumDivisions / this.minorDivisions;
			const majorMarksFrequency = this.mediumDivisions * this.minorDivisions;

			let dPathAttribute = "";
			let i = 0;
			for (let location = offsetStart; location < this.rulerLength; location += divisions) {
				let length = RULER_THICKNESS / 4;
				if (i % majorMarksFrequency === 0) length = RULER_THICKNESS;
				else if (i % this.minorDivisions === 0) length = RULER_THICKNESS / 2;
				i += 1;

				const destination = Math.round(location) + 0.5;
				const startPoint = isVertical ? `${RULER_THICKNESS - length},${destination}` : `${destination},${RULER_THICKNESS - length}`;
				dPathAttribute += `M${startPoint}${lineDirection}${RULER_THICKNESS} `;
			}

			return dPathAttribute;
		},
	},
	methods: {
		handleResize() {
			if (!this.$refs.rulerRef) return;

			const rulerElement = this.$refs.rulerRef as HTMLElement;
			const isVertical = this.direction === RulerDirection.Vertical;

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
	mounted() {
		window.addEventListener("resize", this.handleResize);
		this.handleResize();
	},
	beforeUnmount() {
		window.removeEventListener("resize", this.handleResize);
	},
	data() {
		return {
			rulerLength: 0,
			svgBounds: { width: "0px", height: "0px" },
			RulerDirection,
		};
	},
});
</script>
