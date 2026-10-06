<script lang="ts">
	import { createEventDispatcher, getContext, onDestroy } from "svelte";
	import MenuList from "/src/components/floating-menus/MenuList.svelte";
	import Tooltip from "/src/components/floating-menus/Tooltip.svelte";
	import FloatingMenu, { preventEscapeClosingParentFloatingMenu } from "/src/components/layout/FloatingMenu.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import IconButton from "/src/components/widgets/buttons/IconButton.svelte";
	import NumberInput from "/src/components/widgets/inputs/NumberInput.svelte";
	import Separator from "/src/components/widgets/labels/Separator.svelte";
	import type { MathExpressionAnalysis, MathExpressionEvaluation, MathExpressionStore } from "/src/stores/math-expression";
	import { defaultDigit, parseDecimal } from "/src/utility-functions/decimal";
	import type { Digit } from "/src/utility-functions/decimal";
	import { fromMathGlyphs, mathGlyph, operatorGlyph } from "/src/utility-functions/math-expression";
	import { operatingSystem } from "/src/utility-functions/platform";
	import type {
		ActionShortcut,
		MathExpressionCompletions,
		MathExpressionRange,
		MathExpressionSignature,
		MathExpressionToken,
		MathExpressionTooltip,
		MenuListEntry,
		WidgetId,
	} from "/wrapper/pkg/graphite_wasm_wrapper";

	const dispatch = createEventDispatcher<{ commitText: string; updateText: string; startHistoryTransaction: undefined; commitHistoryTransaction: undefined }>();
	const analyses = getContext<MathExpressionStore>("mathExpression");

	// Content
	export let value: string;
	export let tokens: MathExpressionToken[] = [];
	export let tooltips: MathExpressionTooltip[] = [];
	export let errors: MathExpressionRange[] = [];
	export let disabled = false;
	export let widgetId: WidgetId;
	export let analyze: (source: string, caret?: number, asked?: boolean) => void;
	export let evaluate: (source: string) => void;
	// Tooltips
	export let tooltipLabel: string | undefined = undefined;
	export let tooltipDescription: string | undefined = undefined;
	export let tooltipShortcut: ActionShortcut | undefined = undefined;

	// A selection in the text, by its indices, rather than in the DOM
	type TextRange = { anchor: number; focus: number };
	type Anchor = { left: number; bottom: number };
	type Snapshot = { text: string; selection: TextRange };

	const UNDO_LIMIT = 100;
	const PAIRS: Record<string, string> = { "(": ")", "[": "]", "{": "}" };
	const NO_ERRORS: MathExpressionRange[] = [];

	const WORD_CHARACTER = /^[\p{L}\p{N}]$/u;

	const ordered = ({ anchor, focus }: TextRange): [number, number] => (anchor <= focus ? [anchor, focus] : [focus, anchor]);
	const sameContent = (a: unknown[], b: unknown[]): boolean => JSON.stringify(a) === JSON.stringify(b);

	let editor: HTMLDivElement | undefined;
	// The expression as typed, which the spans in the editor only display, so the caret and every edit are mapped through it
	let text = value;
	let editing = false;
	let composing = false;
	// The selection a composition began from, which its undo step gives back, and whether it began in a DOM awaiting a redraw
	let composedFrom: TextRange | undefined = undefined;
	let composedOverStaleText = false;
	let undoStack: Snapshot[] = [];
	let redoStack: Snapshot[] = [];
	// Where the last typed word character ended, while a run of typing is one undo step
	let typingEnd: number | undefined = undefined;
	// The selection to place once the tokens for the last edit arrive
	let pending: TextRange | undefined = undefined;
	let rendered: { text: string; tokens: MathExpressionToken[]; tooltips: MathExpressionTooltip[]; errors: MathExpressionRange[] } | undefined = undefined;
	// The spans of the bracket pair beside the caret, which are highlighted
	let matchedSpans: Element[] = [];
	// The names offered to finish the one being typed, with the text they were offered for
	let completion: (MathExpressionCompletions & { text: string }) | undefined = undefined;
	let completionOpen = false;
	// The text awaiting the reply with its names
	let completionAsked: { text: string; caret: number } | undefined = undefined;
	let completionAnchor: Anchor = { left: 0, bottom: 0 };
	let completionMenu: MenuList | undefined;
	// Whether the documentation of the call or matrix literal around the caret is wanted, which typing in it, a comma, or Tab brings up
	let signatureOpen = false;
	let signature: { label: string; description: string; code: string; bounds: { left: number; top: number; bottom: number } } | undefined = undefined;
	// Whether the number popover is wanted for the literal at the caret, which Ctrl+Space or a double-click brings up
	let constantOpen = false;
	// The literal as written and the power of ten of its digit that's stepped
	let constant: { start: number; end: number; text: string; digit: Digit; afterOperand: boolean; anchor: Anchor } | undefined = undefined;
	let constantElement: HTMLDivElement | undefined;
	// While the number popover is in use, the text's focus leaving for its drag or field continues the edit, until a press lands elsewhere
	let adjusting = false;
	let pressing = false;
	// Between the start and end of a drag in the number popover, whose changes make one undo step
	let dragging = false;
	// The text's selection while the number popover has the focus, given back with it
	let heldSelection: TextRange | undefined = undefined;
	// The text as it was when the number popover opened on its literal, where that literal began, which reverting puts back, and whether that just happened
	let constantOpened: (Snapshot & { start: number }) | undefined = undefined;
	let reverted = false;
	// Math typed into the number popover's field, awaiting its value to write over the literal it began at
	let evaluating: { source: string; start: number; literal: string } | undefined = undefined;

	$: constantChanged = constantOpened !== undefined && constantOpened.text !== text;

	// A new value the backend sends while the field isn't being edited replaces the text, as in the other text inputs
	$: watchValue(value);

	// The text is drawn only once the backend's tokens for it are here: the widget's own for its value, or the reply to the last edit
	$: {
		// The widget's own tokens come first, since a reply for the same text may be one sent to another node's widget with this ID
		const analysis = $analyses.analyses.get(String(widgetId));
		const known = value === text ? tokens : analysis?.source === text ? analysis.tokens : undefined;
		const knownTooltips = value === text ? tooltips : analysis?.tooltips || [];
		// The errors belong to the value, so an edit leaves them pointing at the wrong characters
		const invalid = value === text ? errors : NO_ERRORS;
		// A composition's text is the browser's to draw until it ends
		if (editor && known && !composing && (rendered?.text !== text || rendered.tokens !== known || rendered.errors !== invalid)) {
			// The same tokens sent again for the drawn text, like the widget's own after a commit, leave nothing to redraw
			const redraw = rendered?.text !== text || !sameContent(rendered.tokens, known) || !sameContent(rendered.errors, invalid);
			if (redraw) {
				render(known, invalid, pending);
				pending = undefined;
			}
			rendered = { text, tokens: known, tooltips: knownTooltips, errors: invalid };
			if (redraw) {
				highlightBrackets();
				updateSignature();
				updateConstant();
			}
		}
	}

	// After the text is drawn, so the menu can line up under the name being typed
	$: offerCompletions($analyses.analyses.get(String(widgetId)));

	$: writeEvaluation($analyses.evaluations.get(String(widgetId)));

	onDestroy(() => analyses.forget(String(widgetId)));

	function watchValue(value: string) {
		if (!editing) text = value;
	}

	// Draws the text as one span per character, so each may carry its token's color, its math alphabet glyph, and its superscript or subscript placement
	function render(tokens: MathExpressionToken[], errors: MathExpressionRange[], selection?: TextRange) {
		if (!editor) return;
		// Redrawing would drop a focused field's caret, so it stays where it was unless an edit says where it goes
		const placed = document.activeElement === editor ? selection || currentSelection() : undefined;

		const fragment = document.createDocumentFragment();
		// The run of erroneous characters being gathered into one span, which draws a single unbroken squiggle under them all
		let invalidRun: HTMLSpanElement | undefined = undefined;
		let next = 0;
		let index = 0;
		while (index < text.length) {
			const character = String.fromCodePoint(text.codePointAt(index) || 0);
			while (next < tokens.length && tokens[next].end <= index) next += 1;
			const token = next < tokens.length && tokens[next].start <= index ? tokens[next] : undefined;

			const span = document.createElement("span");
			if (token) {
				span.className = token.role.toLowerCase();
				span.dataset.token = String(next);
				if (token.role === "Basis") span.classList.add(`basis-${character}`);
				if (token.role === "Bracket" && (character === "{" || character === "}")) span.classList.add(character === "{" ? "opening-brace" : "closing-brace");
				// Exponents nested deeper than three levels stay at the third level's size and height
				if (token.superscript > 0) span.classList.add(`superscript-${Math.min(token.superscript, 3)}`);
				if (typeof token.subscript === "number" && index - token.start >= token.subscript) span.classList.add("subscript");
			}
			const style = token?.role === "Matrix" ? "bold" : token?.role === "Variable" || token?.role === "Basis" ? "italic" : undefined;
			span.textContent = style ? mathGlyph(character, style) : token?.role === "Operator" ? operatorGlyph(character) : character;
			// A character typed as one of the glyphs the others are displayed with is shown as itself, so it reads back as itself
			if (span.textContent === character && fromMathGlyphs(character) !== character) span.dataset.verbatim = "";

			// A run stops at a line break, since one squiggle can't follow the text onto the next line
			const invalid = character !== "\n" && (token?.role === "Error" || errors.some(({ start, end }) => start <= index && index < end));
			if (invalid) {
				if (!invalidRun) {
					invalidRun = document.createElement("span");
					invalidRun.className = "invalid";
					fragment.append(invalidRun);
				}
				invalidRun.append(span);
			} else {
				invalidRun = undefined;
				fragment.append(span);
			}
			index += character.length;
		}
		// A line after a trailing newline has nothing on it to hold the caret without a break element
		if (text.endsWith("\n")) fragment.append(document.createElement("br"));

		// eslint-disable-next-line svelte/no-dom-manipulating
		editor.replaceChildren(fragment);
		if (placed) setSelection(placed);
	}

	// What hovering a drawn token shows, from the tooltips listed once each alongside the tokens
	function tooltipOf(token: MathExpressionToken | undefined): MathExpressionTooltip | undefined {
		return typeof token?.tooltip === "number" ? rendered?.tooltips[token.tooltip] : undefined;
	}

	// A character takes its token's tooltip once hovered, rather than every character carrying a copy of it through each redraw
	function onMouseOver(e: MouseEvent) {
		const span = e.target instanceof HTMLElement ? e.target : undefined;
		const tooltip = span && tooltipOf(rendered?.tokens[Number(span.dataset.token)]);
		if (!span || !tooltip || span.dataset.tooltipLabel !== undefined) return;

		span.dataset.tooltipLabel = tooltip.label;
		span.dataset.tooltipDescription = tooltip.description;
		if (tooltip.form) span.dataset.tooltipCode = tooltip.form;
	}

	// The delimiter beside a collapsed caret and its partner, by their indices among the tokens, the one before the caret taking precedence
	function bracketsAtCaret(): [number, number] | undefined {
		const selection = document.activeElement === editor ? currentSelection() : undefined;
		if (!selection || selection.anchor !== selection.focus || !rendered) return undefined;
		const tokens = rendered.tokens;

		const paired = (index: number) => index >= 0 && typeof tokens[index].partner === "number";
		const before = tokens.findIndex((token) => token.end === selection.focus);
		const after = tokens.findIndex((token) => token.start === selection.focus);
		const bracket = paired(before) ? before : paired(after) ? after : undefined;
		const partner = bracket === undefined ? undefined : tokens[bracket].partner;
		return bracket === undefined || typeof partner !== "number" ? undefined : [bracket, partner];
	}

	function highlightBrackets() {
		const root = editor;
		if (!root) return;
		matchedSpans.forEach((span) => span.classList.remove("matched"));

		matchedSpans = (bracketsAtCaret() || []).flatMap((index) => Array.from(root.querySelectorAll(`[data-token="${index}"]`)));
		matchedSpans.forEach((span) => span.classList.add("matched"));
	}

	// The innermost call or matrix literal around a collapsed caret, by the indices among the tokens of what's documented (the call's name or
	// the literal's opening bracket) and of the opener whose separators divide it, and which of its arguments or entries the caret is in
	function documentedGroupAtCaret(): { documented: number; opener: number; argument: number } | undefined {
		const tokens = rendered?.tokens;
		const selection = document.activeElement === editor ? currentSelection() : undefined;
		if (!tokens || !selection || selection.anchor !== selection.focus) return undefined;
		const caret = selection.focus;

		const documented = (index: number) => {
			const spelling = text.slice(tokens[index].start, tokens[index].end);
			if (spelling === "[") return tooltipOf(tokens[index]) ? index : undefined;
			const name = tokens[index - 1];
			return spelling === "(" && name?.role === "Function" && tooltipOf(name)?.signature ? index - 1 : undefined;
		};
		// A group runs from its opener to its closer, or to the end while its closer isn't typed yet
		const encloses = (index: number) => {
			const { end, partner } = tokens[index];
			if (end > caret) return false;
			return typeof partner === "number" ? partner > index && caret <= tokens[partner].start : true;
		};

		const opener = tokens
			.map((_, index) => index)
			.filter((index) => documented(index) !== undefined && encloses(index))
			.at(-1);
		const name = opener === undefined ? undefined : documented(opener);
		if (opener === undefined || name === undefined) return undefined;

		const argument = tokens.filter((token) => token.separates === opener && token.end <= caret).length;
		return { documented: name, opener, argument };
	}

	// Shows the documentation of the call or matrix literal around the caret while it's wanted, and stops wanting it once the caret leaves
	function updateSignature() {
		// The tokens of an edit's text are still on their way, and drawing them calls this again
		if (!rendered || rendered.text !== text) return;

		const group = signatureOpen ? documentedGroupAtCaret() : undefined;
		const documented = group && rendered.tokens[group.documented];
		const tooltip = tooltipOf(documented);
		if (!group || !documented || !tooltip) {
			signatureOpen = false;
			signature = undefined;
			return;
		}

		// The call or matrix literal ends at its closer, or at the caret while its closer isn't typed yet
		const closer = rendered.tokens[group.opener].partner;
		const end = typeof closer === "number" ? characterBox(rendered.tokens[closer].start) : caretBox(currentSelection().focus);
		signature = {
			label: tooltip.label,
			description: markedDescription(tooltip, group.argument),
			code: signatureCode(tooltip, group.argument),
			bounds: boundsAround(documented.start, end),
		};
	}

	// The parameter an argument fills, where a variadic signature's one parameter stands for every argument
	function markedParameter(signature: MathExpressionSignature, argument: number): number {
		const last = signature.parameters.length - 1;
		return signature.parameters[last] === "…" ? Math.min(argument, last) : argument;
	}

	// The tooltip's description with the argument or entry being typed marked, by its bulleted line in bold or an arrow drawn into the matrix at it
	function markedDescription(tooltip: MathExpressionTooltip, argument: number): string {
		if (!tooltip.signature) return tooltip.description;
		const lines = tooltip.description.split("\n");

		const described = tooltip.signature.lines[markedParameter(tooltip.signature, argument)];
		if (described !== undefined && described < lines.length) {
			lines[described] = lines[described].replace(/^• (.+)$/, "• **$1**");
			return lines.join("\n");
		}

		const pointer = tooltip.signature.pointers[argument];
		const drawing = lines.indexOf("```");
		const line = drawing + 1 + (pointer?.line || 0);
		if (!pointer || drawing === -1 || line >= lines.length) return tooltip.description;

		const characters = Array.from(lines[line]);
		if (pointer.column >= characters.length) return tooltip.description;
		characters[pointer.column] = pointer.arrow;
		lines[line] = characters.join("");
		return lines.join("\n");
	}

	// The tooltip's form, where a signature has the argument or entry being typed in bold
	function signatureCode(tooltip: MathExpressionTooltip, argument: number): string {
		if (!tooltip.signature) return tooltip.form || "";
		const { opening, parameters, separator, closing, range } = tooltip.signature;

		const marked = markedParameter(tooltip.signature, argument);
		const drawn = parameters.map((parameter, index) => (index === marked ? `**${parameter}**` : parameter));
		const result = range ? ` → ${range}` : "";
		return `${opening}${drawn.join(separator)}${closing}${result}`;
	}

	function closeSignature() {
		signatureOpen = false;
		signature = undefined;
	}

	function onSelectionChange() {
		highlightBrackets();
		updateSignature();
		updateConstant();
	}

	// Keeps the number popover on its literal: the one holding the caret or selection, or while the popover has the focus, the one it began at
	function updateConstant() {
		if (!rendered || rendered.text !== text) return;

		const selection = document.activeElement === editor ? currentSelection() : undefined;
		const holds = (start: number, end: number) => {
			if (!selection) return start === constant?.start;
			const [from, to] = ordered(selection);
			return start <= from && to <= end;
		};
		const token = constantOpen ? rendered.tokens.find(({ literal, end }) => literal && holds(literal.start, end)) : undefined;
		const written = token?.literal && text.slice(token.literal.start, token.end);
		const decimal = written && parseDecimal(written);
		if (!token?.literal || !written || !decimal) {
			closeConstant();
			return;
		}

		// The stepped digit stays through a revert and while the popover is on the same literal, unless its underline is dragged to another
		const { start, afterOperand } = token.literal;
		const digit = constant && (constant.start === start || reverted) ? constant.digit : defaultDigit(decimal);
		reverted = false;
		if (constant?.start !== start) constantOpened = { text, selection: currentSelection(), start };

		// The popover stays put while in use, where parenthesizing a negative number moves the literal in by one
		const anchor = adjusting && constant ? constant.anchor : anchorAt(start, characterBox(token.end - 1));
		constant = { start, end: token.end, text: written, digit, afterOperand, anchor };
	}

	// Double-clicking a number, or its sign, selects it whole and brings up its popover as Ctrl+Space does
	function onDoubleClick(e: MouseEvent) {
		const tokens = rendered?.text === text ? rendered.tokens : undefined;
		const index = e.target instanceof HTMLElement ? Number(e.target.dataset.token) : NaN;
		const clicked = tokens?.[index];
		if (!tokens || !clicked) return;

		const token = [clicked, tokens[index + 1]].find((token) => token?.literal && token.literal.start <= clicked.start);
		if (!token?.literal) return;

		setSelection({ anchor: token.literal.start, focus: token.end });
		constantOpen = true;
		updateConstant();
	}

	function closeConstant() {
		// Closing while its field has the focus hands the focus back to the text
		if (constantElement?.contains(document.activeElement)) editor?.focus();
		constantOpen = false;
		constant = undefined;
		constantOpened = undefined;
		reverted = false;
	}

	// Puts the text back as it was when the number popover opened, as an undo step of its own
	function revertConstant() {
		if (!constantOpened || constantOpened.text === text) return;

		pushUndo();
		typingEnd = undefined;
		text = constantOpened.text;
		reverted = true;

		// The popover stays on its literal where it began, wherever parenthesizing a negative number had moved it, as does the selection
		if (constant) constant = { ...constant, start: constantOpened.start };
		if (document.activeElement === editor) {
			request(constantOpened.selection);
		} else {
			heldSelection = constantOpened.selection;
			request();
		}
	}

	// Writes the number popover's new text over its literal, as an undo step of its own unless a drag already began one
	function adjustConstant(written: string) {
		if (!constant) return;

		// A negative number after an operand it multiplies is parenthesized, since its sign alone would subtract
		const parenthesized = written.startsWith("-") && constant.afterOperand;
		const replacement = parenthesized ? `(${written})` : written;
		if (replacement === text.slice(constant.start, constant.end)) return;

		if (!dragging) pushUndo();
		typingEnd = undefined;
		text = text.slice(0, constant.start) + replacement + text.slice(constant.end);

		// The selection stays put, with an end at the old number's start or end kept at the new one's
		const old = constant;
		const start = old.start + (parenthesized ? 1 : 0);
		const end = start + written.length;
		const moved = (index: number) => {
			if (index < old.start) return index;
			if (index > old.end) return index + replacement.length - (old.end - old.start);
			return index === old.end ? end : start + Math.min(index - old.start, written.length);
		};
		const kept = (selection: TextRange | undefined) => selection && { anchor: moved(selection.anchor), focus: moved(selection.focus) };

		constant = { ...old, start, end, text: written, afterOperand: old.afterOperand && !parenthesized };
		if (document.activeElement === editor) {
			request(kept(pending || currentSelection()));
		} else {
			heldSelection = kept(heldSelection);
			request();
		}

		// A drag shows each value live, all within the one history step its start opened
		if (dragging) dispatch("updateText", text);
	}

	// Math typed into the number popover's field is evaluated, for its value to take the literal's place
	function evaluateConstant(source: string) {
		if (!constant) return;

		evaluating = { source, start: constant.start, literal: constant.text };
		evaluate(source);
	}

	// Writes the typed math's value over the literal, if it has one and the popover is still on that literal as it was
	function writeEvaluation(evaluation: MathExpressionEvaluation | undefined) {
		const asked = evaluating;
		if (!asked || evaluation?.source !== asked.source) return;
		evaluating = undefined;

		if (evaluation.result !== undefined && constant?.start === asked.start && constant.text === asked.literal) adjustConstant(evaluation.result);
	}

	function startDrag() {
		pushUndo();
		dragging = true;
		dispatch("startHistoryTransaction");
	}

	function endDrag() {
		dragging = false;
		dispatch("commitHistoryTransaction");
	}

	// A press in the number popover lets the text's focus leave for it, until a press lands elsewhere
	function onWindowPointerDown(e: PointerEvent) {
		pressing = e.target instanceof Node && Boolean(constantElement?.contains(e.target));
		adjusting = pressing;
	}

	// After the number input's own release handling, which gives its field the focus where the press was a click rather than a drag
	function onWindowPointerUp() {
		if (!pressing) return;
		setTimeout(() => {
			pressing = false;
			if (document.activeElement === editor) adjusting = false;
		});
	}

	// Pressing in the popover leaves the focus where it is, except within its field while a value is typed there, where pressing elsewhere in
	// the popover ends the typing, whose focus then returns to the text
	function onConstantMouseDown(e: MouseEvent) {
		const focused = document.activeElement;
		if (e.target === focused) return;

		e.preventDefault();
		if (focused instanceof HTMLElement && constantElement?.contains(focused)) focused.blur();
	}

	// The popover's field giving up the focus returns it to the text, unless a press elsewhere took it
	function onConstantFocusOut(e: FocusEvent) {
		const to = e.relatedTarget;
		if (to === editor || (to instanceof Node && constantElement?.contains(to))) return;

		if (adjusting) setTimeout(returnFocus);
		else finishEditing();
	}

	// Puts the focus back in the text with the selection it had, placed now or by the redraw of a change still on its way
	function returnFocus() {
		if (!editor || document.activeElement === editor) return;

		editor.focus();
		const selection = heldSelection;
		heldSelection = undefined;
		if (!selection) return;

		placeSelection(selection);
	}

	// Places the selection in the DOM, or while an edit's redraw is on its way, leaves it for the redraw to place
	function placeSelection(selection: TextRange) {
		if (rendered?.text === text) setSelection(selection);
		else pending = selection;
	}

	// Moves the caret through the highlighted pair, or else across a separator beside it, returning whether it took the key. Forward, it
	// goes into the opener, then past each of the group's separators, then past the closer, and backward it mirrors that from the closer.
	function stepThroughBrackets(backward: boolean): boolean {
		// Until the last edit's tokens arrive, there are no delimiters to step through, though the key is kept from leaving the field
		if (rendered?.text !== text) return true;

		const tokens = rendered.tokens;
		const selection = document.activeElement === editor ? currentSelection() : undefined;
		if (!tokens || !selection || selection.anchor !== selection.focus) return false;
		const caret = selection.focus;

		// The next of a group's separators past the caret in the direction of travel, or else the group's end
		const onward = (openerIndex: number) => {
			const opener = tokens[openerIndex];
			const closer = typeof opener.partner === "number" ? tokens[opener.partner] : undefined;
			const separators = tokens.filter((token) => token.separates === openerIndex);
			if (backward) return separators.filter((token) => token.start < caret).at(-1)?.start || opener.start;
			return separators.find((token) => token.end > caret)?.end || closer?.end;
		};

		let target: number | undefined = undefined;
		const pair = bracketsAtCaret();
		if (pair) {
			const [opener, closer] = [tokens[Math.min(...pair)], tokens[Math.max(...pair)]];
			if (!backward) {
				if (caret === opener.start) target = opener.end;
				else if (caret === opener.end) target = onward(Math.min(...pair));
				else if (caret === closer.start) target = closer.end;
			} else {
				if (caret === closer.end) target = closer.start;
				else if (caret === closer.start) target = onward(Math.min(...pair));
				else if (caret === opener.end) target = opener.start;
			}
		}

		// Where the pair leaves the caret put, a separator ahead of it is crossed, or else one behind it leads on through its group
		if (target === undefined) {
			const separator = (edge: "start" | "end") => tokens.find((token) => typeof token.separates === "number" && token[edge] === caret);
			const ahead = separator(backward ? "end" : "start");
			const behind = separator(backward ? "start" : "end");
			if (ahead) target = backward ? ahead.start : ahead.end;
			else if (typeof behind?.separates === "number") target = onward(behind.separates);
		}
		if (target === undefined) return false;

		closeCompletions();
		setSelection({ anchor: target, focus: target });
		return true;
	}

	// Sends the text to the backend, holding the selection to place once its tokens come back, and asks for names to finish the
	// one before the caret where the edit continues typing it
	function request(selection?: TextRange, completing = false) {
		pending = selection;
		const caret = completing && selection && selection.anchor === selection.focus ? selection.focus : undefined;
		if (caret === undefined) closeCompletions();
		else completionAsked = { text, caret };
		analyze(text, caret, false);
	}

	// Asks for names to finish the one the caret is in or beside, whatever has been typed of it
	function suggest() {
		const { anchor, focus } = currentSelection();
		if (anchor !== focus) return;

		// A number is adjusted in its popover rather than completed
		constantOpen = true;
		updateConstant();
		if (constant) {
			closeCompletions();
			return;
		}
		constantOpen = false;

		completionAsked = { text, caret: focus };
		analyze(text, focus, true);
	}

	// Shows the names in the reply to the text that asked for them, if it's still the text being edited
	function offerCompletions(analysis: MathExpressionAnalysis | undefined) {
		const asked = completionAsked;
		if (!asked || !analysis || analysis.source !== asked.text || analysis.source !== text) return;
		completionAsked = undefined;

		const offered = analysis.completions;
		if (!offered || offered.entries.flat().length === 0) {
			closeCompletions();
			return;
		}

		completion = { ...offered, text };
		completionAnchor = anchorAt(offered.start, characterBox(Math.max(offered.start, offered.end - 1)));
		completionOpen = true;
		// The names take the documentation's place until a comma or Tab brings it back
		closeSignature();
	}

	function closeCompletions() {
		completionOpen = false;
		completionAsked = undefined;
	}

	// Replaces the name with the chosen one, putting the caret between a call's parentheses, or selecting a suffixed
	// form's underscore so the suffix can be typed over it or after it
	function acceptCompletion(entry: MenuListEntry) {
		const offered = completion;
		closeCompletions();
		if (offered?.text !== text) return;

		const call = entry.value.endsWith("()");
		const name = call ? entry.value.slice(0, -2) : entry.value;
		const [anchor, focus] = name.endsWith("_") ? [name.length - 1, name.length] : call ? [name.length + 1, name.length + 1] : [name.length, name.length];
		replace({ anchor: offered.start, focus: offered.end }, entry.value, (from) => ({ anchor: from + anchor, focus: from + focus }));
	}

	// Where a menu hangs from relative to the field: the left of the character it's for, and the bottom of the line where what it's for ends,
	// so none of that is covered
	function anchorAt(index: number, last: DOMRect = characterBox(index)): Anchor {
		const field = editor?.parentElement;
		if (!editor || !field) return { left: 0, bottom: 0 };

		// An empty field has no character to hang from, so its menus hang from where its text begins
		const box = characterBox(index);
		const left = box.height > 0 ? box.left : editor.getBoundingClientRect().left;
		return { left: Math.max(0, left - field.getBoundingClientRect().left), bottom: lineEdges(last).bottom };
	}

	// What a tooltip for a token stands below or above, relative to the field: from the token's start and the top of its line to the bottom of the
	// line where what it documents ends
	function boundsAround(start: number, last: DOMRect): { left: number; top: number; bottom: number } {
		const fieldLeft = editor?.parentElement?.getBoundingClientRect().left || 0;
		const first = characterBox(start);
		return { left: first.left - fieldLeft, top: lineEdges(first).top, bottom: lineEdges(last).bottom };
	}

	// The top and bottom relative to the field of the line holding a box, found by where its middle falls among the editor's evenly spaced lines,
	// reaching past the editor's padding to the field's edges
	function lineEdges(box: DOMRect): { top: number; bottom: number } {
		const field = editor?.parentElement;
		if (!editor || !field) return { top: 0, bottom: 0 };
		const fieldBounds = field.getBoundingClientRect();

		const style = getComputedStyle(editor);
		const lineHeight = parseFloat(style.lineHeight);
		const contentTop = editor.getBoundingClientRect().top + parseFloat(style.paddingTop);
		const line = Math.max(0, Math.floor(((box.top + box.bottom) / 2 - contentTop) / lineHeight));
		const top = contentTop + line * lineHeight - parseFloat(style.paddingTop);
		const bottom = contentTop + (line + 1) * lineHeight + parseFloat(style.paddingBottom);
		return { top: Math.max(top, fieldBounds.top) - fieldBounds.top, bottom: Math.min(bottom, fieldBounds.bottom) - fieldBounds.top };
	}

	// A character's own box, since where a line wraps, a bare caret before it could stand at the end of the line above
	function characterBox(index: number): DOMRect {
		const range = document.createRange();
		range.setStart(...domPosition(index));
		if (index < text.length) range.setEnd(...domPosition(index + String.fromCodePoint(text.codePointAt(index) || 0).length));

		const boxes = range.getClientRects();
		return boxes[boxes.length - 1] || range.getBoundingClientRect();
	}

	// The caret's box at an index, which after a line break stands at the start of the next line
	function caretBox(index: number): DOMRect {
		const range = document.createRange();
		range.setStart(...domPosition(index));
		return range.getBoundingClientRect();
	}

	// The text typed for the glyphs of a node of the editor's DOM, unless its span shows a character typed as such a glyph
	function typedText(node: Node, display: string): string {
		return node.parentElement?.dataset.verbatim !== undefined ? display : fromMathGlyphs(display);
	}

	// The index into the text of a point in the editor's DOM, counting the characters typed for the glyphs before it
	function textIndex(node: Node, offset: number): number {
		if (!editor) return 0;

		const before = document.createRange();
		before.setStart(editor, 0);
		before.setEnd(node, offset);

		let index = 0;
		const walker = document.createTreeWalker(editor, NodeFilter.SHOW_TEXT);
		for (let textNode = walker.nextNode(); textNode; textNode = walker.nextNode()) {
			if (!(textNode instanceof Text) || !before.intersectsNode(textNode)) continue;
			const end = textNode === before.endContainer ? before.endOffset : textNode.length;
			index += typedText(textNode, textNode.data.slice(0, end)).length;
		}
		return index;
	}

	// The point in the editor's DOM of an index into the text
	function domPosition(index: number): [Node, number] {
		if (!editor) return [document.body, 0];

		const walker = document.createTreeWalker(editor, NodeFilter.SHOW_TEXT);
		let remaining = index;
		let last: Text | undefined = undefined;
		for (let textNode = walker.nextNode(); textNode; textNode = walker.nextNode()) {
			if (!(textNode instanceof Text)) continue;
			const glyphs = Array.from(textNode.data);
			const length = typedText(textNode, textNode.data).length;
			if (remaining <= length) {
				// Counts the displayed code units up to the wanted character
				let display = 0;
				glyphs.every((glyph) => {
					if (remaining <= 0) return false;
					remaining -= typedText(textNode, glyph).length;
					display += glyph.length;
					return true;
				});
				return [textNode, display];
			}
			remaining -= length;
			last = textNode;
		}
		return last ? [last, last.length] : [editor, 0];
	}

	// The selection in the text, which while an edit's redraw is on its way is where the edit put it, since the DOM still shows the text before it
	function currentSelection(): TextRange {
		if (pending && rendered?.text !== text) return pending;

		const selection = window.getSelection();
		if (!editor || !selection?.anchorNode || !selection.focusNode || !editor.contains(selection.anchorNode)) {
			return { anchor: text.length, focus: text.length };
		}
		return { anchor: textIndex(selection.anchorNode, selection.anchorOffset), focus: textIndex(selection.focusNode, selection.focusOffset) };
	}

	function setSelection({ anchor, focus }: TextRange) {
		const selection = window.getSelection();
		if (!selection) return;

		const [anchorNode, anchorOffset] = domPosition(anchor);
		const [focusNode, focusOffset] = domPosition(focus);
		selection.setBaseAndExtent(anchorNode, anchorOffset, focusNode, focusOffset);
	}

	function pushUndo(selection = currentSelection()) {
		undoStack = [...undoStack.slice(-(UNDO_LIMIT - 1)), { text, selection }];
		redoStack = [];
	}

	function restore(from: Snapshot[], to: Snapshot[]) {
		const snapshot = from.at(-1);
		if (!snapshot) return;
		typingEnd = undefined;

		to.push({ text, selection: currentSelection() });
		from.pop();
		text = snapshot.text;
		request(snapshot.selection);
	}

	// Replaces the selected stretch of the text, given in either order, and puts the caret after the insertion
	function replace(selection: TextRange, insertion: string, after = (from: number): TextRange => ({ anchor: from + insertion.length, focus: from + insertion.length }), completing = false) {
		const [from, to] = ordered(selection);
		const replaced = text.slice(0, from) + insertion + text.slice(to);
		// An edit that changes nothing, like Backspace at the start or accepting the name already typed, only moves the caret and shows the docs
		if (replaced === text) {
			signatureOpen = true;
			placeSelection(after(from));
			updateSignature();
			return;
		}

		// Typing a word character right after another extends the same undo step, so an undo takes back a word rather than a letter
		const typed = from === to && WORD_CHARACTER.test(insertion);
		if (!(typed && from === typingEnd)) pushUndo();
		typingEnd = typed ? from + insertion.length : undefined;

		text = replaced;
		signatureOpen = true;
		request(after(from), completing);
	}

	// Reads the text back from the DOM after an edit the browser made itself, like a composed character
	function syncFromDom(selectionBefore?: TextRange) {
		if (!editor || rendered?.text !== text) return;

		let synced = "";
		const walker = document.createTreeWalker(editor, NodeFilter.SHOW_TEXT);
		for (let textNode = walker.nextNode(); textNode; textNode = walker.nextNode()) {
			if (textNode instanceof Text) synced += typedText(textNode, textNode.data);
		}
		if (synced === text) return;

		const selection = currentSelection();
		pushUndo(selectionBefore);
		text = synced;
		request(selection);
	}

	// Every edit is applied to the text rather than the DOM, so the spans never drift from it
	function onBeforeInput(e: InputEvent) {
		if (composing || e.isComposing) return;

		// The DOM's ranges are stale while a redraw is outstanding
		const target = rendered?.text === text ? e.getTargetRanges()[0] : undefined;
		const range = target ? { anchor: textIndex(target.startContainer, target.startOffset), focus: textIndex(target.endContainer, target.endOffset) } : currentSelection();

		if (e.inputType === "insertText" || e.inputType === "insertReplacementText" || e.inputType.startsWith("insertFrom")) {
			e.preventDefault();
			// Text displayed in the math glyphs, as dragged or pasted from such a field, is read back as the characters they stand for
			const insertion = fromMathGlyphs(e.data ?? e.dataTransfer?.getData("text/plain") ?? "");
			const [from, to] = ordered(range);
			// A bar wraps only a selection, since one typed alone may be an absolute value's closer or half of an Or
			const wrapsInBars = insertion === "|" && from !== to;
			const closer = e.inputType === "insertText" ? PAIRS[insertion] || (wrapsInBars ? "|" : undefined) : undefined;

			if (closer !== undefined) {
				// A typed opening bracket brings its closer, around the selection if there is one, keeping the caret between them
				const inner = text.slice(from, to);
				replace(range, insertion + inner + closer, (from) => ({ anchor: from + 1, focus: from + 1 + inner.length }));
			} else if (e.inputType === "insertText" && from === to && Object.values(PAIRS).includes(insertion) && text[from] === insertion) {
				// A typed closing bracket that's already ahead of the caret is stepped over rather than doubled
				closeCompletions();
				placeSelection({ anchor: from + 1, focus: from + 1 });
			} else {
				replace(range, insertion, undefined, e.inputType === "insertText");
			}
		} else if (e.inputType === "insertLineBreak") {
			e.preventDefault();
			replace(range, "\n");
		} else if (e.inputType === "insertParagraph") {
			e.preventDefault();
			commit();
		} else if (e.inputType.startsWith("delete")) {
			e.preventDefault();
			const deleted = target ? range : deletion(range, e.inputType.endsWith("Backward"));
			const [from, to] = ordered(deleted);

			// Backspacing an opening bracket with its closer right after it removes the pair
			const caret = currentSelection();
			const pair = e.inputType === "deleteContentBackward" && caret.anchor === caret.focus && to === from + 1 && PAIRS[text[from]] === text[to];
			replace(pair ? { anchor: from, focus: to + 1 } : deleted, "", undefined, completionOpen);
		} else if (e.inputType === "historyUndo") {
			e.preventDefault();
			restore(undoStack, redoStack);
		} else if (e.inputType === "historyRedo") {
			e.preventDefault();
			restore(redoStack, undoStack);
		} else if (e.inputType.startsWith("format")) {
			e.preventDefault();
		}
	}

	// Where the browser gives no target range, a collapsed caret deletes the one character beside it
	function deletion({ anchor, focus }: TextRange, backward: boolean): TextRange {
		if (anchor !== focus) return { anchor, focus };

		const character = backward ? Array.from(text.slice(0, anchor)).at(-1) || "" : String.fromCodePoint(text.codePointAt(anchor) || 0);
		return backward ? { anchor: anchor - character.length, focus } : { anchor, focus: anchor + (anchor < text.length ? character.length : 0) };
	}

	function onInput() {
		if (!composing) syncFromDom();
	}

	function onCompositionStart() {
		composing = true;
		composedFrom = currentSelection();
		composedOverStaleText = rendered?.text !== text;
	}

	function onCompositionEnd(e: CompositionEvent) {
		composing = false;

		// Reading back a DOM that didn't yet show the last edit would lose that edit, so the composed text is applied to the text instead
		if (composedOverStaleText && composedFrom) replace(composedFrom, fromMathGlyphs(e.data));
		else syncFromDom(composedFrom);

		composedFrom = undefined;
		composedOverStaleText = false;
	}

	function onKeyDown(e: KeyboardEvent) {
		// A key that an input method is composing with is its own, like the Enter that confirms a candidate
		if (composing || e.isComposing) return;

		const accelKey = operatingSystem() === "Mac" ? e.metaKey : e.ctrlKey;
		const key = e.key.toLowerCase();

		if (completionOpen && completionMenu) {
			// The menu's own listener would also read each key, so it gets only those passed here, while the clipboard's go on to the editor
			if (!(accelKey && ["x", "c", "v"].includes(key))) e.stopPropagation();

			if (e.key === "ArrowUp" || e.key === "ArrowDown") {
				completionMenu.keydown(e);
				return;
			}
			// Enter or Tab picks the highlighted name
			if ((e.key === "Enter" || e.key === "Tab") && !e.shiftKey && completionMenu.pickHighlighted()) {
				e.preventDefault();
				return;
			}
			if (e.key === "Escape") {
				closeCompletions();
				return;
			}
			if (["ArrowLeft", "ArrowRight", "Home", "End", "PageUp", "PageDown"].includes(e.key)) closeCompletions();
		}

		// Ctrl+Space asks for completions, as does Option+Escape on Mac, its native completion key
		const asksForCompletions = (e.ctrlKey && e.key === " ") || (operatingSystem() === "Mac" && e.altKey && e.key === "Escape");
		if (asksForCompletions) {
			e.preventDefault();
			// The editor's shortcuts hear Escape even from a text field
			e.stopPropagation();
			suggest();
		} else if (e.key === "Tab") {
			// Tab and Shift+Tab step through the delimiters beside the caret, and only move the focus on where there are none to step through
			if (stepThroughBrackets(e.shiftKey)) {
				e.preventDefault();
				signatureOpen = true;
			}
		} else if (e.key === "Escape" && (constant || dragging)) {
			// A parent floating menu mustn't also close, and during a drag, Escape is the number input's to abort it
			e.stopPropagation();
			if (!dragging) closeConstant();
		} else if (e.key === "Escape" && signature) {
			// A parent floating menu mustn't also close
			e.stopPropagation();
			closeSignature();
		} else if (e.key === "Enter") {
			e.preventDefault();
			if (e.shiftKey) replace(currentSelection(), "\n");
			else commit();
		} else if (e.key === "Escape") {
			cancel();
		} else if (accelKey && key === "z") {
			// The browser has no history of its own to offer for undo, since every edit was applied to the text instead of the DOM
			e.preventDefault();
			if (e.shiftKey) restore(redoStack, undoStack);
			else restore(undoStack, redoStack);
		} else if (accelKey && key === "y") {
			e.preventDefault();
			restore(redoStack, undoStack);
		}
	}

	// The clipboard gets the typed text rather than the displayed glyphs, returning whether there was any to give it
	function onCopy(e: ClipboardEvent): boolean {
		const [from, to] = ordered(currentSelection());
		if (from === to) return false;

		e.clipboardData?.setData("text/plain", text.slice(from, to));
		e.preventDefault();
		return true;
	}

	function onCut(e: ClipboardEvent) {
		if (onCopy(e)) replace(currentSelection(), "");
	}

	// Dragged text carries the typed characters rather than the displayed glyphs, and is copied, since the drop that would move it lands
	// after the field has been redrawn
	function onDragStart(e: DragEvent) {
		const [from, to] = ordered(currentSelection());
		if (!e.dataTransfer || from === to) return;

		e.dataTransfer.clearData();
		e.dataTransfer.setData("text/plain", text.slice(from, to));
		e.dataTransfer.effectAllowed = "copy";
	}

	function onFocus() {
		// The focus coming back from the number popover continues the same edit
		if (!pressing) adjusting = false;
		if (!editing) {
			editing = true;
			undoStack = [];
			redoStack = [];
			typingEnd = undefined;
		}

		// Focus can return the caret to where it was without changing the selection, so no selection change would redraw the highlight
		highlightBrackets();
	}

	function onBlur(e: FocusEvent) {
		highlightBrackets();

		// The focus given to the number popover's field or taken by its drag stays within the edit, holding the selection until the focus returns
		if (adjusting) {
			heldSelection = pending || currentSelection();
			pending = undefined;
			if (!(e.relatedTarget instanceof Node && constantElement?.contains(e.relatedTarget))) setTimeout(returnFocus);
			return;
		}

		finishEditing();
	}

	function finishEditing() {
		closeCompletions();
		closeSignature();
		closeConstant();
		pending = undefined;
		if (!editing) return;
		editing = false;

		if (text !== value) dispatch("commitText", text);
	}

	function commit() {
		editor?.blur();
	}

	function cancel() {
		editing = false;
		pending = undefined;
		text = value;
		editor?.blur();
		if (editor) preventEscapeClosingParentFloatingMenu(editor);
	}

	export function focus() {
		editor?.focus();
	}
