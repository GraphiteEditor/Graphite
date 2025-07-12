<script lang="ts">
	import { tick } from "svelte";
	import type { Snippet } from "svelte";
	import type { SvelteHTMLElements } from "svelte/elements";

	import { platformIsMac } from "@graphite/utility-functions/platform";

	import { preventEscapeClosingParentFloatingMenu } from "@graphite/components/layout/FloatingMenu.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";

	type InputHTMLElementProps = SvelteHTMLElements["input"];
	type TextAreaHTMLElementProps = SvelteHTMLElements["textarea"];

	type CommonProps = {
		class?: string;
		classes?: Record<string, boolean>;
		style?: string;
		styles?: Record<string, string | number | undefined>;
		value: string;
		label?: string | undefined;
		spellcheck?: boolean;
		disabled?: boolean;
		textarea?: boolean;
		tooltip?: string | undefined;
		placeholder?: string | undefined;
		hideContextMenu?: boolean;
		children?: Snippet;
		onfocus?: (event: FocusEvent & { currentTarget: HTMLInputElement | HTMLTextAreaElement }) => void;
		oncommitText?: (arg1: string) => void;
		onkeydown?: (event: KeyboardEvent & { currentTarget: HTMLInputElement | HTMLTextAreaElement }) => void;
		onpointerdown?: (event: PointerEvent & { currentTarget: HTMLInputElement | HTMLTextAreaElement | HTMLLabelElement }) => void;
		ontextChangeCanceled?: () => void;
	};

	type Props = CommonProps &
		(CommonProps["textarea"] extends true
			? TextAreaHTMLElementProps // If 'textarea' is explicitly true
			: InputHTMLElementProps); // Otherwise (false, undefined, or missing)

	let {
		class: className = "",
		classes = {},
		style: styleName = "",
		styles = {},
		value = $bindable(""),
		label = undefined,
		spellcheck = false,
		disabled = false,
		textarea = false,
		tooltip = undefined,
		placeholder = undefined,
		hideContextMenu = false,
		onfocus,
		oncommitText,
		children,
		onpointerdown,
		ontextChangeCanceled,
	}: Props = $props();

	let inputOrTextarea: HTMLInputElement | HTMLTextAreaElement | undefined = $state();
	let id = String(Math.random()).substring(2);
	let macKeyboardLayout = platformIsMac();
	let local = $state<string>(value);

	$effect.pre(() => {
		local = value;
	});

	// Select (highlight) all the text. For technical reasons, it is necessary to pass the current text.
	export function selectAllText(currentText: string) {
		if (!inputOrTextarea) return;

		// Setting the value directly is required to make the following `select()` call work
		local = currentText;
		// Wait for UI to update
		tick().then(() => inputOrTextarea?.select());
	}

	export function focus() {
		inputOrTextarea?.focus();
	}

	export function unFocus() {
		inputOrTextarea?.blur();
	}

	export function element(): HTMLInputElement | HTMLTextAreaElement | undefined {
		return inputOrTextarea;
	}

	function cancel() {
		local = value;
		ontextChangeCanceled?.();
		unFocus();

		if (inputOrTextarea) preventEscapeClosingParentFloatingMenu(inputOrTextarea);
	}

	function onkeydownInput(e: KeyboardEvent & { currentTarget: HTMLInputElement }) {
		if (e.key === "Enter") {
			oncommitText?.(local);
			unFocus();
		}
		if (e.key === "Escape") {
			cancel();
		}
	}

	function onkeydownTextArea(e: KeyboardEvent & { currentTarget: HTMLTextAreaElement }) {
		if ((macKeyboardLayout ? e.metaKey : e.ctrlKey) && e.key === "Enter") {
			oncommitText?.(local ?? "");
			unFocus();
		}
		if (e.key === "Escape") {
			cancel();
		}
	}

	function onblur() {
		oncommitText?.(local ?? "");
	}

	// If oncommitText listener is defined
	// let the text be bound to the local state
	let textBind = $derived.by(() => {
		return {
			get current() {
				return oncommitText ? local : value;
			},
			set current(v: string) {
				if (oncommitText) {
					local = v;
				} else {
					value = v;
				}
			},
		};
	});
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
			bind:value={textBind.current}
			{onfocus}
			{onblur}
			onkeydown={onkeydownInput}
			{onpointerdown}
			oncontextmenu={(e) => hideContextMenu && e.preventDefault()}
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
			bind:value={textBind.current}
			{onfocus}
			{onblur}
			onkeydown={onkeydownTextArea}
			{onpointerdown}
			oncontextmenu={(e) => hideContextMenu && e.preventDefault()}
		></textarea>
	{/if}
	{#if label}
		<label for={`field-input-${id}`} {onpointerdown}>{label}</label>
	{/if}
	{@render children?.()}
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
			unicode-bidi: plaintext;

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
