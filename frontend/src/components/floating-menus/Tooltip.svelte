<script lang="ts">
	import { getContext } from "svelte";
	import FloatingMenu from "/src/components/layout/FloatingMenu.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import ShortcutLabel from "/src/components/widgets/labels/ShortcutLabel.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import type { TooltipStore } from "/src/stores/tooltip";
	import { parseMarkdown } from "/src/utility-functions/markdown";
	import type { EditorWrapper, LabeledShortcut } from "/wrapper/pkg/graphite_wasm_wrapper";

	const tooltip = getContext<TooltipStore>("tooltip");
	const editor = getContext<EditorWrapper>("editor");

	// The tooltip a component shows for part of itself, given with the bounds of that part, rather than the hovered element's at the pointer
	export let given: { label: string; description: string; code: string; bounds: { left: number; top: number; bottom: number } } | undefined = undefined;

	let self: FloatingMenu | undefined;

	$: element = given ? undefined : $tooltip.element;
	$: label = parseMarkdown(filterTodo((given?.label || element?.getAttribute("data-tooltip-label"))?.trim()));
	$: description = parseMarkdown(filterTodo((given?.description || element?.getAttribute("data-tooltip-description"))?.trim()));
	$: code = parseMarkdown((given?.code || element?.getAttribute("data-tooltip-code"))?.trim());
	$: shortcutJSON = element?.getAttribute("data-tooltip-shortcut")?.trim();
	// A given tooltip stands below its bounds from their left, or above them where there's no room below, rather than clearing the pointer below it
	$: position = given ? { x: given.bounds.left, y: (given.bounds.top + given.bounds.bottom) / 2 } : $tooltip.position;
	$: gap = given ? (given.bounds.bottom - given.bounds.top) / 2 : undefined;
	$: shortcut = ((shortcutJSON) => {
		if (!shortcutJSON) return undefined;
		try {
			const parsed: LabeledShortcut = JSON.parse(shortcutJSON);
			if (!Array.isArray(parsed)) return undefined;

			return parsed;
		} catch {
			return undefined;
		}
	})(shortcutJSON);

	// TODO: Once all TODOs are replaced with real text, remove this function
	function filterTodo(text: string | undefined): string | undefined {
		if (text?.trim().toUpperCase() === "TODO" && !editor.inDevelopmentMode()) return "";
		return text;
	}
</script>

{#if label || description}
	<div class="tooltip" style:top={`${position.y}px`} style:left={`${position.x}px`}>
		<FloatingMenu open={true} type="Tooltip" direction="Bottom" alignment={given ? "Start" : "Center"} {gap} bind:this={self}>
			{#if label || shortcut || code}
				<LayoutRow class="tooltip-header">
					{#if label}
						<TextLabel class="tooltip-label">{@html label}</TextLabel>
					{/if}
					{#if shortcut}
						<ShortcutLabel shortcut={{ shortcut }} />
					{/if}
					{#if code}
						<TextLabel class="tooltip-code" monospace={true}>{@html code}</TextLabel>
					{/if}
				</LayoutRow>
			{/if}
			{#if description}
				<TextLabel class="tooltip-description">{@html description}</TextLabel>
			{/if}
		</FloatingMenu>
	</div>
{/if}

<style lang="scss">
	.tooltip {
		position: absolute;
		pointer-events: none;
		width: 0;
		height: 0;

		.floating-menu-content {
			max-width: Min(400px, 50vw);

			.tooltip-header + .tooltip-description {
				margin-top: 4px;
			}

			.text-label {
				white-space: pre-wrap;
			}

			.text-label + .shortcut-label {
				margin-left: 8px;
			}

			.tooltip-code {
				margin-left: auto;
				padding-left: 8px;
				color: var(--color-b-lightgray);

				strong {
					color: var(--color-e-nearwhite);
				}
			}

			.tooltip-description {
				color: var(--color-b-lightgray);
			}
		}
	}
</style>
