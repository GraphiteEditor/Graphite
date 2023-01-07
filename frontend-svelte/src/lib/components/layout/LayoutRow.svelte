<script lang="ts">
	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};
	let styleName = "";
	export { styleName as style };
	export let styles: Record<string, string | number> = {};
	export let tooltip: string | undefined = undefined;
	export let scrollableX: boolean = false;
	export let scrollableY: boolean = false;

	let divElement: HTMLDivElement;

	$: extraClasses = Object.entries(classes)
		.flatMap((classAndState) => (classAndState[1] ? [classAndState[0]] : []))
		.join(" ");
	$: extraStyles = Object.entries(styles)
		.map((styleAndValue) => `${styleAndValue[0]}: ${styleAndValue[1]};`)
		.join(" ");

	export function div(): HTMLDivElement {
		return divElement;
	}
</script>

<div
	class={`layout-row ${className} ${extraClasses}`.trim()}
	class:scrollable-x={scrollableX}
	class:scrollable-y={scrollableY}
	style={`${styleName} ${extraStyles}`.trim()}
	title={tooltip}
	bind:this={divElement}
	on:click
	on:pointerdown
	on:dragleave
	on:dragover
	on:dragstart
	on:dragend
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
