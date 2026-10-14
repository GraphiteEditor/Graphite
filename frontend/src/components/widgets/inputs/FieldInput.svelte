<script lang="ts">
	import { createEventDispatcher, onDestroy, onMount } from "svelte";
	import { preventEscapeClosingParentFloatingMenu } from "/src/components/layout/FloatingMenu.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import { operatingSystem } from "/src/utility-functions/platform";
	import { widgetSizingStyles } from "/src/utility-functions/widgets";
	import type { ActionShortcut, WidgetSizing } from "/wrapper/pkg/graphite_wasm_wrapper";

	const dispatch = createEventDispatcher<{
		value: string;
		textFocused: undefined;
		textChanged: undefined;
		textChangeCanceled: undefined;
	}>();

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};
	let styleName = "";
	export { styleName as style };
	export let styles: Record<string, string | number | undefined> = {};
	export let value: string;
	export let label: string | undefined = undefined;
	export let spellcheck = false;
	export let disabled = false;
	export let monospace = false;
	export let narrow = false;
	export let textarea = false;
	export let sizing: Partial<WidgetSizing> | undefined = undefined;
	export let tooltipLabel: string | undefined = undefined;
	export let tooltipDescription: string | undefined = undefined;
	export let tooltipShortcut: ActionShortcut | undefined = undefined;
	export let placeholder: string | undefined = undefined;
	export let hideContextMenu = false;

	let self: LayoutRow | undefined;
	let inputOrTextarea: HTMLInputElement | HTMLTextAreaElement | undefined;
	let id = String(Math.random()).substring(2);
	let macKeyboardLayout = operatingSystem() === "Mac";

	// While editing, the field keeps its prior width since the label hides and the text no longer sizes it
	let editing = false;
	let unfocusedWidth: number | undefined = undefined;
	let editingWidth: number | undefined = undefined;
	const resizeObserver = new ResizeObserver(([entry]) => {
		if (!editing) unfocusedWidth = entry.borderBoxSize[0].inlineSize;
	});

	$: dispatch("value", value);

	onMount(() => {
		const element = self?.div();
		if (element) resizeObserver.observe(element);
	});

	onDestroy(() => resizeObserver.disconnect());

	function onFocus() {
		editing = true;
		editingWidth = unfocusedWidth;

		dispatch("textFocused");
	}

	function onBlur() {
		editing = false;
		editingWidth = undefined;

		dispatch("textChanged");
	}

	// Select (highlight) all the text. For technical reasons, it is necessary to pass the current text.
	export function selectAllText(currentText: string) {
		if (!inputOrTextarea) return;

		// Setting the value directly is required to make the following `select()` call work
		inputOrTextarea.value = currentText;
		inputOrTextarea.select();
	}

	export function focus() {
		inputOrTextarea?.focus();
	}

	export function unFocus() {
		inputOrTextarea?.blur();
	}

	export function getValue(): string {
		return inputOrTextarea?.value || "";
	}

	export function setInputElementValue(value: string) {
		if (!inputOrTextarea) return;

		inputOrTextarea.value = value;
	}

	export function element(): HTMLInputElement | HTMLTextAreaElement | undefined {
		return inputOrTextarea;
	}

	function cancel() {
		dispatch("textChangeCanceled");

		if (inputOrTextarea) preventEscapeClosingParentFloatingMenu(inputOrTextarea);
	}
</script>

<!-- This is a base component, extended by others like NumberInput and TextInput. It should not be used directly. -->
<LayoutRow
	class={`field-input ${className}`}
	classes={{ disabled, narrow, monospace, ...classes }}
	style={styleName}
	styles={{
		...styles,
		...widgetSizingStyles(sizing),
		...(editingWidth !== undefined ? { width: `${editingWidth}px` } : {}),
	}}
	{tooltipLabel}
	{tooltipDescription}
	{tooltipShortcut}
	bind:this={self}
	{...$$restProps}
