<script lang="ts">
	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};
	let styleName = "";
	export { styleName as style };
	export let styles: Record<string, string | number | undefined> = {};
	export let tooltip: string | undefined = undefined;
	// TODO: Add middle-click drag scrolling
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
	class={`layout-row ${className} ${extraClasses}`.trim()}
	class:scrollable-x={scrollableX}
	class:scrollable-y={scrollableY}
	style={`${styleName} ${extraStyles}`.trim() || undefined}
	title={tooltip}
	bind:this={self}
	on:auxclick
	on:blur
	on:click
	on:dblclick
	on:dragend
	on:dragleave
	on:dragover
	on:dragstart
	on:drop
	on:mousedown
	on:mouseup
	on:pointerdown
	on:pointerenter
	on:pointerleave
	on:scroll
	{...$$restProps}
>
	<slot />
</div>

<!-- Unused (each impacts performance, see <https://github.com/GraphiteEditor/Graphite/issues/1877>):
on:contextmenu
on:copy
on:cut
on:drag
on:dragenter
on:focus
on:fullscreenchange
on:fullscreenerror
on:gotpointercapture
on:keydown
on:keypress
on:keyup
on:lostpointercapture
on:mouseenter
on:mouseleave
on:mousemove
on:mouseout
on:mouseover
on:paste
on:pointercancel
on:pointermove
on:pointerout
on:pointerover
on:pointerup
on:select
on:touchcancel
on:touchend
-->

<style lang="scss" global>
	.layout-row {
		display: flex;
		flex-direction: row;
		flex-grow: 1;
	}
</style>
