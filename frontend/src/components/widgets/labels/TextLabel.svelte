<script lang="ts">
	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};
	let styleName = "";
	export { styleName as style };
	export let styles: Record<string, string | number | undefined> = {};
	export let disabled = false;
	export let bold = false;
	export let italic = false;
	export let tableAlign = false;
	export let minWidth = 0;
	export let multiline = false;
	export let tooltip: string | undefined = undefined;

	$: extraClasses = Object.entries(classes)
		.flatMap(([className, stateName]) => (stateName ? [className] : []))
		.join(" ");
	$: extraStyles = Object.entries(styles)
		.flatMap((styleAndValue) => (styleAndValue[1] !== undefined ? [`${styleAndValue[0]}: ${styleAndValue[1]};`] : []))
		.join(" ");
</script>

<span
	class={`text-label ${className} ${extraClasses}`.trim()}
	class:disabled
	class:bold
	class:italic
	class:multiline
	class:table-align={tableAlign}
	style:min-width={minWidth > 0 ? `${minWidth}px` : ""}
	style={`${styleName} ${extraStyles}`.trim() || undefined}
	title={tooltip}
>
	<slot />
</span>

<style lang="scss" global>
	.text-label {
		line-height: 18px;
		white-space: nowrap;
		// Force Safari to not draw a text cursor, even though this element has `user-select: none`
		cursor: default;

		&.disabled {
			color: var(--color-8-uppergray);
		}

		&.bold {
			font-weight: 700;
		}

		&.italic {
			font-style: italic;
		}

		&.multiline {
			white-space: pre-wrap;
			margin: 4px 0;
		}

		&.table-align {
			flex: 0 0 30%;
			text-align: right;
		}
	}
</style>
