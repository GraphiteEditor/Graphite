<template>
	<div class="canvas-ruler" :class="direction.toLowerCase()" ref="rulerRef">
		<div class="marks">
			<div class="mark" v-for="(mark, index) in marks" :key="index" :style="markStyle(mark)"></div>
		</div>
	</div>
</template>

<style lang="scss">
.canvas-ruler {
	flex: 1 1 100%;
	background: var(--color-5-dullgray);

	&.vertical {
		width: 16px;

		.marks {
			flex-direction: column;

			.mark {
				height: 1px;
			}
		}
	}

	&.horizontal {
		height: 16px;

		.marks {
			flex-direction: row;

			.mark {
				width: 1px;
			}
		}
	}

	.marks {
		width: 100%;
		height: 100%;
		position: relative;
		overflow: hidden;
		display: flex;
		align-items: flex-end;

		.mark {
			position: absolute;
			background: var(--color-7-middlegray);
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

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
		marks(): Array<{ location: number; length: number }> {
			const markLocations = [];
			const divisions = this.majorMarkSpacing / this.mediumDivisions / this.minorDivisions;

			let offsetStart = this.origin % this.majorMarkSpacing;
			if (offsetStart < this.majorMarkSpacing) offsetStart -= this.majorMarkSpacing;

			const majorMarksFrequency = this.mediumDivisions * this.minorDivisions;
			let i = 0;
			for (let position = offsetStart; position < this.rulerLength; position += divisions) {
				let length = 4;
				if (i % majorMarksFrequency === 0) length = 16;
				else if (i % this.minorDivisions === 0) length = 8;
				i += 1;

				markLocations.push({ location: Math.round(position), length });
			}

			return markLocations;
		},
	},
	methods: {
		markStyle(mark: { location: number; length: number }) {
			const isVertical = this.direction === RulerDirection.Vertical;
			return isVertical ? { top: `${mark.location}px`, width: `${mark.length}px` } : { left: `${mark.location}px`, height: `${mark.length}px` };
		},
		handleResize() {
			if (!this.$refs.rulerRef) return;

			const rulerElement = this.$refs.rulerRef as HTMLElement;
			const isVertical = this.direction === RulerDirection.Vertical;

			const newLength = isVertical ? rulerElement.clientHeight : rulerElement.clientWidth;
			const roundedUp = (Math.floor(newLength / this.majorMarkSpacing) + 1) * this.majorMarkSpacing;

			if (roundedUp !== this.rulerLength) this.rulerLength = roundedUp;
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
			RulerDirection,
		};
	},
});
</script>
