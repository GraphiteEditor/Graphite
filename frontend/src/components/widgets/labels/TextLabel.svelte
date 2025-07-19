<script lang="ts">
	import type { Snippet } from "svelte";
	import type { SvelteHTMLElements } from "svelte/elements";

	type LabelHTMLElementProps = SvelteHTMLElements["label"];

	type Props = {
		class?: string;
		classes?: Record<string, boolean>;
		style?: string;
		styles?: Record<string, string | number | undefined>;
		disabled?: boolean;
		bold?: boolean;
		italic?: boolean;
		centerAlign?: boolean;
		tableAlign?: boolean;
		minWidth?: number;
		multiline?: boolean;
		tooltip?: string | undefined;
		checkboxId?: bigint | undefined;
		children?: Snippet;
	} & LabelHTMLElementProps;

	let {
		class: className = "",
		classes = {},
		style: styleName = "",
		styles = {},
		disabled = false,
		bold = false,
		italic = false,
		centerAlign = false,
		tableAlign = false,
		minWidth = 0,
		multiline = false,
		tooltip = undefined,
		checkboxId = undefined,
		children,
	}: Props = $props();

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
</script>

<label
	class={`text-label ${className} ${extraClasses}`.trim()}
	class:disabled
	class:bold
	class:italic
	class:multiline
	class:center-align={centerAlign}
	class:table-align={tableAlign}
	style:min-width={minWidth > 0 ? `${minWidth}px` : undefined}
	style={`${styleName} ${extraStyles}`.trim() || undefined}
	title={tooltip}
	for={checkboxId !== undefined ? `checkbox-input-${checkboxId}` : undefined}
>
	{@render children?.()}
</label>

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

		&.center-align {
			text-align: center;
		}

		&.table-align {
			flex: 0 0 30%;
			text-align: right;
		}
	}
</style>
