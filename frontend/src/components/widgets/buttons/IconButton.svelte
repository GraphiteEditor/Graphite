<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import IconLabel from "/src/components/widgets/labels/IconLabel.svelte";
	import type { IconName, IconSize } from "/src/icons";
	import type { ActionShortcut, DragDropKinds } from "/wrapper/pkg/graphite_wasm_wrapper";

	// Content
	export let icon: IconName;
	export let hoverIcon: IconName | undefined = undefined;
	export let size: IconSize;
	export let disabled = false;
	// Styling
	export let emphasized = false;
	// Tooltips
	export let tooltipLabel: string | undefined = undefined;
	export let tooltipDescription: string | undefined = undefined;
	export let tooltipShortcut: ActionShortcut | undefined = undefined;
	// Callbacks
	export let action: (e?: MouseEvent) => void;
	// Fired when a draggable item, of a kind given by `dragDropKinds`, is dropped onto this button. The button lists those kinds in its
	// `data-drag-droppable` attribute, where the consumer of the drag interaction looks for them before dispatching a `dragdrop` event on it.
	export let actionDragDrop: (() => void) | undefined = undefined;
	export let dragDropKinds: DragDropKinds = "None";

	$: droppableKinds = actionDragDrop ? { None: undefined, Layers: "layers", LayersAndChainNodes: "layers chain-nodes" }[dragDropKinds] : undefined;

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	$: extraClasses = Object.entries(classes)
		.flatMap(([className, stateName]) => (stateName ? [className] : []))
		.join(" ");

	// Element-level listener for the `dragdrop` custom event that consumers dispatch when something is dropped on this button
	let buttonElement: HTMLButtonElement | undefined;
	function handleDragDrop() {
		actionDragDrop?.();
	}
	onMount(() => buttonElement?.addEventListener("dragdrop", handleDragDrop));
	onDestroy(() => buttonElement?.removeEventListener("dragdrop", handleDragDrop));

	// After a pointer click, the new state's icon replaces the hover icon until the pointer leaves and comes back
	let hoverIconSuppressed = false;
	$: showsHoverIcon = Boolean(hoverIcon) && !disabled && !hoverIconSuppressed;

	function click(e: MouseEvent) {
		if (e.detail > 0) hoverIconSuppressed = true;
		action(e);
	}
</script>

<button
	class={`icon-button size-${size} ${className} ${extraClasses}`.trim()}
	class:hover-icon={showsHoverIcon}
	class:disabled
	class:emphasized
	bind:this={buttonElement}
	on:click={click}
	on:pointerleave={() => (hoverIconSuppressed = false)}
	{disabled}
	data-tooltip-label={tooltipLabel}
	data-tooltip-description={tooltipDescription}
	data-tooltip-shortcut={tooltipShortcut?.shortcut ? JSON.stringify(tooltipShortcut.shortcut) : undefined}
	data-drag-droppable={droppableKinds}
	data-icon-button
	tabindex={emphasized ? -1 : 0}
	{...$$restProps}
>
	<IconLabel {icon} />
	{#if hoverIcon && showsHoverIcon}
		<IconLabel icon={hoverIcon} />
	{/if}
</button>

<style lang="scss">
	.icon-button {
		display: flex;
		justify-content: center;
		align-items: center;
		flex: 0 0 auto;
		margin: 0;
		padding: 0;
		border: none;
		border-radius: 2px;
		background: none;

		svg {
			fill: var(--color-e-nearwhite);
		}

		// The `where` pseudo-class does not contribute to specificity
		& + :where(.icon-button) {
			margin-left: 0;
		}

		&:hover {
			background: var(--color-5-dullgray);
		}

		&.hover-icon {
			&:not(:hover) .icon-label:nth-of-type(2) {
				display: none;
			}

			&:hover .icon-label:nth-of-type(1) {
				display: none;
			}
		}

		&.disabled {
			background: none;

			svg {
				fill: var(--color-8-uppergray);
			}
		}

		&.emphasized {
			background: var(--color-e-nearwhite);

			svg {
				fill: var(--color-2-mildblack);
			}
		}

		&.size-12 {
			width: 12px;
			height: 12px;
		}

		&.size-16 {
			width: 16px;
			height: 16px;
		}

		&.size-24 {
			width: 24px;
			height: 24px;
		}

		&.size-32 {
			width: 32px;
			height: 32px;
		}
	}
</style>
