<template>
	<div class="popover" :class="direction.toLowerCase()" v-if="open">
		<div class="tail"></div>
		<div class="popover-container" ref="popoverContainer">
			<div class="popover-content" ref="popoverContent">
				<slot></slot>
			</div>
		</div>
	</div>
</template>

<style lang="scss">
@use "sass:color";

.popover {
	position: absolute;
	width: 0;
	height: 0;
	display: flex;
	// Overlays begin at a z-index of 1000
	z-index: 1000;

	&.top,
	&.bottom {
		flex-direction: column;
	}
}

.tail {
	width: 0;
	height: 0;
	border-style: solid;
	// Put the tail above the popover's shadow
	z-index: 1;
	// Draw over the application without being clipped by the containing panel's `overflow: hidden`
	position: fixed;

	.top > & {
		border-width: 8px 6px 0 6px;
		border-color: var(--popover-opacity-color-2-mildblack) transparent transparent transparent;
		margin-left: -6px;
		margin-bottom: 2px;
	}

	.bottom > & {
		border-width: 0 6px 8px 6px;
		border-color: transparent transparent var(--popover-opacity-color-2-mildblack) transparent;
		margin-left: -6px;
		margin-top: 2px;
	}

	.left > & {
		border-width: 6px 0 6px 8px;
		border-color: transparent transparent transparent var(--popover-opacity-color-2-mildblack);
		margin-top: -6px;
		margin-right: 2px;
	}

	.right > & {
		border-width: 6px 8px 6px 0;
		border-color: transparent var(--popover-opacity-color-2-mildblack) transparent transparent;
		margin-top: -6px;
		margin-left: 2px;
	}
}

.popover-container {
	display: flex;

	.top > & {
		justify-content: center;
		margin-bottom: 10px;
	}

	.bottom > & {
		justify-content: center;
		margin-top: 10px;
	}

	.left > & {
		align-items: center;
		margin-right: 10px;
	}

	.right > & {
		align-items: center;
		margin-left: 10px;
	}

	.popover-content {
		background: var(--popover-opacity-color-2-mildblack);
		box-shadow: var(--color-0-black) 0 0 4px;
		border-radius: 4px;
		color: var(--color-e-nearwhite);
		font-size: inherit;
		padding: 8px;
		z-index: 0;
		display: flex;
		flex-direction: column;
		// Draw over the application without being clipped by the containing panel's `overflow: hidden`
		position: fixed;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export enum PopoverDirection {
	Top = "Top",
	Bottom = "Bottom",
	Left = "Left",
	Right = "Right",
}

export default defineComponent({
	components: {},
	props: {
		direction: { type: String, default: PopoverDirection.Bottom },
	},
	data() {
		return {
			open: false,
			mouseStillDown: false,
			PopoverDirection,
		};
	},
	updated() {
		const popoverContainer = this.$refs.popoverContainer as HTMLElement;
		const popoverContent = this.$refs.popoverContent as HTMLElement;
		const workspace = document.querySelector(".workspace");

		if (popoverContent && workspace) {
			const workspaceBounds = workspace.getBoundingClientRect();
			const popoverBounds = popoverContent.getBoundingClientRect();

			if (this.direction === PopoverDirection.Left || this.direction === PopoverDirection.Right) {
				const topOffset = popoverBounds.top - workspaceBounds.top - 8;
				if (topOffset < 0) popoverContainer.style.transform = `translate(0, ${-topOffset}px)`;

				const bottomOffset = workspaceBounds.bottom - popoverBounds.bottom - 8;
				if (bottomOffset < 0) popoverContainer.style.transform = `translate(0, ${bottomOffset}px)`;
			}

			if (this.direction === PopoverDirection.Top || this.direction === PopoverDirection.Bottom) {
				const leftOffset = popoverBounds.left - workspaceBounds.left - 8;
				if (leftOffset < 0) popoverContainer.style.transform = `translate(${-leftOffset}px, 0)`;

				const rightOffset = workspaceBounds.right - popoverBounds.right - 8;
				if (rightOffset < 0) popoverContainer.style.transform = `translate(${rightOffset}px, 0)`;
			}
		}
	},
	methods: {
		setOpen() {
			this.open = true;
		},
		setClosed() {
			this.open = false;
		},
		mouseMoveHandler(e: MouseEvent) {
			const MOUSE_STRAY_DISTANCE = 100;

			// Close the popover if the mouse has strayed far enough from its bounds
			if (this.isMouseEventOutsidePopover(e, MOUSE_STRAY_DISTANCE)) {
				this.setClosed();
			}

			// eslint-disable-next-line no-bitwise
			const eventIncludesLmb = Boolean(e.buttons & 1);

			// Clean up any messes from lost mouseup events
			if (!this.open && !eventIncludesLmb) {
				this.mouseStillDown = false;
				window.removeEventListener("mouseup", this.mouseUpHandler);
			}
		},
		mouseDownHandler(e: MouseEvent) {
			// Close the popover if the mouse clicked outside the popover (but within stray distance)
			if (this.isMouseEventOutsidePopover(e)) {
				this.setClosed();

				// Track if the left mouse button is now down so its later click event can be canceled
				const eventIsForLmb = e.button === 0;
				if (eventIsForLmb) this.mouseStillDown = true;
			}
		},
		mouseUpHandler(e: MouseEvent) {
			const eventIsForLmb = e.button === 0;

			if (this.mouseStillDown && eventIsForLmb) {
				// Clean up self
				this.mouseStillDown = false;
				window.removeEventListener("mouseup", this.mouseUpHandler);

				// Prevent the click event from firing, which would normally occur right after this mouseup event
				window.addEventListener("click", this.clickHandlerCapture, true);
			}
		},
		clickHandlerCapture(e: MouseEvent) {
			// Stop the click event from reopening this popover if the click event targets the popover's button
			e.stopPropagation();

			// Clean up self
			window.removeEventListener("click", this.clickHandlerCapture, true);
		},
		isMouseEventOutsidePopover(e: MouseEvent, extraDistanceAllowed = 0): boolean {
			const popoverContent = this.$refs.popoverContent as HTMLElement;
			const popoverBounds = popoverContent.getBoundingClientRect();

			if (popoverBounds.left - e.clientX >= extraDistanceAllowed) return true;
			if (e.clientX - popoverBounds.right >= extraDistanceAllowed) return true;
			if (popoverBounds.top - e.clientY >= extraDistanceAllowed) return true;
			if (e.clientY - popoverBounds.bottom >= extraDistanceAllowed) return true;

			return false;
		},
	},
	watch: {
		open(newState: boolean, oldState: boolean) {
			if (newState && !oldState) {
				// Close popover if mouse strays far enough away
				window.addEventListener("mousemove", this.mouseMoveHandler);

				// Close popover if mouse is outside (but within stray distance)
				window.addEventListener("mousedown", this.mouseDownHandler);

				// Cancel the subsequent click event to prevent the popover from reopening if the popover's button is the click event target
				window.addEventListener("mouseup", this.mouseUpHandler);
			}
			if (!newState && oldState) {
				window.removeEventListener("mousemove", this.mouseMoveHandler);
				window.removeEventListener("mousedown", this.mouseDownHandler);
			}
		},
	},
});
</script>
