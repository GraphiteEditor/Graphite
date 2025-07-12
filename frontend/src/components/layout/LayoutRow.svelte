<script lang="ts">
	import type { Snippet } from "svelte";
	import type { SvelteHTMLElements } from "svelte/elements";

	type DivHTMLElementProps = SvelteHTMLElements["div"];

	type Props = {
		class?: string;
		classes?: Record<string, boolean>;
		style?: string;
		styles?: Record<string, string | number | undefined>;
		tooltip?: string | undefined;
		// TODO: Add middle-click drag scrolling
		scrollableX?: boolean;
		scrollableY?: boolean;
		children?: Snippet;
	} & DivHTMLElementProps;

	let { class: className = "", classes = {}, style: styleName = "", styles = {}, tooltip = undefined, scrollableX = false, scrollableY = false, children, ...rest }: Props = $props();

	let self: HTMLDivElement | undefined = $state();

	let extraClasses = $derived(
		Object.entries(classes)
			.flatMap(([className, stateName]) => (stateName ? [className] : []))
			.join(" "),
	);
	let extraStyles = $derived(
		Object.entries(styles)
			.flatMap((styleAndValue) => (styleAndValue[1] !== undefined ? [`${styleAndValue[0]}: ${styleAndValue[1]};`] : []))
			.join(" "),
	);

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
	{...rest}
>
	{@render children?.()}
</div>

<!-- Unused (each impacts performance, see <https://github.com/GraphiteEditor/Graphite/issues/1877>):
 onauxclick={bubble('auxclick')}
onblur={bubble('blur')}
onclick={bubble('click')}
ondblclick={bubble('dblclick')}
ondragend={bubble('dragend')}
ondragleave={bubble('dragleave')}
ondragover={bubble('dragover')}
ondragstart={bubble('dragstart')}
ondrop={bubble('drop')}
onmouseup={bubble('mouseup')}
onpointerdown={bubble('pointerdown')}
onpointerenter={bubble('pointerenter')}
onpointerleave={bubble('pointerleave')}
onscroll={bubble('scroll')}
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
on:mousedown
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