>
	{#if label}
		<label for={`field-input-${id}`} on:pointerdown>{label}</label>
		<div class="after-label-spacer"></div>
	{/if}
	{#if !textarea}
		<input
			type="text"
			class:has-label={label}
			id={`field-input-${id}`}
			{spellcheck}
			{disabled}
			{placeholder}
			bind:this={inputOrTextarea}
			bind:value
			on:focus={onFocus}
			on:blur={onBlur}
			on:change={() => dispatch("textChanged")}
			on:keydown={(e) => e.key === "Enter" && dispatch("textChanged")}
			on:keydown={(e) => e.key === "Escape" && cancel()}
			on:keydown
			on:pointerdown
			on:contextmenu={(e) => hideContextMenu && e.preventDefault()}
			data-input-element
		/>
	{:else}
		<textarea
			class:has-label={label}
			id={`field-input-${id}`}
			class="scrollable-y"
			data-scrollable-y
			{spellcheck}
			{disabled}
			bind:this={inputOrTextarea}
			bind:value
			on:focus={onFocus}
			on:blur={onBlur}
			on:change={() => dispatch("textChanged")}
			on:keydown={(e) => (macKeyboardLayout ? e.metaKey : e.ctrlKey) && e.key === "Enter" && dispatch("textChanged")}
			on:keydown={(e) => e.key === "Escape" && cancel()}
			on:keydown
			on:pointerdown
			on:contextmenu={(e) => hideContextMenu && e.preventDefault()}
			data-input-element
		></textarea>
	{/if}
	<slot />
</LayoutRow>

<style lang="scss">
	.field-input {
		height: auto;
		position: relative;
		overflow: hidden;
		border-radius: 2px;
		background: var(--color-1-nearblack);

		// Firefox before 152 (June 2026) and Safari before 26.2 (Dec 2025) lack this, so they assume the input text field is 30px wide, as set by `min-width: 30px` below
		@supports (field-sizing: content) {
			input[type="text"]:not(:focus) {
				field-sizing: content;
				flex: 1 1 auto;
				width: auto;
				min-width: auto;

				&.has-label {
					flex: 0 0 auto;
				}
			}

			label {
				flex: 1 1 0;
			}
		}

		&.narrow.narrow {
			--widget-height: 20px;
		}

		&.monospace {
			input,
			textarea {
				font-family: "Source Code Pro", monospace;
			}
		}

		label {
			flex: 1 1 auto;
			line-height: calc(var(--widget-height) - 6px);
			padding: 3px 0;
			margin-left: 8px;
			overflow: hidden;
			text-overflow: ellipsis;
			white-space: nowrap;

			// This keeps the label, when abbreviated by ellipsis, from touching the number value.
			// This only shrinks after the label has shrunk to 0 (which is why this is a separate element, not margin or padding on the label or input).
			+ .after-label-spacer {
				width: 4px;
				flex: 0 1 auto;
			}
		}

		&:not(.disabled) label {
			cursor: text;
		}

		input,
		textarea {
			flex: 1 1 100%;
			width: 0;
			min-width: 30px;
			height: calc(var(--widget-height) - 6px);
			line-height: calc(var(--widget-height) - 6px);
			margin: 0 8px;
			padding: 3px 0;
			outline: none; // Ok for input/textarea element
			border: none;
			background: none;
			color: var(--color-e-nearwhite);
			caret-color: var(--color-e-nearwhite);
			unicode-bidi: plaintext;
		}

		input {
			&:not(:focus).has-label {
				text-align: right;
				margin-left: 0;
				margin-right: 8px;
			}

			&:focus {
				text-align: left;
			}
		}

		&:has(input[type="text"]:focus) {
			label,
			.after-label-spacer {
				display: none;
			}
		}

		textarea {
			min-height: calc((var(--widget-height) - 6px) * 3);
			margin: 3px;
			padding: 0 5px;
			box-sizing: border-box;
			resize: vertical;
		}

		&.disabled {
			background: var(--color-2-mildblack);

			label,
			input,
			textarea {
				color: var(--color-8-uppergray);
			}

			input {
				pointer-events: none;
			}
		}
	}
</style>
