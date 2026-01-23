<script lang="ts">
	import { getContext } from "svelte";

	import type { Editor } from "@graphite/editor";
	import type { LabeledShortcut } from "@graphite/messages";
	import type { TooltipState } from "@graphite/state-providers/tooltip";

	import FloatingMenu from "@graphite/components/layout/FloatingMenu.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import ShortcutLabel from "@graphite/components/widgets/labels/ShortcutLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	const tooltip = getContext<TooltipState>("tooltip");
	const editor = getContext<Editor>("editor");

	let self: FloatingMenu | undefined;

	$: label = parseMarkdown(filterTodo($tooltip.element?.getAttribute("data-tooltip-label")?.trim()));
	$: description = parseMarkdown(filterTodo($tooltip.element?.getAttribute("data-tooltip-description")?.trim()));
	$: shortcutJSON = $tooltip.element?.getAttribute("data-tooltip-shortcut")?.trim();
	$: shortcut = ((shortcutJSON) => {
		if (!shortcutJSON) return undefined;
		try {
			return JSON.parse(shortcutJSON) as LabeledShortcut;
		} catch {
			return undefined;
		}
	})(shortcutJSON);

	// TODO: Once all TODOs are replaced with real text, remove this function
	function filterTodo(text: string | undefined): string | undefined {
		if (text?.trim().toUpperCase() === "TODO" && !editor.handle.inDevelopmentMode()) return "";
		return text;
	}

	function parseMarkdown(markdown: string | undefined): string | undefined {
		if (!markdown) return undefined;

		let text = markdown.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&apos;");

		return (
			text
				// .split("\n")
				// .map((line) => line.trim())
				// .join("\n")
				// .split("\n\n")
				// .map((paragraph) => paragraph.replaceAll("\n", " "))
				// .join("\n\n")
				// Bold
				.replace(/\*\*((?:(?!\*\*).)+)\*\*/g, "<strong>$1</strong>")
				// Italic
				.replace(/\*([^*]+)\*/g, "<em>$1</em>")
				// Backticks
				.replace(/`([^`]+)`/g, "<code>$1</code>")
		);
	}
</script>

{#if label || description}
	<div class="tooltip" style:top={`${$tooltip.position.y}px`} style:left={`${$tooltip.position.x}px`}>
		<FloatingMenu open={true} type="Tooltip" direction="Bottom" bind:this={self}>
			{#if label || shortcut}
				<LayoutRow class="tooltip-header">
					{#if label}
						<TextLabel class="tooltip-label">{@html label}</TextLabel>
					{/if}
					{#if shortcut}
						<ShortcutLabel shortcut={{ shortcut }} />
					{/if}
				</LayoutRow>
			{/if}
			{#if description}
				<TextLabel class="tooltip-description">{@html description}</TextLabel>
			{/if}
		</FloatingMenu>
	</div>
{/if}

<style lang="scss" global>
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

			.tooltip-description {
				color: var(--color-b-lightgray);
			}
		}
	}
</style>
