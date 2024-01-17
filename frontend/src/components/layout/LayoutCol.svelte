<script lang="ts">
	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};
	let styleName = "";
	export { styleName as style };
	export let styles: Record<string, string | number | undefined> = {};
	export let tooltip: string | undefined = undefined;
	export let scrollableX = false;
	export let scrollableY = false;

	let self: HTMLDivElement | undefined;

	$: extraClasses = Object.entries(classes)
		.flatMap(([className, stateName]) => (stateName ? [className] : []))
		.join(" ");
	$: extraStyles = Object.entries(styles)
		.flatMap((styleAndValue) => (styleAndValue[1] !== undefined ? [`${styleAndValue[0]}: ${styleAndValue[1]};`] : []))
		.join(" ");

	export function div(): HTMLDivElement | undefined {
		return self;
	}
</script>

<!-- Excluded events because these require `|passive` or `|nonpassive` modifiers. Use a <div> for these instead: `on:wheel`, `on:touchmove`, `on:touchstart` -->
<div
	data-scrollable-x={scrollableX ? "" : undefined}
	data-scrollable-y={scrollableY ? "" : undefined}
	class={`layout-col ${className} ${extraClasses}`.trim()}
	class:scrollable-x={scrollableX}
	class:scrollable-y={scrollableY}
	style={`${styleName} ${extraStyles}`.trim() || undefined}
	title={tooltip}
	bind:this={self}
	on:focus
	on:blur
	on:fullscreenchange
	on:fullscreenerror
	on:scroll
	on:cut
	on:copy
	on:paste
	on:keydown
	on:keypress
	on:keyup
	on:auxclick
	on:click
	on:contextmenu
	on:dblclick
	on:mousedown
	on:mouseenter
	on:mouseleave
	on:mousemove
	on:mouseover
	on:mouseout
	on:mouseup
	on:select
	on:drag
	on:dragend
	on:dragenter
	on:dragstart
	on:dragleave
	on:dragover
	on:drop
	on:touchcancel
	on:touchend
	on:pointerover
	on:pointerenter
	on:pointerdown
	on:pointermove
	on:pointerup
	on:pointercancel
	on:pointerout
	on:pointerleave
	on:gotpointercapture
	on:lostpointercapture
	{...$$restProps}
>
	<slot />
</div>

<style lang="scss" global>
	.layout-col {
		display: flex;
		flex-direction: column;
		flex-grow: 1;
	}
</style>
