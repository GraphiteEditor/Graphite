<script lang="ts" context="module">
	export type MenuType = "Popover" | "Tooltip" | "Dropdown" | "Dialog" | "Cursor";

	/// Prevents the escape key from closing the parent floating menu of the given element.
	/// This works by momentarily setting the `data-escape-does-not-close` attribute on the parent floating menu element.
	/// After checking for the Escape key, it checks (in one `setTimeout`) for the attribute and ignores the key if it's present.
	/// Then after two calls of `setTimeout`, we can safely remove the attribute here.
	export function preventEscapeClosingParentFloatingMenu(element: HTMLElement) {
		const floatingMenuParent = element.closest("[data-floating-menu-content]") || undefined;
		if (floatingMenuParent instanceof HTMLElement) {
			floatingMenuParent.setAttribute("data-escape-does-not-close", "");
			setTimeout(() => {
				setTimeout(() => {
					floatingMenuParent.removeAttribute("data-escape-does-not-close");
				}, 0);
			}, 0);
		}
	}
</script>

<script lang="ts">
	import { onMount, afterUpdate, createEventDispatcher, tick } from "svelte";

	import type { MenuDirection } from "@graphite/messages";
	import { browserVersion } from "@graphite/utility-functions/platform";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";

	const BUTTON_LEFT = 0;
	const POINTER_STRAY_DISTANCE = 100;

	const dispatch = createEventDispatcher<{ open: boolean; naturalWidth: number }>();

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};
	let styleName = "";
	export { styleName as style };
	export let styles: Record<string, string | number | undefined> = {};
	export let open: boolean;
	export let type: MenuType;
	export let direction: MenuDirection = "Bottom";
	export let windowEdgeMargin = 6;
	export let scrollableY = false;
	export let minWidth = 0;
	export let escapeCloses = true;
	export let strayCloses = true;

	let tail: HTMLDivElement | undefined;
	let self: HTMLDivElement | undefined;
	let floatingMenuContainer: HTMLDivElement | undefined;
	let floatingMenuContent: LayoutCol | undefined;

	let containerResizeObserver = new ResizeObserver((entries: ResizeObserverEntry[]) => {
		resizeObserverCallback(entries);
	});
	let wasOpen = open;
	let measuringOngoing = false;
	let measuringOngoingGuard = false;
	let minWidthParentWidth = 0;
	let pointerStillDown = false;
	let floatingMenuBounds = new DOMRect();
	let floatingMenuContentBounds = new DOMRect();

	$: watchOpenChange(open);

	$: minWidthStyleValue = measuringOngoing ? "0" : `${Math.max(minWidth, minWidthParentWidth)}px`;
	$: displayTail = open && type === "Popover";
	$: displayContainer = open || measuringOngoing;
	$: extraClasses = Object.entries(classes)
		.flatMap(([className, stateName]) => (stateName ? [className] : []))
		.join(" ");
	$: extraStyles = Object.entries(styles)
		.flatMap((styleAndValue) => (styleAndValue[1] !== undefined ? [`${styleAndValue[0]}: ${styleAndValue[1]};`] : []))
		.join(" ");

	// Generic function to get constraint bounds for positioning
	// Returns the bounds of the scrollable parent if one exists, otherwise returns window bounds
	function getConstraintBounds(element: HTMLElement | undefined): DOMRect {
		const scrollableParent = element?.closest("[data-scrollable-x], [data-scrollable-y]");
		
		if (scrollableParent) {
			return scrollableParent.getBoundingClientRect();
		}
		
		return document.documentElement.getBoundingClientRect();
	}

	// Called only when `open` is changed from outside this component
	async function watchOpenChange(isOpen: boolean) {
		const scrollableParent = self?.closest("[data-scrollable-x], [data-scrollable-y]");
		const isInScrollableContainer = Boolean(scrollableParent);
		
		// Mitigate a Safari rendering bug - only apply if NOT in scrollable container
		if (browserVersion().toLowerCase().includes("safari") && !isInScrollableContainer) {
			const scrollable = self?.closest("[data-scrollable-x], [data-scrollable-y]");
			if (scrollable instanceof HTMLElement) {
				scrollable.style.overflow = isOpen ? "hidden" : "";
			}
		}

		// Switching from closed to open
		if (isOpen && !wasOpen) {
			// Close floating menu if pointer strays far enough away
			window.addEventListener("pointermove", pointerMoveHandler);
			// Close floating menu if esc is pressed
			window.addEventListener("keydown", keyDownHandler);
			// Close floating menu if pointer is outside (but within stray distance)
			window.addEventListener("pointerdown", pointerDownHandler);
			// Cancel the subsequent click event to prevent the floating menu from reopening if the floating menu's button is the click event target
			window.addEventListener("pointerup", pointerUpHandler);

			await tick();

			// Add scroll listener for menus in scrollable containers
			if (isInScrollableContainer && scrollableParent) {
				const scrollHandler = () => {
					// Get constraint bounds from scrollable parent
					const constraintBounds = scrollableParent.getBoundingClientRect();
					const buttonBounds = self?.getBoundingClientRect();
					
					// Close menu if button is scrolled out of view
					if (buttonBounds) {
						const isOffScreen = 
							buttonBounds.right < constraintBounds.left ||
							buttonBounds.left > constraintBounds.right ||
							buttonBounds.bottom < constraintBounds.top ||
							buttonBounds.top > constraintBounds.bottom;
						
						if (isOffScreen) {
							dispatch("open", false);
							return;
						}
					}
					
					// Update position
					positionAndStyleFloatingMenu();
				};
				
				scrollableParent.addEventListener("scroll", scrollHandler);
			}

			// Start a new observation of the now-open floating menu
			if (floatingMenuContainer) {
				containerResizeObserver.disconnect();
				containerResizeObserver.observe(floatingMenuContainer);
			}
		}

		// Switching from open to closed
		if (!isOpen && wasOpen) {
			// Clean up observation of the now-closed floating menu
			containerResizeObserver.disconnect();

			window.removeEventListener("pointermove", pointerMoveHandler);
			window.removeEventListener("keydown", keyDownHandler);
			window.removeEventListener("pointerdown", pointerDownHandler);

			// Clean up scroll listener
			if (isInScrollableContainer && scrollableParent) {
				scrollableParent.removeEventListener("scroll", positionAndStyleFloatingMenu);
			}
		}

		// Now that we're done reading the old state, update it to the current state for next time
		wasOpen = isOpen;
	}

	onMount(() => {
		// Measure the content and round up its width and height to the nearest even integer.
		const floatingMenuContentDiv = floatingMenuContent?.div?.();
		if (type === "Dialog" && floatingMenuContentDiv) {
			const resizeObserver = new ResizeObserver((entries) => {
				entries.forEach((entry) => {
					const existingWidth = Number(floatingMenuContentDiv.style.getPropertyValue("--even-integer-subpixel-expansion-x"));
					const existingHeight = Number(floatingMenuContentDiv.style.getPropertyValue("--even-integer-subpixel-expansion-y"));

					let { width, height } = entry.contentRect;
					width -= existingWidth;
					height -= existingHeight;

					let targetWidth = Math.ceil(width);
					if (targetWidth % 2 === 1) targetWidth += 1;
					let targetHeight = Math.ceil(height);
					if (targetHeight % 2 === 1) targetHeight += 1;

					floatingMenuContentDiv.style.setProperty("--even-integer-subpixel-expansion-x", `${targetWidth - width}`);
					floatingMenuContentDiv.style.setProperty("--even-integer-subpixel-expansion-y", `${targetHeight - height}`);
				});
			});
			resizeObserver.observe(floatingMenuContentDiv);
		}
	});

	afterUpdate(() => {
		if (!measuringOngoingGuard) positionAndStyleFloatingMenu();
	});

	function resizeObserverCallback(entries: ResizeObserverEntry[]) {
		minWidthParentWidth = entries[0].contentRect.width;
	}

	function positionAndStyleFloatingMenu() {
		if (type === "Cursor") return;

		const floatingMenuContentDiv = floatingMenuContent?.div?.();
		if (!self || !floatingMenuContainer || !floatingMenuContent || !floatingMenuContentDiv) return;

		// Get constraint bounds generically
		const constraintBounds = getConstraintBounds(self);
		floatingMenuBounds = self.getBoundingClientRect();
		const floatingMenuContainerBounds = floatingMenuContainer.getBoundingClientRect();

		// Check if in scrollable container
		const scrollableParent = self?.closest("[data-scrollable-x], [data-scrollable-y]");
		const isInScrollableContainer = Boolean(scrollableParent);

		// For tooltips, flip direction if overflowing
		if (type === "Tooltip") {
			const floatingMenuContentBounds = floatingMenuContentDiv.getBoundingClientRect();
			const overflowingTop = floatingMenuContentBounds.top - windowEdgeMargin <= constraintBounds.top;
			const overflowingBottom = floatingMenuContentBounds.bottom + windowEdgeMargin >= constraintBounds.bottom;
			const overflowingLeft = floatingMenuContentBounds.left - windowEdgeMargin <= constraintBounds.left;
			const overflowingRight = floatingMenuContentBounds.right + windowEdgeMargin >= constraintBounds.right;

			if (direction === "Top" && overflowingTop) direction = "Bottom";
			else if (direction === "Bottom" && overflowingBottom) direction = "Top";
			else if (direction === "Left" && overflowingLeft) direction = "Right";
			else if (direction === "Right" && overflowingRight) direction = "Left";
		}

		const inParentFloatingMenu = Boolean(floatingMenuContainer.closest("[data-floating-menu-content]"));

		if (!inParentFloatingMenu) {
			let tailOffset = 0;
			if (type === "Popover") tailOffset = 10;
			if (type === "Tooltip") tailOffset = direction === "Bottom" ? 20 : 10;

			// For menus in scrollable containers, position dynamically and center on button
			if (isInScrollableContainer) {
				floatingMenuContentDiv.style.position = "fixed";

				const buttonCenterX = floatingMenuBounds.x + floatingMenuBounds.width / 2;
				const buttonCenterY = floatingMenuBounds.y + floatingMenuBounds.height / 2;

				// Set position based on direction
				if (direction === "Bottom") {
					floatingMenuContentDiv.style.top = `${tailOffset + floatingMenuBounds.y}px`;
					floatingMenuContentDiv.style.left = `${buttonCenterX}px`;
					floatingMenuContentDiv.style.bottom = "";
					floatingMenuContentDiv.style.right = "";
					floatingMenuContentDiv.style.transform = "translateX(-50%)";
				} else if (direction === "Top") {
					floatingMenuContentDiv.style.bottom = `${tailOffset + (constraintBounds.height - (floatingMenuBounds.y - constraintBounds.top))}px`;
					floatingMenuContentDiv.style.left = `${buttonCenterX}px`;
					floatingMenuContentDiv.style.top = "";
					floatingMenuContentDiv.style.right = "";
					floatingMenuContentDiv.style.transform = "translateX(-50%)";
				} else if (direction === "Right") {
					floatingMenuContentDiv.style.left = `${tailOffset + floatingMenuBounds.x}px`;
					floatingMenuContentDiv.style.top = `${buttonCenterY}px`;
					floatingMenuContentDiv.style.bottom = "";
					floatingMenuContentDiv.style.right = "";
					floatingMenuContentDiv.style.transform = "translateY(-50%)";
				} else if (direction === "Left") {
					floatingMenuContentDiv.style.right = `${tailOffset + (constraintBounds.width - (floatingMenuBounds.x - constraintBounds.left))}px`;
					floatingMenuContentDiv.style.top = `${buttonCenterY}px`;
					floatingMenuContentDiv.style.bottom = "";
					floatingMenuContentDiv.style.left = "";
					floatingMenuContentDiv.style.transform = "translateY(-50%)";
				}

				// Recalculate bounds after positioning
				floatingMenuContentBounds = floatingMenuContentDiv.getBoundingClientRect();

				const overflowingLeft = floatingMenuContentBounds.left - windowEdgeMargin <= constraintBounds.left;
				const overflowingRight = floatingMenuContentBounds.right + windowEdgeMargin >= constraintBounds.right;
				const overflowingTop = floatingMenuContentBounds.top - windowEdgeMargin <= constraintBounds.top;
				const overflowingBottom = floatingMenuContentBounds.bottom + windowEdgeMargin >= constraintBounds.bottom;

				// Adjust for overflow
				if (direction === "Bottom" || direction === "Top") {
					if (overflowingLeft) {
						const overflow = windowEdgeMargin + constraintBounds.left - floatingMenuContentBounds.left;
						floatingMenuContentDiv.style.left = `${buttonCenterX + overflow}px`;
					} else if (overflowingRight) {
						const overflow = floatingMenuContentBounds.right + windowEdgeMargin - constraintBounds.right;
						floatingMenuContentDiv.style.left = `${buttonCenterX - overflow}px`;
					}
				} else if (direction === "Left" || direction === "Right") {
					if (overflowingTop) {
						const overflow = windowEdgeMargin + constraintBounds.top - floatingMenuContentBounds.top;
						floatingMenuContentDiv.style.top = `${buttonCenterY + overflow}px`;
					} else if (overflowingBottom) {
						const overflow = floatingMenuContentBounds.bottom + windowEdgeMargin - constraintBounds.bottom;
						floatingMenuContentDiv.style.top = `${buttonCenterY - overflow}px`;
					}
				}
			} else {
				// Standard positioning for non-scrollable contexts
				floatingMenuContentDiv.style.position = "fixed";

				if (direction === "Bottom") floatingMenuContentDiv.style.top = `${tailOffset + floatingMenuBounds.y}px`;
				if (direction === "Top") floatingMenuContentDiv.style.bottom = `${tailOffset + (constraintBounds.height - floatingMenuBounds.y)}px`;
				if (direction === "Right") floatingMenuContentDiv.style.left = `${tailOffset + floatingMenuBounds.x}px`;
				if (direction === "Left") floatingMenuContentDiv.style.right = `${tailOffset + (constraintBounds.width - floatingMenuBounds.x)}px`;
			}

			// Update tail position
			if (tail) {
				const buttonCenterX = floatingMenuBounds.x + floatingMenuBounds.width / 2;
				const buttonCenterY = floatingMenuBounds.y + floatingMenuBounds.height / 2;

				const dialogBounds = floatingMenuContentDiv.getBoundingClientRect();
				const borderRadius = 4;
				const tailWidth = 12;

				if (direction === "Bottom" || direction === "Top") {
					const minX = dialogBounds.left + borderRadius + tailWidth / 2;
					const maxX = dialogBounds.right - borderRadius - tailWidth / 2;
					const constrainedX = Math.max(minX, Math.min(maxX, buttonCenterX));

					if (direction === "Bottom") {
						tail.style.top = `${floatingMenuBounds.y}px`;
						tail.style.left = `${constrainedX}px`;
					} else {
						tail.style.bottom = `${constraintBounds.height - floatingMenuBounds.y}px`;
						tail.style.left = `${constrainedX}px`;
					}
				} else if (direction === "Left" || direction === "Right") {
					const minY = dialogBounds.top + borderRadius + tailWidth / 2;
					const maxY = dialogBounds.bottom - borderRadius - tailWidth / 2;
					const constrainedY = Math.max(minY, Math.min(maxY, buttonCenterY));

					if (direction === "Right") {
						tail.style.left = `${floatingMenuBounds.x}px`;
						tail.style.top = `${constrainedY}px`;
					} else {
						tail.style.right = `${constraintBounds.width - floatingMenuBounds.x}px`;
						tail.style.top = `${constrainedY}px`;
					}
				}
			}
		}

		// Handle overflow for non-scrollable contexts
		if (!isInScrollableContainer) {
			floatingMenuContentBounds = floatingMenuContentDiv.getBoundingClientRect();

			const overflowingLeft = floatingMenuContentBounds.left - windowEdgeMargin <= constraintBounds.left;
			const overflowingRight = floatingMenuContentBounds.right + windowEdgeMargin >= constraintBounds.right;
			const overflowingTop = floatingMenuContentBounds.top - windowEdgeMargin <= constraintBounds.top;
			const overflowingBottom = floatingMenuContentBounds.bottom + windowEdgeMargin >= constraintBounds.bottom;

			type Edge = "Top" | "Bottom" | "Left" | "Right";
			let zeroedBorderVertical: Edge | undefined;
			let zeroedBorderHorizontal: Edge | undefined;

			if (direction === "Top" || direction === "Bottom") {
				zeroedBorderVertical = direction === "Top" ? "Bottom" : "Top";

				if (overflowingLeft) {
					floatingMenuContentDiv.style.left = `${windowEdgeMargin}px`;
					if (constraintBounds.left + floatingMenuContainerBounds.left === 12) zeroedBorderHorizontal = "Left";
				}
				if (overflowingRight) {
					floatingMenuContentDiv.style.right = `${windowEdgeMargin}px`;
					if (constraintBounds.right - floatingMenuContainerBounds.right === 12) zeroedBorderHorizontal = "Right";
				}
			}
			if (direction === "Left" || direction === "Right") {
				zeroedBorderHorizontal = direction === "Left" ? "Right" : "Left";

				if (overflowingTop) {
					floatingMenuContentDiv.style.top = `${windowEdgeMargin}px`;
					if (constraintBounds.top + floatingMenuContainerBounds.top === 12) zeroedBorderVertical = "Top";
				}
				if (overflowingBottom) {
					floatingMenuContentDiv.style.bottom = `${windowEdgeMargin}px`;
					if (constraintBounds.bottom - floatingMenuContainerBounds.bottom === 12) zeroedBorderVertical = "Bottom";
				}
			}

			// Remove rounded corner where tail meets content
			if (displayTail && windowEdgeMargin === 6 && zeroedBorderVertical && zeroedBorderHorizontal) {
				switch (`${zeroedBorderVertical}${zeroedBorderHorizontal}`) {
					case "TopLeft":
						floatingMenuContentDiv.style.borderTopLeftRadius = "0";
						break;
					case "TopRight":
						floatingMenuContentDiv.style.borderTopRightRadius = "0";
						break;
					case "BottomLeft":
						floatingMenuContentDiv.style.borderBottomLeftRadius = "0";
						break;
					case "BottomRight":
						floatingMenuContentDiv.style.borderBottomRightRadius = "0";
						break;
					default:
						break;
				}
			}
		}
	}

	export function div(): HTMLDivElement | undefined {
		return self;
	}

	export async function measureAndEmitNaturalWidth() {
		if (!measuringOngoingGuard) return;

		await tick();
		await document.fonts.ready;

		measuringOngoing = true;
		measuringOngoingGuard = true;
		await tick();

		const naturalWidth: number | undefined = floatingMenuContent?.div?.()?.clientWidth;

		measuringOngoing = false;
		await tick();
		measuringOngoingGuard = false;

		if (naturalWidth !== undefined && naturalWidth >= 0) {
			dispatch("naturalWidth", naturalWidth);
		}
	}

	function pointerMoveHandler(e: PointerEvent) {
		const target = e.target as HTMLElement | undefined;
		const ownSpawner: HTMLElement | undefined = self?.parentElement?.querySelector(":scope > [data-floating-menu-spawner]") || undefined;
		const targetSpawner: HTMLElement | undefined = target?.closest?.("[data-floating-menu-spawner]") || undefined;

		hoverTransfer(self, ownSpawner, targetSpawner);

		const notHoveringOverOwnSpawner = ownSpawner !== targetSpawner;
		if (strayCloses && notHoveringOverOwnSpawner && isPointerEventOutsideFloatingMenu(e, POINTER_STRAY_DISTANCE)) {
			dispatch("open", false);
		}

		const BUTTONS_LEFT = 0b0000_0001;
		const eventIncludesLmb = Boolean(e.buttons & BUTTONS_LEFT);
		if (!open && !eventIncludesLmb) {
			pointerStillDown = false;
			window.removeEventListener("pointerup", pointerUpHandler);
		}
	}

	function hoverTransfer(self: HTMLDivElement | undefined, ownSpawner: HTMLElement | undefined, targetSpawner: HTMLElement | undefined) {
		const getDepthFromAncestor = (item: Element, ancestor: Element): number | undefined => {
			let depth = 1;
			let parent = item.parentElement || undefined;
			while (parent) {
				if (parent === ancestor) return depth;
				parent = parent.parentElement || undefined;
				depth += 1;
			}
			return undefined;
		};

		const ownDescendantMenuSpawners = Array.from(self?.parentElement?.querySelectorAll("[data-floating-menu-spawner]") || []);
		let currentAncestor = (targetSpawner && ownSpawner?.parentElement) || undefined;
		
		while (currentAncestor) {
			const ownSpawnerDepthFromCurrentAncestor = ownSpawner && getDepthFromAncestor(ownSpawner, currentAncestor);
			const currentAncestor2 = currentAncestor;

			const listOfDescendantSpawners = Array.from(currentAncestor?.querySelectorAll("[data-floating-menu-spawner]") || []);
			const filteredListOfDescendantSpawners = listOfDescendantSpawners.filter((item: Element): boolean => {
				const notOurself = !ownDescendantMenuSpawners.includes(item);
				const notUnequalDepths = notOurself && getDepthFromAncestor(item, currentAncestor2) === ownSpawnerDepthFromCurrentAncestor;
				return notUnequalDepths && !(item as HTMLElement).getAttribute?.("data-floating-menu-spawner")?.includes("no-hover-transfer");
			});

			if (filteredListOfDescendantSpawners.length === 0) {
				currentAncestor = currentAncestor?.parentElement || undefined;
			} else {
				const foundTarget = filteredListOfDescendantSpawners.find((item: Element): boolean => item === targetSpawner);
				if (foundTarget) {
					dispatch("open", false);
					(foundTarget as HTMLElement).click();
				}
				break;
			}
		}
	}

	function keyDownHandler(e: KeyboardEvent) {
		if (escapeCloses && e.key === "Escape") {
			setTimeout(() => {
				if (!floatingMenuContainer?.querySelector("[data-floating-menu-content][data-escape-does-not-close]")) {
					dispatch("open", false);
				}
			}, 0);

			if (self) preventEscapeClosingParentFloatingMenu(self);
		}
	}

	function pointerDownHandler(e: PointerEvent) {
		if (isPointerEventOutsideFloatingMenu(e)) {
			dispatch("open", false);
			const eventIsForLmb = e.button === BUTTON_LEFT;
			if (eventIsForLmb) pointerStillDown = true;
		}
	}

	function pointerUpHandler(e: PointerEvent) {
		const eventIsForLmb = e.button === BUTTON_LEFT;
		if (pointerStillDown && eventIsForLmb) {
			pointerStillDown = false;
			window.removeEventListener("pointerup", pointerUpHandler);
			window.addEventListener("click", clickHandlerCapture, true);
		}
	}

	function clickHandlerCapture(e: MouseEvent) {
		e.stopPropagation();
		window.removeEventListener("click", clickHandlerCapture, true);
	}

	function isPointerEventOutsideFloatingMenu(e: PointerEvent, extraDistanceAllowed = 0): boolean {
		const allContainedFloatingMenus = [...(self?.querySelectorAll("[data-floating-menu-content]") || [])];
		return !allContainedFloatingMenus.find((element) => !isPointerEventOutsideMenuElement(e, element, extraDistanceAllowed));
	}

	function isPointerEventOutsideMenuElement(e: PointerEvent, element: Element, extraDistanceAllowed = 0): boolean {
		const floatingMenuBounds = element.getBoundingClientRect();

		if (floatingMenuBounds.left - e.clientX >= extraDistanceAllowed) return true;
		if (e.clientX - floatingMenuBounds.right >= extraDistanceAllowed) return true;
		if (floatingMenuBounds.top - e.clientY >= extraDistanceAllowed) return true;
		if (e.clientY - floatingMenuBounds.bottom >= extraDistanceAllowed) return true;

		return false;
	}
