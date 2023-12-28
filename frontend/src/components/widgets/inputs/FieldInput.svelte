<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import { platformIsMac } from "@graphite/utility-functions/platform";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";

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
	export let textarea = false;
	export let tooltip: string | undefined = undefined;
	export let placeholder: string | undefined = undefined;
	export let hideContextMenu = false;

	let inputOrTextarea: HTMLInputElement | HTMLTextAreaElement | undefined;
	let id = String(Math.random()).substring(2);
	let macKeyboardLayout = platformIsMac();

	$: inputValue = value;

	$: dispatch("value", inputValue);

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
</script>

<!-- This is a base component, extended by others like NumberInput and TextInput. It should not be used directly. -->
<LayoutRow class={`field-input ${className}`} classes={{ disabled, ...classes }} style={styleName} {styles} {tooltip}>
	{#if !textarea}
		<input
			type="text"
			class:has-label={label}
			id={`field-input-${id}`}
			{spellcheck}
			{disabled}
			{placeholder}
			bind:this={inputOrTextarea}
			bind:value={inputValue}
			on:focus={() => dispatch("textFocused")}
			on:blur={() => dispatch("textChanged")}
			on:change={() => dispatch("textChanged")}
			on:keydown={(e) => e.key === "Enter" && dispatch("textChanged")}
			on:keydown={(e) => e.key === "Escape" && dispatch("textChangeCanceled")}
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
			bind:value={inputValue}
			on:focus={() => dispatch("textFocused")}
			on:blur={() => dispatch("textChanged")}
			on:change={() => dispatch("textChanged")}
			on:keydown={(e) => (macKeyboardLayout ? e.metaKey : e.ctrlKey) && e.key === "Enter" && dispatch("textChanged")}
			on:keydown={(e) => e.key === "Escape" && dispatch("textChangeCanceled")}
			on:pointerdown
			on:contextmenu={(e) => hideContextMenu && e.preventDefault()}
		/>
	{/if}
	{#if label}
		<label for={`field-input-${id}`} on:pointerdown>{label}</label>
	{/if}
	<slot />
</LayoutRow>

<style lang="scss" global>
	.field-input {
		min-width: 80px;
		height: auto;
		position: relative;
		border-radius: 2px;
		background: var(--color-1-nearblack);
		flex-direction: row-reverse;

		label {
			flex: 0 0 auto;
			line-height: 18px;
			padding: 3px 0;
			padding-right: 4px;
			margin-left: 8px;
			overflow: hidden;
			text-overflow: ellipsis;
			white-space: nowrap;
		}

		&:not(.disabled) label {
			cursor: text;
		}

		input,
		textarea {
			flex: 1 1 100%;
			width: 0;
			min-width: 30px;
			height: 18px;
			line-height: 18px;
			margin: 0 8px;
			padding: 3px 0;
			outline: none; // Ok for input/textarea element
			border: none;
			background: none;
			color: var(--color-e-nearwhite);
			caret-color: var(--color-e-nearwhite);

			&::selection {
				background-color: var(--color-4-dimgray);

				// Target only Safari
				@supports (background: -webkit-named-image(i)) {
					& {
						// Setting an alpha value opts out of Safari's "fancy" (but not visible on dark backgrounds) selection highlight rendering
						// https://stackoverflow.com/a/71753552/775283
						background-color: rgba(var(--color-4-dimgray-rgb), calc(254 / 255));
					}
				}
			}
		}

		input {
			&:not(:focus).has-label {
				text-align: right;
				margin-left: 0;
				margin-right: 8px;
			}

			&:focus {
				text-align: left;

				& + label {
					display: none;
				}
			}
		}

		textarea {
			min-height: calc(18px * 3);
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
				// Disables drag-selecting the text, since `user-select: none` doesn't work for input elements
				pointer-events: none;
			}
		}
	}
</style>
