<template>
	<div class="floating-menu" :class="[direction.toLowerCase(), type.toLowerCase()]" v-if="open || type === 'Dialog'" ref="floatingMenu">
		<div class="tail" v-if="type === 'Popover'" :style="tailStyle"></div>
		<div class="floating-menu-container" ref="floatingMenuContainer">
			<LayoutCol class="floating-menu-content" data-floating-menu-content :scrollableY="scrollableY" ref="floatingMenuContent" :style="floatingMenuContentStyle">
				<slot></slot>
			</LayoutCol>
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
			background: rgba(var(--color-2-mildblack-rgb), 0.95);
			box-shadow: rgba(var(--color-0-black-rgb), 50%) 0 2px 4px;
			border-radius: var(--floating-menu-content-border-radius);
			color: var(--color-e-nearwhite);
			font-size: inherit;
			padding: 8px;
			z-index: 0;
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
		border-color: rgba(var(--color-2-mildblack-rgb), 0.95) transparent transparent transparent;
		margin-left: -6px;
		margin-bottom: 2px;
	}

	&.bottom .tail {
		border-width: 0 6px 8px 6px;
		border-color: transparent transparent rgba(var(--color-2-mildblack-rgb), 0.95) transparent;
		margin-left: -6px;
		margin-top: 2px;
	}

	&.left .tail {
		border-width: 6px 0 6px 8px;
		border-color: transparent transparent transparent rgba(var(--color-2-mildblack-rgb), 0.95);
		margin-top: -6px;
		margin-right: 2px;
	}

	&.right .tail {
		border-width: 6px 8px 6px 0;
		border-color: transparent rgba(var(--color-2-mildblack-rgb), 0.95) transparent transparent;
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
import { defineComponent, PropType, StyleValue } from "vue";

import LayoutCol from "@/components/layout/LayoutCol.vue";

export type MenuDirection = "Top" | "Bottom" | "Left" | "Right" | "TopLeft" | "TopRight" | "BottomLeft" | "BottomRight" | "Center";
export type MenuType = "Popover" | "Dropdown" | "Dialog";

const POINTER_STRAY_DISTANCE = 100;

export default defineComponent({
	props: {
		direction: { type: String as PropType<MenuDirection>, default: "Bottom" },
		type: { type: String as PropType<MenuType>, required: true },
		windowEdgeMargin: { type: Number as PropType<number>, default: 6 },
		minWidth: { type: Number as PropType<number>, default: 0 },
		scrollableY: { type: Boolean as PropType<boolean>, default: false },
	},
	data() {
		const containerResizeObserver = new ResizeObserver((entries) => {
			const content = entries[0].target.querySelector("[data-floating-menu-content]") as HTMLElement;
			content.style.minWidth = `${entries[0].contentRect.width}px`;
		});
		return {
			open: false,
			pointerStillDown: false,
			containerResizeObserver,
			workspaceBounds: new DOMRect(),
			floatingMenuBounds: new DOMRect(),
			floatingMenuContentBounds: new DOMRect(),
		};
	},
	// Gets the client bounds of the elements and apply relevant styles to them
	// TODO: Use the Vue :style attribute more whilst not causing recursive updates
	updated() {
		const workspace = document.querySelector("[data-workspace]");
		const floatingMenuContainer = this.$refs.floatingMenuContainer as HTMLElement;
		const floatingMenuContentComponent = this.$refs.floatingMenuContent as typeof LayoutCol;
		const floatingMenuContent = floatingMenuContentComponent && (floatingMenuContentComponent.$el as HTMLElement);
		const floatingMenu = this.$refs.floatingMenu as HTMLElement;

		if (!workspace || !floatingMenuContainer || !floatingMenuContentComponent || !floatingMenuContent || !floatingMenu) return;

		this.workspaceBounds = workspace.getBoundingClientRect();
		this.floatingMenuBounds = floatingMenu.getBoundingClientRect();
		this.floatingMenuContentBounds = floatingMenuContent.getBoundingClientRect();

		// Required to correctly position content when scrolled (it has a `position: fixed` to prevent clipping)
		const tailOffset = this.type === "Popover" ? 10 : 0;
		if (this.direction === "Bottom") floatingMenuContent.style.top = `${tailOffset + this.floatingMenuBounds.top}px`;
		if (this.direction === "Top") floatingMenuContent.style.bottom = `${tailOffset + this.floatingMenuBounds.bottom}px`;
		if (this.direction === "Right") floatingMenuContent.style.left = `${tailOffset + this.floatingMenuBounds.left}px`;
		if (this.direction === "Left") floatingMenuContent.style.right = `${tailOffset + this.floatingMenuBounds.right}px`;

		type Edge = "Top" | "Bottom" | "Left" | "Right";
		let zeroedBorderVertical: Edge | undefined;
		let zeroedBorderHorizontal: Edge | undefined;

		if (this.direction === "Top" || this.direction === "Bottom") {
			zeroedBorderVertical = this.direction === "Top" ? "Bottom" : "Top";

			if (this.floatingMenuContentBounds.left - this.windowEdgeMargin <= this.workspaceBounds.left) {
				floatingMenuContent.style.left = `${this.windowEdgeMargin}px`;
				if (this.workspaceBounds.left + floatingMenuContainer.getBoundingClientRect().left === 12) zeroedBorderHorizontal = "Left";
			}
			if (this.floatingMenuContentBounds.right + this.windowEdgeMargin >= this.workspaceBounds.right) {
				floatingMenuContent.style.right = `${this.windowEdgeMargin}px`;
				if (this.workspaceBounds.right - floatingMenuContainer.getBoundingClientRect().right === 12) zeroedBorderHorizontal = "Right";
			}
		}
		if (this.direction === "Left" || this.direction === "Right") {
			zeroedBorderHorizontal = this.direction === "Left" ? "Right" : "Left";

			if (this.floatingMenuContentBounds.top - this.windowEdgeMargin <= this.workspaceBounds.top) {
				floatingMenuContent.style.top = `${this.windowEdgeMargin}px`;
				if (this.workspaceBounds.top + floatingMenuContainer.getBoundingClientRect().top === 12) zeroedBorderVertical = "Top";
			}
			if (this.floatingMenuContentBounds.bottom + this.windowEdgeMargin >= this.workspaceBounds.bottom) {
				floatingMenuContent.style.bottom = `${this.windowEdgeMargin}px`;
				if (this.workspaceBounds.bottom - floatingMenuContainer.getBoundingClientRect().bottom === 12) zeroedBorderVertical = "Bottom";
			}
		}

		// Remove the rounded corner from the content where the tail perfectly meets the corner
		if (this.type === "Popover" && this.windowEdgeMargin === 6 && zeroedBorderVertical && zeroedBorderHorizontal) {
			switch (`${zeroedBorderVertical}${zeroedBorderHorizontal}`) {
				case "TopLeft":
					floatingMenuContent.style.borderTopLeftRadius = "0";
					break;
				case "TopRight":
					floatingMenuContent.style.borderTopRightRadius = "0";
					break;
				case "BottomLeft":
					floatingMenuContent.style.borderBottomLeftRadius = "0";
					break;
				case "BottomRight":
					floatingMenuContent.style.borderBottomRightRadius = "0";
					break;
				default:
					break;
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
				const floatingMenuContent = (this.$refs.floatingMenuContent as typeof LayoutCol).$el as HTMLElement;
				const width = floatingMenuContent.clientWidth;
				callback(width);
			});
		},
		disableMinWidth(callback: (minWidth: string) => void) {
			this.$nextTick(() => {
				const floatingMenuContent = (this.$refs.floatingMenuContent as typeof LayoutCol).$el as HTMLElement;
				const initialMinWidth = floatingMenuContent.style.minWidth;
				floatingMenuContent.style.minWidth = "0";
				callback(initialMinWidth);
			});
		},
		enableMinWidth(minWidth: string) {
			const floatingMenuContent = (this.$refs.floatingMenuContent as typeof LayoutCol).$el as HTMLElement;
			floatingMenuContent.style.minWidth = minWidth;
		},
		pointerMoveHandler(e: PointerEvent) {
			const target = e.target as HTMLElement;
			const pointerOverFloatingMenuKeepOpen = target && (target.closest("[data-hover-menu-keep-open]") as HTMLElement);
			const pointerOverFloatingMenuSpawner = target && (target.closest("[data-hover-menu-spawner]") as HTMLElement);
			// TODO: Simplify the following expression when optional chaining is supported by the build system
			const pointerOverOwnFloatingMenuSpawner =
				pointerOverFloatingMenuSpawner && pointerOverFloatingMenuSpawner.parentElement && pointerOverFloatingMenuSpawner.parentElement.contains(this.$refs.floatingMenu as HTMLElement);
			// Swap this open floating menu with the one created by the floating menu spawner being hovered over
			if (pointerOverFloatingMenuSpawner && !pointerOverOwnFloatingMenuSpawner) {
				this.setClosed();
				pointerOverFloatingMenuSpawner.click();
			}
			// Close the floating menu if the pointer has strayed far enough from its bounds
			if (this.isPointerEventOutsideFloatingMenu(e, POINTER_STRAY_DISTANCE) && !pointerOverOwnFloatingMenuSpawner && !pointerOverFloatingMenuKeepOpen) {
				// TODO: Extend this rectangle bounds check to all `data-hover-menu-keep-open` element bounds up the DOM tree since currently
				// submenus disappear with zero stray distance if the cursor is further than the stray distance from only the top-level menu
				this.setClosed();
			}
			const eventIncludesLmb = Boolean(e.buttons & 1);
			// Clean up any messes from lost pointerup events
			if (!this.open && !eventIncludesLmb) {
				this.pointerStillDown = false;
				window.removeEventListener("pointerup", this.pointerUpHandler);
			}
		},
		pointerDownHandler(e: PointerEvent) {
			// Close the floating menu if the pointer clicked outside the floating menu (but within stray distance)
			if (this.isPointerEventOutsideFloatingMenu(e)) {
				this.setClosed();
				// Track if the left pointer button is now down so its later click event can be canceled
				const eventIsForLmb = e.button === 0;
				if (eventIsForLmb) this.pointerStillDown = true;
			}
		},
		pointerUpHandler(e: PointerEvent) {
			const eventIsForLmb = e.button === 0;
			if (this.pointerStillDown && eventIsForLmb) {
				// Clean up self
				this.pointerStillDown = false;
				window.removeEventListener("pointerup", this.pointerUpHandler);
				// Prevent the click event from firing, which would normally occur right after this pointerup event
				window.addEventListener("click", this.clickHandlerCapture, true);
			}
		},
		clickHandlerCapture(e: MouseEvent) {
			// Stop the click event from reopening this floating menu if the click event targets the floating menu's button
			e.stopPropagation();
			// Clean up self
			window.removeEventListener("click", this.clickHandlerCapture, true);
		},
		isPointerEventOutsideFloatingMenu(e: PointerEvent, extraDistanceAllowed = 0): boolean {
			// Considers all child menus as well as the top-level one.
			const allContainedFloatingMenus = [...this.$el.querySelectorAll("[data-floating-menu-content]")];
			return !allContainedFloatingMenus.find((element) => !this.isPointerEventOutsideMenuElement(e, element, extraDistanceAllowed));
		},
		isPointerEventOutsideMenuElement(e: PointerEvent, element: HTMLElement, extraDistanceAllowed = 0): boolean {
			const floatingMenuBounds = element.getBoundingClientRect();
			if (floatingMenuBounds.left - e.clientX >= extraDistanceAllowed) return true;
			if (e.clientX - floatingMenuBounds.right >= extraDistanceAllowed) return true;
			if (floatingMenuBounds.top - e.clientY >= extraDistanceAllowed) return true;
			if (e.clientY - floatingMenuBounds.bottom >= extraDistanceAllowed) return true;
			return false;
		},
	},
	watch: {
		open(newState: boolean, oldState: boolean) {
			// Switching from closed to open
			if (newState && !oldState) {
				// Close floating menu if pointer strays far enough away
				window.addEventListener("pointermove", this.pointerMoveHandler);
				// Close floating menu if pointer is outside (but within stray distance)
				window.addEventListener("pointerdown", this.pointerDownHandler);
				// Cancel the subsequent click event to prevent the floating menu from reopening if the floating menu's button is the click event target
				window.addEventListener("pointerup", this.pointerUpHandler);
				// Floating menu min-width resize observer
				this.$nextTick(() => {
					const floatingMenuContainer = this.$refs.floatingMenuContainer as HTMLElement;
					if (floatingMenuContainer) {
						this.containerResizeObserver.disconnect();
						this.containerResizeObserver.observe(floatingMenuContainer);
					}
				});
			}

			// Switching from open to closed
			if (!newState && oldState) {
				window.removeEventListener("pointermove", this.pointerMoveHandler);
				window.removeEventListener("pointerdown", this.pointerDownHandler);
				this.containerResizeObserver.disconnect();
			}
		},
	},
	computed: {
		floatingMenuContentStyle(): StyleValue {
			return {
				minWidth: this.minWidth > 0 ? `${this.minWidth}px` : "",
			};
		},
		// Required to correctly position the tail when scrolled (it has a `position: fixed` to prevent clipping)
		tailStyle(): StyleValue {
			if (this.direction === "Bottom") return { top: `${this.floatingMenuBounds.top}px` };
			if (this.direction === "Top") return { bottom: `${this.floatingMenuBounds.bottom}px` };
			if (this.direction === "Right") return { left: `${this.floatingMenuBounds.left}px` };
			if (this.direction === "Left") return { right: `${this.floatingMenuBounds.right}px` };
			return {};
		},
	},
	components: { LayoutCol },
});
</script>
