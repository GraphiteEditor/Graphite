<template>
	<div class="popover-mount" v-if="open">
		<div class="tail left"></div>
		<div class="popover">
			<div class="popover-content" ref="popoverContent">
				<slot></slot>
			</div>
		</div>
	</div>
</template>

<style lang="scss">
.popover-mount {
	position: absolute;
	width: 0;
	height: 0;
	display: flex;
}

.tail {
	width: 0;
	height: 0;
	border-style: solid;
	z-index: 1;

	&.top {
		border-width: 0 6px 8px 6px;
		border-color: transparent transparent #222222e6 transparent;
		margin-left: -6px;
		margin-top: 2px;
	}

	&.bottom {
		border-width: 8px 6px 0 6px;
		border-color: #222222e6 transparent transparent transparent;
		margin-left: -6px;
		margin-bottom: 2px;
	}

	&.left {
		border-width: 6px 8px 6px 0;
		border-color: transparent #222222e6 transparent transparent;
		margin-top: -6px;
		margin-left: 2px;
	}

	&.right {
		border-width: 6px 0 6px 8px;
		border-color: transparent transparent transparent #222222e6;
		margin-top: -6px;
		margin-right: 2px;
	}
}

.popover {
	display: flex;
	align-items: center;

	.popover-content {
		background: #222222e6;
		box-shadow: #000 0 0 4px;
		border-radius: 4px;
		color: #eee;
		font-size: inherit;
		padding: 8px;
		z-index: 0;
		display: flex;
		// This `position: relative` is used to allow `top`/`right`/`bottom`/`left` properties to shift the content back from overflowing the workspace
		position: relative;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export default defineComponent({
	components: {},
	data() {
		return {
			open: false,
			mouseStillDown: false,
		};
	},
	updated() {
		const popoverContent = this.$refs.popoverContent as HTMLElement;
		const workspace = document.querySelector(".workspace");
		if (popoverContent && workspace) {
			const workspaceBounds = workspace.getBoundingClientRect();

			const popoverBounds = popoverContent.getBoundingClientRect();

			const bottomOffset = workspaceBounds.bottom - popoverBounds.bottom - 8;
			if (bottomOffset < 0) popoverContent.style.top = `${bottomOffset}px`;
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