</script>

<LayoutRow class="math-expression-input" classes={{ disabled }} {tooltipLabel} {tooltipDescription} {tooltipShortcut}>
	<div
		class="editor"
		role="textbox"
		tabindex={disabled ? undefined : 0}
		contenteditable={!disabled}
		spellcheck="false"
		autocapitalize="off"
		bind:this={editor}
		on:beforeinput={onBeforeInput}
		on:input={onInput}
		on:compositionstart={onCompositionStart}
		on:compositionend={onCompositionEnd}
		on:keydown={onKeyDown}
		on:dblclick={onDoubleClick}
		on:mouseover={onMouseOver}
		on:copy={onCopy}
		on:cut={onCut}
		on:dragstart={onDragStart}
		on:focus={onFocus}
		on:blur={onBlur}
	></div>
	<!-- Pressing on the menu leaves the focus in the text, so clicking a name picks it rather than committing the text -->
	<div class="completions" style:left={`${completionAnchor.left}px`} style:top={`${completionAnchor.bottom}px`} role="presentation" on:mousedown|preventDefault>
		<MenuList
			on:open={({ detail }) => (completionOpen = detail)}
			on:activeEntry={({ detail }) => acceptCompletion(detail)}
			open={completionOpen}
			activeEntry={completion?.entries.flat()[completion.best]}
			highlightFollowsActiveEntry={true}
			entries={completion?.entries || []}
			entriesHash={completion?.entriesHash || 0n}
			direction="Bottom"
			monospace={true}
			scrollableY={true}
			bind:this={completionMenu}
		/>
	</div>
	<!-- Opening afresh wherever it's anchored, since a menu positions itself only when it opens or resizes -->
	{#if constant && !completionOpen}
		{#key `${constant.anchor.left} ${constant.anchor.bottom}`}
			<div
				class="constant"
				style:left={`${constant.anchor.left}px`}
				style:top={`${constant.anchor.bottom}px`}
				role="presentation"
				bind:this={constantElement}
				on:mousedown={onConstantMouseDown}
				on:focusout={onConstantFocusOut}
			>
				<FloatingMenu open={true} type="Popover" tail={false} alignment="Start" direction="Bottom" windowEdgeMargin={0} escapeCloses={false} strayCloses={false}>
					<LayoutRow>
						<!-- Wide enough for its digits, which the stepped digit's underline needs to line up with -->
						<NumberInput
							exact={constant.text}
							digit={constant.digit}
							minWidth={Math.max(120, constant.text.length * 8 + 64)}
							on:exactValue={({ detail }) => adjustConstant(detail)}
							on:commitText={({ detail }) => evaluateConstant(detail)}
							on:digit={({ detail }) => constant && (constant = { ...constant, digit: detail })}
							on:startHistoryTransaction={startDrag}
							on:commitHistoryTransaction={endDrag}
						/>
						<Separator style="Related" />
						<IconButton icon="HistoryUndo" size={24} disabled={!constantChanged} action={revertConstant} tooltipLabel="Revert" tooltipDescription="Restore the initial value." />
						<IconButton icon={constantChanged ? "Checkmark" : "CloseX"} size={24} action={closeConstant} tooltipLabel={constantChanged ? "Confirm" : "Close"} />
					</LayoutRow>
				</FloatingMenu>
			</div>
		{/key}
	{/if}
	{#if signature && !completionOpen && !constant}
		{#key JSON.stringify(signature.bounds)}
			<Tooltip given={signature} />
		{/key}
	{/if}
</LayoutRow>

<svelte:document on:selectionchange={onSelectionChange} />
<svelte:window on:pointerdown|capture={onWindowPointerDown} on:pointerup|capture={onWindowPointerUp} />

<style lang="scss">
	.math-expression-input {
		flex: 1 1 auto;
		min-width: 80px;
		position: relative;
		border-radius: 2px;
		background: var(--color-1-nearblack);
		--math-number: #eeeeee;
		--math-matrix: #a095ff;
		--math-basis-i: #ff6363;
		--math-basis-j: #66c14a;
		--math-basis-k: #76b6ff;
		--math-variable: #fff9a8;
		--math-function: #c9c2ff;
		--math-operator: #ffffff;
		--math-bracket: #f2b532;
		--math-error: #ff0000;
		--math-matched-bracket: #eeeeee;

		.editor {
			flex: 1 1 100%;
			min-width: 30px;
			min-height: calc(var(--widget-height) - 6px);
			line-height: calc(var(--widget-height) - 6px);
			margin: 0 8px;
			// The math font's glyphs sit a pixel higher in the line box than Source Sans Pro's, so the padding shifts them down to line up with the other fields
			padding: 4px 0 2px 0;
			outline: none;
			font-family: "STIX Two Math", "Source Sans Pro", serif;
			font-size: 16px;
			color: var(--color-e-nearwhite);
			caret-color: var(--color-e-nearwhite);
			white-space: pre-wrap;
			overflow-wrap: anywhere;
			unicode-bidi: plaintext;

			.number {
				color: var(--math-number);
			}

			.variable {
				color: var(--math-variable);
			}

			.matrix {
				color: var(--math-matrix);
			}

			.basis-i {
				color: var(--math-basis-i);
			}

			.basis-j {
				color: var(--math-basis-j);
			}

			.basis-k {
				color: var(--math-basis-k);
			}

			.function {
				color: var(--math-function);
			}

			// A keyword is an operator spelled as a word
			.operator,
			.keyword {
				color: var(--math-operator);
			}

			.keyword {
				font-size: 0.8em;
			}

			.bracket {
				color: var(--math-bracket);
			}

			// A piecewise's braces are set apart from its cases without spaces in the text, the opener's by letter spacing so a caret after it lands past the gap
			.opening-brace {
				letter-spacing: 0.2em;

				&.matched::after {
					right: calc(0.2em - 1px);
				}
			}

			.closing-brace {
				margin-left: 0.2em;
			}

			.matrixliteral {
				color: var(--math-matrix);
			}

			.matched {
				position: relative;
				z-index: 0;

				&::after {
					content: "";
					position: absolute;
					z-index: -1;
					inset: -1px;
					background: rgb(from var(--math-matched-bracket) r g b / 0.33);
				}
			}

			.error {
				color: var(--math-error);
			}

			// A wavy text decoration would restart its wave at every character's span, so the squiggle is a masked strip under the run
			.invalid {
				position: relative;

				&::after {
					content: "";
					position: absolute;
					left: 0;
					right: 0;
					bottom: -2px;
					height: 4px;
					background: var(--math-error);
					mask-image: url('data:image/svg+xml;utf8,\
						<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 6 4" width="6px" height="4px"><path d="M-3 2Q-1.5 4 0 2T3 2T6 2T9 2" fill="none" stroke="black" /></svg>\
						');
					mask-repeat: repeat-x;
					pointer-events: none;
				}
			}

			.superscript-1 {
				--exponent-level: 1;
			}

			.superscript-2 {
				--exponent-level: 2;
			}

			.superscript-3 {
				--exponent-level: 3;
			}

			.subscript {
				--subscript: 1;
			}

			// Each exponent level is 20% smaller than its parent and raised a fifth of its own size, a subscript 20% smaller again
			// and lowered, so each sibling span gets the compounded size and the summed offset for its place
			.superscript-1,
			.superscript-2,
			.superscript-3,
			.subscript {
				position: relative;
				font-size: calc(pow(0.8, var(--exponent-level, 0) + var(--subscript, 0)) * 1em);
				top: calc((-0.8em * (pow(1.25, var(--exponent-level, 0)) - 1)) / pow(0.8, var(--subscript, 0)) + 0.25em * var(--subscript, 0));
			}
		}

		// Points at the start of the name being typed or the number being adjusted, on the bottom of its line, below which their menus open
		.completions,
		.constant {
			position: absolute;
			width: 0;
			height: 0;
		}

		.completions .menu-list .floating-menu-container .floating-menu-content {
			max-height: 240px;
		}

		&.disabled {
			background: var(--color-2-mildblack);

			.editor {
				color: var(--color-8-uppergray);
				pointer-events: none;
			}
		}
	}
</style>
