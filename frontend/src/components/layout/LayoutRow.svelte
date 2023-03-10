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

	let self: HTMLDivElement;

	$: extraClasses = Object.entries(classes)
		.flatMap((classAndState) => (classAndState[1] ? [classAndState[0]] : []))
		.join(" ");
	$: extraStyles = Object.entries(styles)
		.flatMap((styleAndValue) => (styleAndValue[1] !== undefined ? [`${styleAndValue[0]}: ${styleAndValue[1]};`] : []))
		.join(" ");

	export function div(): HTMLDivElement {
		return self;
	}
</script>

<div
	class={`layout-row ${className} ${extraClasses}`.trim()}
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
	on:wheel
	on:drag
	on:dragend
	on:dragenter
	on:dragstart
	on:dragleave
	on:dragover
	on:drop
	on:touchcancel
	on:touchend
	on:touchmove
	on:touchstart
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
	.layout-row {
		display: flex;
		flex-direction: row;
		flex-grow: 1;
	}
</style>
