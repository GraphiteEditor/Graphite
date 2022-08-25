<template>
	<div class="floating-menu" :class="[direction.toLowerCase(), type.toLowerCase()]" ref="floatingMenu">
		<div class="tail" v-if="open && type === 'Popover'" ref="tail"></div>
		<div class="floating-menu-container" v-if="open || measuringOngoing" ref="floatingMenuContainer">
			<LayoutCol class="floating-menu-content" :style="{ minWidth: minWidthStyleValue }" :scrollableY="scrollableY" ref="floatingMenuContent" data-floating-menu-content>
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

		> .floating-menu-container > .floating-menu-content {
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
import { defineComponent, nextTick, type PropType } from "vue";

import LayoutCol from "@/components/layout/LayoutCol.vue";

export type MenuDirection = "Top" | "Bottom" | "Left" | "Right" | "TopLeft" | "TopRight" | "BottomLeft" | "BottomRight" | "Center";
export type MenuType = "Popover" | "Dropdown" | "Dialog";

const POINTER_STRAY_DISTANCE = 100;

export default defineComponent({
	emits: ["update:open", "naturalWidth"],
	props: {
		open: { type: Boolean as PropType<boolean>, required: true },
		type: { type: String as PropType<MenuType>, required: true },
		direction: { type: String as PropType<MenuDirection>, default: "Bottom" },
		windowEdgeMargin: { type: Number as PropType<number>, default: 6 },
		scrollableY: { type: Boolean as PropType<boolean>, default: false },
		minWidth: { type: Number as PropType<number>, default: 0 },
		escapeCloses: { type: Boolean as PropType<boolean>, default: true },
	},
	data() {
		// The resize observer is attached to the floating menu container, which is the zero-height div of the width of the parent element's floating menu spawner.
		// Since CSS doesn't let us make the floating menu (with `position: fixed`) have a 100% width of this container, we need to use JS to observe its size and
		// tell the floating menu content to use it as a min-width so the floating menu is at least the width of the parent element's floating menu spawner.
		// This is the opposite concern of the natural width measurement system, which gets the natural width of the floating menu content in order for the
		// spawner widget to optionally set its min-size to the floating menu's natural width.
		const containerResizeObserver = new ResizeObserver((entries: ResizeObserverEntry[]) => {
			this.resizeObserverCallback(entries);
		});

		return {
			measuringOngoing: false,
			measuringOngoingGuard: false,
			minWidthParentWidth: 0,
			containerResizeObserver,
			pointerStillDown: false,
			workspaceBounds: new DOMRect(),
			floatingMenuBounds: new DOMRect(),
			floatingMenuContentBounds: new DOMRect(),
		};
	},
	computed: {
		minWidthStyleValue() {
			if (this.measuringOngoing) return "0";
			return `${Math.max(this.minWidth, this.minWidthParentWidth)}px`;
		},
	},
	// Gets the client bounds of the elements and apply relevant styles to them
	// TODO: Use the Vue :style attribute more whilst not causing recursive updates
	async updated() {
		// Turning measuring on and off both cause the component to change, which causes the `updated()` Vue event to fire extraneous times (hurting performance and sometimes causing an infinite loop)
		if (this.measuringOngoingGuard) return;

		this.positionAndStyleFloatingMenu();
	},
	methods: {
		resizeObserverCallback(entries: ResizeObserverEntry[]) {
			this.minWidthParentWidth = entries[0].contentRect.width;
		},
		positionAndStyleFloatingMenu() {
			const workspace = document.querySelector("[data-workspace]");
			const floatingMenuContainer = this.$refs.floatingMenuContainer as HTMLElement;
			const floatingMenuContentComponent = this.$refs.floatingMenuContent as typeof LayoutCol;
			const floatingMenuContent: HTMLElement | undefined = floatingMenuContentComponent?.$el;
			const floatingMenu = this.$refs.floatingMenu as HTMLElement;

			if (!workspace || !floatingMenuContainer || !floatingMenuContentComponent || !floatingMenuContent || !floatingMenu) return;

			this.workspaceBounds = workspace.getBoundingClientRect();
			this.floatingMenuBounds = floatingMenu.getBoundingClientRect();
			this.floatingMenuContentBounds = floatingMenuContent.getBoundingClientRect();

			const inParentFloatingMenu = Boolean(floatingMenuContainer.closest("[data-floating-menu-content]"));

			if (!inParentFloatingMenu) {
				// Required to correctly position content when scrolled (it has a `position: fixed` to prevent clipping)
				const tailOffset = this.type === "Popover" ? 10 : 0;
				if (this.direction === "Bottom") floatingMenuContent.style.top = `${tailOffset + this.floatingMenuBounds.top}px`;
				if (this.direction === "Top") floatingMenuContent.style.bottom = `${tailOffset + this.floatingMenuBounds.bottom}px`;
				if (this.direction === "Right") floatingMenuContent.style.left = `${tailOffset + this.floatingMenuBounds.left}px`;
				if (this.direction === "Left") floatingMenuContent.style.right = `${tailOffset + this.floatingMenuBounds.right}px`;

				// Required to correctly position tail when scrolled (it has a `position: fixed` to prevent clipping)
				const tail = this.$refs.tail as HTMLElement;
				if (tail) {
					if (this.direction === "Bottom") tail.style.top = `${this.floatingMenuBounds.top}px`;
					if (this.direction === "Top") tail.style.bottom = `${this.floatingMenuBounds.bottom}px`;
					if (this.direction === "Right") tail.style.left = `${this.floatingMenuBounds.left}px`;
					if (this.direction === "Left") tail.style.right = `${this.floatingMenuBounds.right}px`;
				}
			}

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
		// To be called by the parent component. Measures the actual width of the floating menu content element and returns it in a promise.
		async measureAndEmitNaturalWidth(): Promise<void> {
			// Wait for the changed content which fired the `updated()` Vue event to be put into the DOM
			await nextTick();

			// Wait until all fonts have been loaded and rendered so measurements of content involving text are accurate
			// API is experimental but supported in all browsers - https://developer.mozilla.org/en-US/docs/Web/API/FontFaceSet/ready
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			await (document as any).fonts.ready;

			// Make the component show itself with 0 min-width so it can be measured, and wait until the values have been updated to the DOM
			this.measuringOngoing = true;
			this.measuringOngoingGuard = true;

			await nextTick();

			// Only measure if the menu is visible, perhaps because a parent component with a `v-if` condition is false
			let naturalWidth;
			if (this.$refs.floatingMenuContent) {
				// Measure the width of the floating menu content element
				const floatingMenuContent: HTMLElement = (this.$refs.floatingMenuContent as typeof LayoutCol).$el;
				naturalWidth = floatingMenuContent?.clientWidth;
			}

			// Turn off measuring mode for the component, which triggers another call to the `updated()` Vue event, so we can turn off the protection after that has happened
			this.measuringOngoing = false;
			await nextTick();
			this.measuringOngoingGuard = false;

			// Emit the measured natural width to the parent
			if (naturalWidth !== undefined && naturalWidth >= 0) {
				this.$emit("naturalWidth", naturalWidth);
			}
		},
		pointerMoveHandler(e: PointerEvent) {
			const target = e.target as HTMLElement | undefined;
			const pointerOverFloatingMenuKeepOpen = target?.closest("[data-hover-menu-keep-open]") as HTMLElement | undefined;
			const pointerOverFloatingMenuSpawner = target?.closest("[data-hover-menu-spawner]") as HTMLElement | undefined;
			const pointerOverOwnFloatingMenuSpawner = pointerOverFloatingMenuSpawner?.parentElement?.contains(this.$refs.floatingMenu as HTMLElement);

			// Swap this open floating menu with the one created by the floating menu spawner being hovered over
			if (pointerOverFloatingMenuSpawner && !pointerOverOwnFloatingMenuSpawner) {
				this.$emit("update:open", false);
				pointerOverFloatingMenuSpawner.click();
			}

			// Close the floating menu if the pointer has strayed far enough from its bounds
			if (this.isPointerEventOutsideFloatingMenu(e, POINTER_STRAY_DISTANCE) && !pointerOverOwnFloatingMenuSpawner && !pointerOverFloatingMenuKeepOpen) {
				// TODO: Extend this rectangle bounds check to all `data-hover-menu-keep-open` element bounds up the DOM tree since currently
				// submenus disappear with zero stray distance if the cursor is further than the stray distance from only the top-level menu
				this.$emit("update:open", false);
			}

			// Clean up any messes from lost pointerup events
			const eventIncludesLmb = Boolean(e.buttons & 1);
			if (!this.open && !eventIncludesLmb) {
				this.pointerStillDown = false;
				window.removeEventListener("pointerup", this.pointerUpHandler);
			}
		},
		keyDownHandler(e: KeyboardEvent) {
			if (this.escapeCloses && e.key.toLowerCase() === "escape") {
				this.$emit("update:open", false);
			}
		},
		pointerDownHandler(e: PointerEvent) {
			// Close the floating menu if the pointer clicked outside the floating menu (but within stray distance)
			if (this.isPointerEventOutsideFloatingMenu(e)) {
				this.$emit("update:open", false);

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
		// Called only when `open` is changed from outside this component (with v-model)
		async open(newState: boolean, oldState: boolean) {
			// Switching from closed to open
			if (newState && !oldState) {
				// Close floating menu if pointer strays far enough away
				window.addEventListener("pointermove", this.pointerMoveHandler);
				// Close floating menu if esc is pressed
				window.addEventListener("keydown", this.keyDownHandler);
				// Close floating menu if pointer is outside (but within stray distance)
				window.addEventListener("pointerdown", this.pointerDownHandler);
				// Cancel the subsequent click event to prevent the floating menu from reopening if the floating menu's button is the click event target
				window.addEventListener("pointerup", this.pointerUpHandler);

				// Floating menu min-width resize observer

				await nextTick();

				const floatingMenuContainer = this.$refs.floatingMenuContainer as HTMLElement;
				if (!floatingMenuContainer) return;

				// Start a new observation of the now-open floating menu
				this.containerResizeObserver.disconnect();
				this.containerResizeObserver.observe(floatingMenuContainer);
			}

			// Switching from open to closed
			if (!newState && oldState) {
				// Clean up observation of the now-closed floating menu
				this.containerResizeObserver.disconnect();

				window.removeEventListener("pointermove", this.pointerMoveHandler);
				window.removeEventListener("keydown", this.keyDownHandler);
				window.removeEventListener("pointerdown", this.pointerDownHandler);
				// The `pointerup` event is removed in `pointerMoveHandler()` and `pointerDownHandler()`
			}
		},
	},
	components: { LayoutCol },
});
</script>