</script>

<div
	class={`floating-menu ${direction.toLowerCase()} ${type.toLowerCase()} ${className} ${extraClasses}`.trim()}
	style={`${styleName} ${extraStyles}`.trim() || undefined}
	bind:this={self}
	{...$$restProps}
>
	{#if displayTail}
		<div class="tail" bind:this={tail} />
	{/if}
	{#if displayContainer}
		<div class="floating-menu-container" bind:this={floatingMenuContainer}>
			<LayoutCol class="floating-menu-content" styles={{ "min-width": minWidthStyleValue }} {scrollableY} bind:this={floatingMenuContent} data-floating-menu-content>
				<slot />
			</LayoutCol>
		</div>
	{/if}
</div>

<style lang="scss" global>
	.floating-menu {
		position: absolute;
		width: 0;
		height: 0;
		display: flex;
		// Floating menus begin at a z-index of 1000
		z-index: 1000;
		--floating-menu-content-offset: 0;

		.tail {
			// Put the tail above the floating menu's shadow
			z-index: 10;
			// Draw over the application without being clipped by the containing panel's `overflow: hidden`
			position: fixed;

			&,
			&::before {
				width: 0;
				height: 0;
				border-style: solid;
			}

			&::before {
				content: "";
				position: absolute;
			}
		}

		.floating-menu-container {
			display: flex;

			.floating-menu-content {
				background: var(--color-2-mildblack);
				box-shadow: rgba(var(--color-0-black-rgb), 0.5) 0 2px 4px;
				border: 1px solid var(--color-3-darkgray);
				border-radius: 4px;
				color: var(--color-e-nearwhite);
				font-size: inherit;
				padding: 8px;
				z-index: 0;
				// Draw over the application without being clipped by the containing panel's `overflow: hidden`
				position: fixed;
				// Counteract the rightward shift caused by the border
				margin-left: -1px;
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
		}

		&.cursor .floating-menu-container .floating-menu-content {
			background: none;
			box-shadow: none;
			border-radius: 0;
			padding: 0;
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

		&.top .tail,
		&.topleft .tail,
		&.topright .tail {
			border-color: var(--color-3-darkgray) transparent transparent transparent;

			&::before {
				border-color: var(--color-2-mildblack) transparent transparent transparent;
				bottom: 0;
			}

			&,
			&::before {
				border-width: 8px 6px 0 6px;
				margin-left: -6px;
				margin-bottom: 2px;
			}
		}

		&.bottom .tail,
		&.bottomleft .tail,
		&.bottomright .tail {
			border-color: transparent transparent var(--color-3-darkgray) transparent;

			&::before {
				border-color: transparent transparent var(--color-2-mildblack) transparent;
				top: 0;
			}

			&,
			&::before {
				border-width: 0 6px 8px 6px;
				margin-left: -6px;
				margin-top: 2px;
			}
		}

		&.left .tail {
			border-color: transparent transparent transparent var(--color-3-darkgray);

			&::before {
				border-color: transparent transparent transparent var(--color-2-mildblack);
				right: 0;
			}

			&,
			&::before {
				border-width: 6px 0 6px 8px;
				margin-top: -6px;
				margin-right: 2px;
			}
		}

		&.right .tail {
			border-color: transparent var(--color-3-darkgray) transparent transparent;

			&::before {
				border-color: transparent var(--color-2-mildblack) transparent transparent;
				left: 0;
			}

			&,
			&::before {
				border-width: 6px 8px 6px 0;
				margin-top: -6px;
				margin-left: 2px;
			}
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
