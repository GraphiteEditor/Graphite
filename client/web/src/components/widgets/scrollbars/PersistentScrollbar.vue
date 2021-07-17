<template>
	<div class="persistent-scrollbar" :class="direction.toLowerCase()">
		<button class="arrow decrease"></button>
		<div class="scroll-track">
			<div class="scroll-click-area decrease" :style="[trackStart, preThumb, sides]"></div>
			<div class="scroll-thumb" :style="[thumbStart, thumbEnd, sides]"></div>
			<div class="scroll-click-area increase" :style="[postThumb, trackEnd, sides]"></div>
		</div>
		<button class="arrow increase"></button>
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
		}

		.arrow.increase {
			margin: 4px 3px;
			border-width: 8px 5px 0 5px;
			border-color: var(--color-5-dullgray) transparent transparent transparent;

			&:hover {
				border-color: var(--color-6-lowergray) transparent transparent transparent;
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
		}

		.arrow.increase {
			margin: 3px 4px;
			border-width: 5px 0 5px 8px;
			border-color: transparent transparent transparent var(--color-5-dullgray);

			&:hover {
				border-color: transparent transparent transparent var(--color-6-lowergray);
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
		trackStart(): { left: string } | { top: string } {
			return this.direction === ScrollbarDirection.Vertical ? { top: "0%" } : { left: "0%" };
		},
		preThumb(): { right: string } | { bottom: string } {
			const start = 25;

			return this.direction === ScrollbarDirection.Vertical ? { bottom: `${100 - start}%` } : { right: `${100 - start}%` };
		},
		thumbStart(): { left: string } | { top: string } {
			const start = 25;

			return this.direction === ScrollbarDirection.Vertical ? { top: `${start}%` } : { left: `${start}%` };
		},
		thumbEnd(): { right: string } | { bottom: string } {
			const end = 25;

			return this.direction === ScrollbarDirection.Vertical ? { bottom: `${end}%` } : { right: `${end}%` };
		},
		postThumb(): { left: string } | { top: string } {
			const end = 25;

			return this.direction === ScrollbarDirection.Vertical ? { top: `${100 - end}%` } : { left: `${100 - end}%` };
		},
		trackEnd(): { right: string } | { bottom: string } {
			return this.direction === ScrollbarDirection.Vertical ? { bottom: "0%" } : { right: "0%" };
		},
		sides(): { left: string; right: string } | { top: string; bottom: string } {
			return this.direction === ScrollbarDirection.Vertical ? { left: "0%", right: "0%" } : { top: "0%", bottom: "0%" };
		},
	},
	data() {
		return {
			ScrollbarDirection,
		};
	},
});
</script>
