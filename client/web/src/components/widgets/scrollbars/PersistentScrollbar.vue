<template>
	<div class="persistent-scrollbar" :class="direction.toLowerCase()">
		<button class="arrow decrease" @mousedown="changePosition(-50)"></button>
		<div class="scroll-track" ref="scrollTrack" @mousedown="grabArea">
			<div class="scroll-thumb" @mousedown="grabHandle" :class="{ dragging: dragging }" ref="handle" :style="[thumbStart, thumbEnd, sides]"></div>
		</div>
		<button class="arrow increase" @click="changePosition(50)"></button>
	</div>
</template>

<style lang="scss">
.persistent-scrollbar {
	display: flex;
	flex: 1 1 100%;

	.arrow {
		flex: 0 0 auto;
		display: block;
		background: none;
		outline: none;
		border: none;
		border-style: solid;
		width: 0;
		height: 0;
		padding: 0;
	}

	.scroll-track {
		flex: 1 1 100%;
		position: relative;

		.scroll-thumb {
			position: absolute;
			border-radius: 4px;
			background: var(--color-5-dullgray);

			&:hover {
				background: var(--color-6-lowergray);
			}
			&.dragging {
				background: var(--color-accent-hover);
			}
		}

		.scroll-click-area {
			position: absolute;
		}
	}

	&.vertical {
		flex-direction: column;

		.arrow.decrease {
			margin: 4px 3px;
			border-width: 0 5px 8px 5px;
			border-color: transparent transparent var(--color-5-dullgray) transparent;

			&:hover {
				border-color: transparent transparent var(--color-6-lowergray) transparent;
			}
			&:active {
				border-color: transparent transparent var(--color-c-brightgray) transparent;
			}
		}

		.arrow.increase {
			margin: 4px 3px;
			border-width: 8px 5px 0 5px;
			border-color: var(--color-5-dullgray) transparent transparent transparent;

			&:hover {
				border-color: var(--color-6-lowergray) transparent transparent transparent;
			}
			&:active {
				border-color: var(--color-c-brightgray) transparent transparent transparent;
			}
		}
	}

	&.horizontal {
		flex-direction: row;

		.arrow.decrease {
			margin: 3px 4px;
			border-width: 5px 8px 5px 0;
			border-color: transparent var(--color-5-dullgray) transparent transparent;

			&:hover {
				border-color: transparent var(--color-6-lowergray) transparent transparent;
			}
			&:active {
				border-color: transparent var(--color-c-brightgray) transparent transparent;
			}
		}

		.arrow.increase {
			margin: 3px 4px;
			border-width: 5px 0 5px 8px;
			border-color: transparent transparent transparent var(--color-5-dullgray);

			&:hover {
				border-color: transparent transparent transparent var(--color-6-lowergray);
			}
			&:active {
				border-color: transparent transparent transparent var(--color-c-brightgray);
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

export enum ScrollbarDirection {
	"Horizontal" = "Horizontal",
	"Vertical" = "Vertical",
}

export default defineComponent({
	props: {
		direction: { type: String as PropType<ScrollbarDirection>, default: ScrollbarDirection.Vertical },
	},
	computed: {
		thumbStart(): { left: string } | { top: string } {
			const start = this.handlePosition - this.handleLength / 2;

			return this.direction === ScrollbarDirection.Vertical ? { top: `${start * 100}%` } : { left: `${start * 100}%` };
		},
		thumbEnd(): { right: string } | { bottom: string } {
			const end = 1 - this.handlePosition - this.handleLength / 2;

			return this.direction === ScrollbarDirection.Vertical ? { bottom: `${end * 100}%` } : { right: `${end * 100}%` };
		},
		sides(): { left: string; right: string } | { top: string; bottom: string } {
			return this.direction === ScrollbarDirection.Vertical ? { left: "0%", right: "0%" } : { top: "0%", bottom: "0%" };
		},
	},
	data() {
		return {
			ScrollbarDirection,
			handlePosition: 0.5,
			handleLength: 0.2,
			dragging: false,
			dragOffset: 0,
		};
	},
	mounted() {
		window.addEventListener("mouseup", () => {
			this.dragging = false;
		});
		window.addEventListener("mousemove", this.mouseMove);
	},
	watch: {
		handlePosition(newPos: number) {
			const clamped_position = Math.min(Math.max(newPos, this.handleLength / 2), 1 - this.handleLength / 2);
			this.handlePosition = clamped_position;
		},
	},
	methods: {
		trackLength(): number {
			const track = this.$refs.scrollTrack as HTMLElement;
			return this.direction === ScrollbarDirection.Vertical ? track.clientHeight - this.handleLength : track.clientWidth;
		},
		trackOffset(): number {
			const track = this.$refs.scrollTrack as HTMLElement;
			return this.direction === ScrollbarDirection.Vertical ? track.getBoundingClientRect().top : track.getBoundingClientRect().left;
		},
		updateHandlePosition(e: MouseEvent) {
			const position = this.direction === ScrollbarDirection.Vertical ? e.clientY : e.clientX;
			this.handlePosition = (position + this.dragOffset - this.trackOffset()) / this.trackLength();
		},
		grabHandle(e: MouseEvent) {
			if (!this.dragging) {
				this.dragging = true;
				const position = this.direction === ScrollbarDirection.Vertical ? e.clientY : e.clientX;
				this.dragOffset = this.handlePosition * this.trackLength() + this.trackOffset() - position;
				this.updateHandlePosition(e);
			}
		},
		grabArea(e: MouseEvent) {
			if (!this.dragging) {
				this.dragging = true;
				this.dragOffset = 0;
				this.updateHandlePosition(e);
			}
		},
		mouseUp() {
			this.dragging = false;
			this.dragOffset = 0;
		},
		mouseMove(e: MouseEvent) {
			if (this.dragging) {
				this.updateHandlePosition(e);
			}
		},
		changePosition(difference: number) {
			this.handlePosition += difference / this.trackLength();
		},
		updateBox(size: number, position: number) {
			this.handleLength = this.trackLength() / size;
			this.handlePosition = position / size;
		},
	},
});
</script>
