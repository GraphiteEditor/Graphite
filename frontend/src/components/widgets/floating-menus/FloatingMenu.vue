<template>
	<div class="floating-menu" :class="[direction.toLowerCase(), type.toLowerCase()]" v-if="open || type === MenuType.Dialog" ref="floatingMenu">
		<div class="tail" v-if="type === MenuType.Popover"></div>
		<div class="floating-menu-container" ref="floatingMenuContainer">
			<div class="floating-menu-content" :class="{ 'scrollable-y': scrollable }" ref="floatingMenuContent" :style="floatingMenuContentStyle">
				<slot></slot>
			</div>
		</div>
	</div>
</template>

<style lang="scss">
.floating-menu {
	position: absolute;
	width: 0;
	height: 0;
	display: flex;
	// Floating menus begin at a z-index of 1000
	z-index: 1000;
	--floating-menu-content-offset: 0;
	--floating-menu-content-border-radius: 4px;

	&.bottom {
		--floating-menu-content-border-radius: 0 0 4px 4px;
	}

	.tail {
		width: 0;
		height: 0;
		border-style: solid;
		// Put the tail above the floating menu's shadow
		z-index: 10;
		// Draw over the application without being clipped by the containing panel's `overflow: hidden`
		position: fixed;
	}

	.floating-menu-container {
		display: flex;

		.floating-menu-content {
			background: var(--floating-menu-opacity-color-2-mildblack);
			box-shadow: var(--floating-menu-shadow) 0 2px 4px;
			border-radius: var(--floating-menu-content-border-radius);
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

	&.dropdown {
		&.top {
			width: 100%;
			left: 0;
			top: 0;
		}

		&.bottom {
			width: 100%;
			left: 0;
			bottom: 0;
		}

		&.left {
			height: 100%;
			top: 0;
			left: 0;
		}

		&.right {
			height: 100%;
			top: 0;
			right: 0;
		}

		&.topleft {
			top: 0;
			left: 0;
			margin-top: -4px;
		}

		&.topright {
			top: 0;
			right: 0;
			margin-top: -4px;
		}

		&.topleft {
			bottom: 0;
			left: 0;
			margin-bottom: -4px;
		}

		&.topright {
			bottom: 0;
			right: 0;
			margin-bottom: -4px;
		}
	}

	&.top.dropdown .floating-menu-container,
	&.bottom.dropdown .floating-menu-container {
		justify-content: left;
	}

	&.popover {
		--floating-menu-content-offset: 10px;
		--floating-menu-content-border-radius: 4px;
	}

	&.center {
		justify-content: center;
		align-items: center;

		.floating-menu-content {
			transform: translate(-50%, -50%);
		}
	}

	&.top,
	&.bottom {
		flex-direction: column;
	}

	&.top .tail {
		border-width: 8px 6px 0 6px;
		border-color: var(--floating-menu-opacity-color-2-mildblack) transparent transparent transparent;
		margin-left: -6px;
		margin-bottom: 2px;
	}

	&.bottom .tail {
		border-width: 0 6px 8px 6px;
		border-color: transparent transparent var(--floating-menu-opacity-color-2-mildblack) transparent;
		margin-left: -6px;
		margin-top: 2px;
	}

	&.left .tail {
		border-width: 6px 0 6px 8px;
		border-color: transparent transparent transparent var(--floating-menu-opacity-color-2-mildblack);
		margin-top: -6px;
		margin-right: 2px;
	}

	&.right .tail {
		border-width: 6px 8px 6px 0;
		border-color: transparent var(--floating-menu-opacity-color-2-mildblack) transparent transparent;
		margin-top: -6px;
		margin-left: 2px;
	}

	&.top .floating-menu-container {
		justify-content: center;
		margin-bottom: var(--floating-menu-content-offset);
	}

	&.bottom .floating-menu-container {
		justify-content: center;
		margin-top: var(--floating-menu-content-offset);
	}

	&.left .floating-menu-container {
		align-items: center;
		margin-right: var(--floating-menu-content-offset);
	}

	&.right .floating-menu-container {
		align-items: center;
		margin-left: var(--floating-menu-content-offset);
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export enum MenuDirection {
	Top = "Top",
	Bottom = "Bottom",
	Left = "Left",
	Right = "Right",
	TopLeft = "TopLeft",
	TopRight = "TopRight",
	BottomLeft = "BottomLeft",
	BottomRight = "BottomRight",
	Center = "Center",
}

export enum MenuType {
	Popover = "Popover",
	Dropdown = "Dropdown",
	Dialog = "Dialog",
}

export default defineComponent({
	components: {},
	props: {
		direction: { type: String, default: MenuDirection.Bottom },
		type: { type: String, required: true },
		windowEdgeMargin: { type: Number, default: 8 },
		minWidth: { type: Number, default: 0 },
		scrollable: { type: Boolean, default: false },
	},
	data() {
		return {
			open: false,
			mouseStillDown: false,
			MenuDirection,
			MenuType,
		};
	},
	updated() {
		const floatingMenuContainer = this.$refs.floatingMenuContainer as HTMLElement;
		const floatingMenuContent = this.$refs.floatingMenuContent as HTMLElement;
		const workspace = document.querySelector(".workspace-row");

		if (floatingMenuContent && workspace) {
			const workspaceBounds = workspace.getBoundingClientRect();
			const floatingMenuBounds = floatingMenuContent.getBoundingClientRect();

			if (this.direction === MenuDirection.Left || this.direction === MenuDirection.Right) {
				const topOffset = floatingMenuBounds.top - workspaceBounds.top - this.windowEdgeMargin;
				if (topOffset < 0) floatingMenuContainer.style.transform = `translate(0, ${-topOffset}px)`;

				const bottomOffset = workspaceBounds.bottom - floatingMenuBounds.bottom - this.windowEdgeMargin;
				if (bottomOffset < 0) floatingMenuContainer.style.transform = `translate(0, ${bottomOffset}px)`;
			}

			if (this.direction === MenuDirection.Top || this.direction === MenuDirection.Bottom) {
				const leftOffset = floatingMenuBounds.left - workspaceBounds.left - this.windowEdgeMargin;
				if (leftOffset < 0) floatingMenuContainer.style.transform = `translate(${-leftOffset}px, 0)`;

				const rightOffset = workspaceBounds.right - floatingMenuBounds.right - this.windowEdgeMargin;
				if (rightOffset < 0) floatingMenuContainer.style.transform = `translate(${rightOffset}px, 0)`;
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
		isOpen(): boolean {
			return this.open;
		},
		getWidth(callback: (width: number) => void) {
			this.$nextTick(() => {
				const floatingMenuContent = this.$refs.floatingMenuContent as HTMLElement;
				const width = floatingMenuContent.clientWidth;

				callback(width);
			});
		},
		disableMinWidth(callback: (minWidth: string) => void) {
			this.$nextTick(() => {
				const floatingMenuContent = this.$refs.floatingMenuContent as HTMLElement;
				const initialMinWidth = floatingMenuContent.style.minWidth;
				floatingMenuContent.style.minWidth = "0";

				callback(initialMinWidth);
			});
		},
		enableMinWidth(minWidth: string) {
			const floatingMenuContent = this.$refs.floatingMenuContent as HTMLElement;
			floatingMenuContent.style.minWidth = minWidth;
		},
		mouseMoveHandler(e: MouseEvent) {
			const MOUSE_STRAY_DISTANCE = 100;
			const target = e.target as HTMLElement;
			const mouseOverFloatingMenuKeepOpen = target && (target.closest("[data-hover-menu-keep-open]") as HTMLElement);
			const mouseOverFloatingMenuSpawner = target && (target.closest("[data-hover-menu-spawner]") as HTMLElement);
			// TODO: Simplify the following expression when optional chaining is supported by the build system
			const mouseOverOwnFloatingMenuSpawner =
				mouseOverFloatingMenuSpawner && mouseOverFloatingMenuSpawner.parentElement && mouseOverFloatingMenuSpawner.parentElement.contains(this.$refs.floatingMenu as HTMLElement);

			// Swap this open floating menu with the one created by the floating menu spawner being hovered over
			if (mouseOverFloatingMenuSpawner && !mouseOverOwnFloatingMenuSpawner) {
				this.setClosed();
				mouseOverFloatingMenuSpawner.click();
			}

			// Close the floating menu if the mouse has strayed far enough from its bounds
			if (this.isMouseEventOutsideFloatingMenu(e, MOUSE_STRAY_DISTANCE) && !mouseOverOwnFloatingMenuSpawner && !mouseOverFloatingMenuKeepOpen) {
				// TODO: Extend this rectangle bounds check to all `data-hover-menu-keep-open` element bounds up the DOM tree since currently
				// submenus disappear with zero stray distance if the cursor is further than the stray distance from only the top-level menu
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
			// Close the floating menu if the mouse clicked outside the floating menu (but within stray distance)
			if (this.isMouseEventOutsideFloatingMenu(e)) {
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
			// Stop the click event from reopening this floating menu if the click event targets the floating menu's button
			e.stopPropagation();

			// Clean up self
			window.removeEventListener("click", this.clickHandlerCapture, true);
		},
		isMouseEventOutsideFloatingMenu(e: MouseEvent, extraDistanceAllowed = 0): boolean {
			const floatingMenuContent = this.$refs.floatingMenuContent as HTMLElement;
			if (!floatingMenuContent) return true;
			const floatingMenuBounds = floatingMenuContent.getBoundingClientRect();

			if (floatingMenuBounds.left - e.clientX >= extraDistanceAllowed) return true;
			if (e.clientX - floatingMenuBounds.right >= extraDistanceAllowed) return true;
			if (floatingMenuBounds.top - e.clientY >= extraDistanceAllowed) return true;
			if (e.clientY - floatingMenuBounds.bottom >= extraDistanceAllowed) return true;

			return false;
		},
	},
	watch: {
		open(newState: boolean, oldState: boolean) {
			if (newState && !oldState) {
				// Close floating menu if mouse strays far enough away
				window.addEventListener("mousemove", this.mouseMoveHandler);

				// Close floating menu if mouse is outside (but within stray distance)
				window.addEventListener("mousedown", this.mouseDownHandler);

				// Cancel the subsequent click event to prevent the floating menu from reopening if the floating menu's button is the click event target
				window.addEventListener("mouseup", this.mouseUpHandler);
			}
			if (!newState && oldState) {
				window.removeEventListener("mousemove", this.mouseMoveHandler);
				window.removeEventListener("mousedown", this.mouseDownHandler);
			}
		},
	},
	computed: {
		floatingMenuContentStyle(): Partial<CSSStyleDeclaration> {
			return {
				minWidth: this.minWidth > 0 ? `${this.minWidth}px` : "",
			};
		},
	},
});
</script>
