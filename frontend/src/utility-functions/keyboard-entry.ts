export function makeKeyboardModifiersBitfield(e: WheelEvent | PointerEvent | MouseEvent | KeyboardEvent): number {
	return (
		// Shift (all platforms)
		(Number(e.shiftKey) << 0) |
		// Alt (all platforms, also called Option on Mac)
		(Number(e.altKey) << 1) |
		// Control (all platforms)
		(Number(e.ctrlKey) << 2) |
		// Meta (Windows/Linux) or Command (Mac)
		(Number(e.metaKey) << 3)
	);
}

// Necessary because innerText puts an extra newline character at the end when the text is more than one line.
export function textInputCleanup(text: string): string {
	if (text[text.length - 1] === "\n") return text.slice(0, -1);
	return text;
}

// This function tries to find what scan code the user pressed, even if using a non-US keyboard.
// Directly using `KeyboardEvent.code` scan code only works on a US QWERTY layout, because alternate layouts like
// QWERTZ (German) or AZERTY (French) will end up reporting the wrong keys.
// Directly using `KeyboardEvent.key` doesn't work because the results are often garbage, as the printed character
// varies when the Shift key is pressed, or worse, when the Option (Alt) key on a Mac is pressed.
// This function does its best to try and sort through both of those sources of information to determine the localized scan code.
//
// This function is an imperfect stopgap solution to allow non-US keyboards to be handled on a best-effort basis.
// Eventually we will need a more robust system based on a giant database of keyboard layouts from all around the world.
// We'd provide the user a choice of layout, and aim to detect a default based on the `key` and `code` values entered by the user
// combined with `Keyboard.getLayoutMap()` where supported in Chromium-based browsers and perhaps the browser's language and IP address.
// We are also limited by browser APIs, since the spec doesn't support what we need it to:
// <https://github.com/WICG/keyboard-map/issues/26>
// In the desktop version of VS Code, this is achieved with this Electron plugin:
// <https://github.com/Microsoft/node-native-keymap>
// We may be able to port that (it's a relatively small codebase) to Rust for use with Tauri.
// But on the web, just like VS Code, we're limited by the shortcomings of the spec.
// A collection of further insights:
// <https://docs.google.com/document/d/1p17IBbYGsZivLIMhKZOaCJFAJFokbPfKrkB37fOPXSM/edit>
// And it's a really good idea to read the explainer on keyboard layout variations and the whole spec (it's quite digestible):
// <https://www.w3.org/TR/uievents-code/#key-alphanumeric-writing-system>
export async function getLocalizedScanCode(e: KeyboardEvent): Promise<string> {
	const keyText = e.key;
	const scanCode = e.code;

	// Use the key code directly if it isn't one that changes per locale (i.e. isn't a writing system key or one of the other few exceptions)
	const scanCodeNotLocaleSpecific = !LOCALE_SPECIFIC_KEY_CODES.includes(scanCode);
	if (scanCodeNotLocaleSpecific) {
		return scanCode;
	}

	// Use the key directly if it's one of the exceptions that usually don't change, but sometimes do in a predictable way
	if (SCAN_CODES_FOR_NON_WRITING_KEYS_THAT_VARY_PER_LOCALE.includes(scanCode)) {
		// Numpad comma and period which swap in some locales as decimal and thousands separator symbols
		if (NUMPAD_DECIMAL_AND_THOUSANDS_SEPARATORS.includes(scanCode)) {
			switch (scanCode) {
				case ".":
					return "NumpadDecimal";
				case ",":
					return "NumpadComma";
				default:
					return scanCode;
			}
		}

		// The AltRight key changes from a key value of "Alt" to "AltGraph" on keyboards with an AltGraph key
		if (scanCode === "AltRight") {
			return keyText === "Alt" ? "AltRight" : "AltGraph";
		}
	}

	// Use good-enough-for-now heuristics on the writing system keys, which are commonly subject to change by locale

	// Number scan codes
	if (/^Digit[0-9]$/.test(scanCode)) {
		// For now it's good enough to treat every digit key, regardless of locale, as just its digit from the standard US layout.
		// Even on a keyboard like the French AZERTY layout, where numbers are shifted, users still refer to those keys by their numbers.
		// This unfortunately means that any special symbols under these keys are overridden by their number, making it impossible to access some shortcuts that rely on those special symbols.
		// We'll have to deal with that for now, and find a way to upgrade or properly replace this system, or assign alternate keymaps based on locale, when people complain.
		return scanCode;
	}

	// Letter scan codes
	if (/^Key([A-Z])$/.test(scanCode)) {
		// Get the uppercase letter, with any accents or discritics removed if possible
		const rawLetter = keyText
			.normalize("NFD")
			.replace(/\p{Diacritic}/gu, "")
			.toUpperCase();

		// If the key letter is in the A-Z range, use the key code for that letter
		if (/^[A-Z]$/.test(rawLetter)) return `Key${rawLetter}`;

		// If the key text isn't one of the named attribute values, that means it must be the literal unicode value which we use directly
		// It is likely a weird symbol that isn't in the A-Z range even with accents removed.
		// It might be a symbol from an Option key combination on a Mac. Or it might be from a non-Latin alphabet like Cyrillic.
		if (!KEY_ATTRIBUTE_VALUES.has(keyText)) {
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			if (navigator && "keyboard" in navigator && "getLayoutMap" in (navigator as any).keyboard) {
				// eslint-disable-next-line @typescript-eslint/no-explicit-any
				const layout = await (navigator as any).keyboard.getLayoutMap();

				type KeyCode = string;
				type KeySymbol = string;
				// Get all the keyboard mappings and transform the key symbols to uppercase
				const keyboardLayoutMap: [KeyCode, KeySymbol][] = [...layout.entries()].map(([keyCode, keySymbol]) => [keyCode, keySymbol.toUpperCase()]);

				// If we match the uppercase version of the pressed key character, use the scan code that produces it
				const matchedEntry = keyboardLayoutMap.find(([_, keySymbol]) => keySymbol === keyText.toUpperCase());
				if (matchedEntry) return matchedEntry[0];
			}

			// If the keyboard layout API is unavailable, or it didn't match anything, just return the scan code that the user typed
			// This isn't perfect because alternate keyboard layouts may end up having the US QWERTY key,
			// but it's all we can do without a giant database of keyboard layouts and Mac Option key combinations
			return scanCode;
		}

		// If the key's named attribute value shares a name with a scan code, use that scan code
		if (KEY_CODE_NAMES.includes(keyText)) {
			return scanCodeFromKeyText(keyText);
		}
		if (KEY_ATTRIBUTE_VALUES_INVOLVING_HANDEDNESS.includes(keyText)) {
			// Since for some reason we're in a situation where we are using the key instead of the scan code to
			// match one of the modifier keys which have both a Left and Right variant as part of the scan code names,
			// but no handedness as part of the key's named attribute values, we default to the left side as it's more common.
			return `${keyText}Left`;
		}

		// All reasonable attempts to figure out what this key is has now failed, so we fall back on the US QWERTY layout scan code
		return scanCode;
	}

	// If the key text is the unicode character for one of the standard symbols on the US keyboard, use the symbol even though it's not located on the same scan code as on a US keyboard
	if (WRITING_SYSTEM_SPECIAL_CHARS.includes(keyText)) {
		return scanCodeFromKeyText(keyText);
	}

	// If the key is otherwise totally unrecognized, we ignore it
	if (keyText === "Unidentified" || scanCode === "Unidentified") return "Unidentified";

	// As a last resort, we just use the scan code
	return scanCode;
}

function scanCodeFromKeyText(keyText: string): string {
	// There are many possible unicode symbols as well as named attribute values, but we only care about finding the equivalent scan code (based on the US keyboard) without regard for modifiers

	// Match any handed modifier keys by claiming it's the left-handed modifier
	if (KEY_ATTRIBUTE_VALUES_INVOLVING_HANDEDNESS.includes(keyText)) {
		return `${keyText}Left`;
	}

	// Match any named attribute keys to an identical key name
	const identicalName = KEY_CODE_NAMES.find((code) => code === keyText);
	if (identicalName) return keyText;

	// Match the space character
	// Order matters because the next step assumes it can safely ignore the space character
	if (keyText === " ") return "Space";

	// Match individual characters by the scan code which produces that symbol on a US keyboard, either shifted or not
	// This also includes the `Digit*` and `Key*` codes
	const matchedScanCode = KEY_CODES.find((info) => info.keys?.us?.includes(keyText));
	if (matchedScanCode) return matchedScanCode.code;

	return "Unidentified";
}

type KeyCategories = "writing-system" | "functional" | "functional-jp-kr" | "control-pad" | "arrow-pad" | "numpad" | "function" | "media" | "unidentified";
type KeyboardLocale = "us";
type ScanCodeInfo = { code: string; category: KeyCategories; keys?: Record<KeyboardLocale, string | undefined> };
const KEY_CODES: ScanCodeInfo[] = [
	// Writing system keys
	// Codes produce different printed characters depending on locale
	// https://www.w3.org/TR/uievents-code/#key-alphanumeric-writing-system

	{ code: "Digit0", category: "writing-system", keys: { us: "0 )" } },
	{ code: "Digit1", category: "writing-system", keys: { us: "1 !" } },
	{ code: "Digit2", category: "writing-system", keys: { us: "2 @" } },
	{ code: "Digit3", category: "writing-system", keys: { us: "3 #" } },
	{ code: "Digit4", category: "writing-system", keys: { us: "4 $" } },
	{ code: "Digit5", category: "writing-system", keys: { us: "5 %" } },
	{ code: "Digit6", category: "writing-system", keys: { us: "6 ^" } },
	{ code: "Digit7", category: "writing-system", keys: { us: "7 &" } },
	{ code: "Digit8", category: "writing-system", keys: { us: "8 *" } },
	{ code: "Digit9", category: "writing-system", keys: { us: "9 (" } },

	{ code: "KeyA", category: "writing-system", keys: { us: "a A" } },
	{ code: "KeyB", category: "writing-system", keys: { us: "b B" } },
	{ code: "KeyC", category: "writing-system", keys: { us: "c C" } },
	{ code: "KeyD", category: "writing-system", keys: { us: "d D" } },
	{ code: "KeyE", category: "writing-system", keys: { us: "e E" } },
	{ code: "KeyF", category: "writing-system", keys: { us: "f F" } },
	{ code: "KeyG", category: "writing-system", keys: { us: "g G" } },
	{ code: "KeyH", category: "writing-system", keys: { us: "h H" } },
	{ code: "KeyI", category: "writing-system", keys: { us: "i I" } },
	{ code: "KeyJ", category: "writing-system", keys: { us: "j J" } },
	{ code: "KeyK", category: "writing-system", keys: { us: "k K" } },
	{ code: "KeyL", category: "writing-system", keys: { us: "l L" } },
	{ code: "KeyM", category: "writing-system", keys: { us: "m M" } },
	{ code: "KeyN", category: "writing-system", keys: { us: "n N" } },
	{ code: "KeyO", category: "writing-system", keys: { us: "o O" } },
	{ code: "KeyP", category: "writing-system", keys: { us: "p P" } },
	{ code: "KeyQ", category: "writing-system", keys: { us: "q Q" } },
	{ code: "KeyR", category: "writing-system", keys: { us: "r R" } },
	{ code: "KeyS", category: "writing-system", keys: { us: "s S" } },
	{ code: "KeyT", category: "writing-system", keys: { us: "t T" } },
	{ code: "KeyU", category: "writing-system", keys: { us: "u U" } },
	{ code: "KeyV", category: "writing-system", keys: { us: "v V" } },
	{ code: "KeyW", category: "writing-system", keys: { us: "w W" } },
	{ code: "KeyX", category: "writing-system", keys: { us: "x X" } },
	{ code: "KeyY", category: "writing-system", keys: { us: "y Y" } },
	{ code: "KeyZ", category: "writing-system", keys: { us: "z Z" } },

	{ code: "Backquote", category: "writing-system", keys: { us: "` ~" } },
	{ code: "Backslash", category: "writing-system", keys: { us: "\\ |" } },
	{ code: "BracketLeft", category: "writing-system", keys: { us: "[ {" } },
	{ code: "BracketRight", category: "writing-system", keys: { us: "] }" } },
	{ code: "Comma", category: "writing-system", keys: { us: ", <" } },
	{ code: "Equal", category: "writing-system", keys: { us: "= +" } },
	{ code: "Minus", category: "writing-system", keys: { us: "- _" } },
	{ code: "Period", category: "writing-system", keys: { us: ". >" } },
	{ code: "Quote", category: "writing-system", keys: { us: "' \"" } },
	{ code: "Semicolon", category: "writing-system", keys: { us: "; :" } },
	{ code: "Slash", category: "writing-system", keys: { us: "/ ?" } },

	{ code: "IntlBackslash", category: "writing-system", keys: { us: undefined } },
	{ code: "IntlRo", category: "writing-system", keys: { us: undefined } },
	{ code: "IntlYen", category: "writing-system", keys: { us: undefined } },

	// Functional keys
	// https://www.w3.org/TR/uievents-code/#key-alphanumeric-functional
	// Codes have the same meaning regardless of locale, except for "AltRight"
	{ code: "AltLeft", category: "functional" },
	{ code: "AltRight", category: "functional" }, // Exception: `key` value is either "Alt" or "AltGraph" depending on locale (e.g. US vs. French, respectively)
	// The W3C table includes this in the Writing System Keys table instead of the Functional Keys table, but its diagrams
	// and text describe it as a functional key, so it has been moved here under the assumption that the table is incorrect
	// https://github.com/w3c/uievents-code/issues/34
	{ code: "Backspace", category: "writing-system" }, // Shares a name with a key attribute
	{ code: "CapsLock", category: "functional" }, // Shares a name with a key attribute
	{ code: "ContextMenu", category: "functional" }, // Shares a name with a key attribute
	{ code: "ControlLeft", category: "functional" }, // Shares a name with a key attribute as "Control"
	{ code: "ControlRight", category: "functional" }, // Shares a name with a key attribute as "Control"
	{ code: "Enter", category: "functional" }, // Shares a name with a key attribute
	{ code: "MetaLeft", category: "functional" }, // Shares a name with a key attribute as "Meta"
	{ code: "MetaRight", category: "functional" }, // Shares a name with a key attribute as "Meta"
	{ code: "ShiftLeft", category: "functional" }, // Shares a name with a key attribute as "Shift"
	{ code: "ShiftRight", category: "functional" }, // Shares a name with a key attribute as "Shift"
	{ code: "Space", category: "functional" },
	{ code: "Tab", category: "functional" }, // Shares a name with a key attribute

	// Functional Japanese/Korean keys
	{ code: "Convert", category: "functional-jp-kr" }, // Shares a name with a key attribute
	{ code: "KanaMode", category: "functional-jp-kr" }, // Shares a name with a key attribute
	{ code: "Lang1", category: "functional-jp-kr" },
	{ code: "Lang2", category: "functional-jp-kr" },
	{ code: "Lang3", category: "functional-jp-kr" },
	{ code: "Lang4", category: "functional-jp-kr" },
	{ code: "Lang5", category: "functional-jp-kr" },
	{ code: "NonConvert", category: "functional-jp-kr" }, // Shares a name with a key attribute

	// Control pad keys
	{ code: "Delete", category: "control-pad" }, // Shares a name with a key attribute
	{ code: "End", category: "control-pad" }, // Shares a name with a key attribute
	{ code: "Help", category: "control-pad" }, // Shares a name with a key attribute
	{ code: "Home", category: "control-pad" }, // Shares a name with a key attribute
	{ code: "Insert", category: "control-pad" }, // Shares a name with a key attribute
	{ code: "PageDown", category: "control-pad" }, // Shares a name with a key attribute
	{ code: "PageUp", category: "control-pad" }, // Shares a name with a key attribute

	// Arrow pad keys
	{ code: "ArrowDown", category: "arrow-pad" }, // Shares a name with a key attribute
	{ code: "ArrowLeft", category: "arrow-pad" }, // Shares a name with a key attribute
	{ code: "ArrowRight", category: "arrow-pad" }, // Shares a name with a key attribute
	{ code: "ArrowUp", category: "arrow-pad" }, // Shares a name with a key attribute

	// Numpad keys
	{ code: "Numpad0", category: "numpad" },
	{ code: "Numpad1", category: "numpad" },
	{ code: "Numpad2", category: "numpad" },
	{ code: "Numpad3", category: "numpad" },
	{ code: "Numpad4", category: "numpad" },
	{ code: "Numpad5", category: "numpad" },
	{ code: "Numpad6", category: "numpad" },
	{ code: "Numpad7", category: "numpad" },
	{ code: "Numpad8", category: "numpad" },
	{ code: "Numpad9", category: "numpad" },
	{ code: "NumLock", category: "numpad" }, // Shares a name with a key attribute
	{ code: "NumpadAdd", category: "numpad" },
	{ code: "NumpadBackspace", category: "numpad" },
	{ code: "NumpadClear", category: "numpad" },
	{ code: "NumpadClearEntry", category: "numpad" },
	{ code: "NumpadComma", category: "numpad" }, // Exception: Produces either a comma (,) or period (.) depending on locale (e.g. comma in US vs. period in Brazil)
	{ code: "NumpadDecimal", category: "numpad" }, // Exception: Produces either a comma (,) or period (.) depending on locale (e.g. period in US vs. decimal in Brazil)
	{ code: "NumpadDivide", category: "numpad" },
	{ code: "NumpadEnter", category: "numpad" },
	{ code: "NumpadEqual", category: "numpad" },
	{ code: "NumpadHash", category: "numpad" },
	{ code: "NumpadMemoryAdd", category: "numpad" },
	{ code: "NumpadMemoryClear", category: "numpad" },
	{ code: "NumpadMemoryRecall", category: "numpad" },
	{ code: "NumpadMemoryStore", category: "numpad" },
	{ code: "NumpadMemorySubtract", category: "numpad" },
	{ code: "NumpadMultiply", category: "numpad" },
	{ code: "NumpadParenLeft", category: "numpad" },
	{ code: "NumpadParenRight", category: "numpad" },
	{ code: "NumpadStar", category: "numpad" },
	{ code: "NumpadSubtract", category: "numpad" },

	// Function keys
	{ code: "Escape", category: "function" }, // Shares a name with a key attribute
	{ code: "F1", category: "function" }, // Shares a name with a key attribute
	{ code: "F2", category: "function" }, // Shares a name with a key attribute
	{ code: "F3", category: "function" }, // Shares a name with a key attribute
	{ code: "F4", category: "function" }, // Shares a name with a key attribute
	{ code: "F5", category: "function" }, // Shares a name with a key attribute
	{ code: "F6", category: "function" }, // Shares a name with a key attribute
	{ code: "F7", category: "function" }, // Shares a name with a key attribute
	{ code: "F8", category: "function" }, // Shares a name with a key attribute
	{ code: "F9", category: "function" }, // Shares a name with a key attribute
	{ code: "F10", category: "function" }, // Shares a name with a key attribute
	{ code: "F11", category: "function" }, // Shares a name with a key attribute
	{ code: "F12", category: "function" }, // Shares a name with a key attribute
	{ code: "F13", category: "function" }, // Shares a name with a key attribute
	{ code: "F14", category: "function" }, // Shares a name with a key attribute
	{ code: "F15", category: "function" }, // Shares a name with a key attribute
	{ code: "F16", category: "function" }, // Shares a name with a key attribute
	{ code: "F17", category: "function" }, // Shares a name with a key attribute
	{ code: "F18", category: "function" }, // Shares a name with a key attribute
	{ code: "F19", category: "function" }, // Shares a name with a key attribute
	{ code: "F20", category: "function" }, // Shares a name with a key attribute
	{ code: "F21", category: "function" }, // Shares a name with a key attribute
	{ code: "F22", category: "function" }, // Shares a name with a key attribute
	{ code: "F23", category: "function" }, // Shares a name with a key attribute
	{ code: "F24", category: "function" }, // Shares a name with a key attribute
	{ code: "Fn", category: "function" }, // Shares a name with a key attribute
	{ code: "FnLock", category: "function" }, // Shares a name with a key attribute
	{ code: "PrintScreen", category: "function" }, // Shares a name with a key attribute
	{ code: "ScrollLock", category: "function" }, // Shares a name with a key attribute
	{ code: "Pause", category: "function" }, // Shares a name with a key attribute

	// Media keys
	{ code: "BrowserBack", category: "media" }, // Shares a name with a key attribute
	{ code: "BrowserFavorites", category: "media" }, // Shares a name with a key attribute
	{ code: "BrowserForward", category: "media" }, // Shares a name with a key attribute
	{ code: "BrowserHome", category: "media" }, // Shares a name with a key attribute
	{ code: "BrowserRefresh", category: "media" }, // Shares a name with a key attribute
	{ code: "BrowserSearch", category: "media" }, // Shares a name with a key attribute
	{ code: "BrowserStop", category: "media" }, // Shares a name with a key attribute
	{ code: "Eject", category: "media" }, // Shares a name with a key attribute
	{ code: "LaunchApp1", category: "media" },
	{ code: "LaunchApp2", category: "media" },
	{ code: "LaunchMail", category: "media" }, // Shares a name with a key attribute
	{ code: "MediaPlayPause", category: "media" }, // Shares a name with a key attribute
	{ code: "MediaSelect", category: "media" },
	{ code: "MediaStop", category: "media" }, // Shares a name with a key attribute
	{ code: "MediaTrackNext", category: "media" }, // Shares a name with a key attribute
	{ code: "MediaTrackPrevious", category: "media" }, // Shares a name with a key attribute
	{ code: "Power", category: "media" }, // Shares a name with a key attribute
	{ code: "Sleep", category: "media" },
	{ code: "AudioVolumeDown", category: "media" }, // Shares a name with a key attribute
	{ code: "AudioVolumeMute", category: "media" }, // Shares a name with a key attribute
	{ code: "AudioVolumeUp", category: "media" }, // Shares a name with a key attribute
	{ code: "WakeUp", category: "media" }, // Shares a name with a key attribute

	// Unidentified keys
	{ code: "Unidentified", category: "unidentified" }, // Shares a name with a key attribute
];
const KEY_CODE_NAMES = Object.values(KEY_CODES).map((info) => info.code);
// const KEY_CODE_NAMES_WITHOUT_HANDEDNESS = KEY_CODE_NAMES.filter((code) => !(code.endsWith("Right") && HANDED_KEY_ATTRIBUTE_VALUES.some((modifier) => code === `${modifier}Right`))).map((code) =>
// code.endsWith("Left") && HANDED_KEY_ATTRIBUTE_VALUES.some((modifier) => code === `${modifier}Left`) ? code.replace("Left", "") : code
// );
const NUMPAD_DECIMAL_AND_THOUSANDS_SEPARATORS = ["NumpadComma", "NumpadDecimal"];
const SCAN_CODES_FOR_NON_WRITING_KEYS_THAT_VARY_PER_LOCALE = ["AltRight", ...NUMPAD_DECIMAL_AND_THOUSANDS_SEPARATORS];
const LOCALE_SPECIFIC_KEY_CODES_INFO = KEY_CODES.filter((key) => key.category === "writing-system" || SCAN_CODES_FOR_NON_WRITING_KEYS_THAT_VARY_PER_LOCALE.includes(key.code));
const LOCALE_SPECIFIC_KEY_CODES = LOCALE_SPECIFIC_KEY_CODES_INFO.map((info) => info.code);
const WRITING_SYSTEM_SPECIAL_CHARS = Object.values(KEY_CODES)
	.filter((info) => info.category === "writing-system")
	.flatMap((info) => info.keys?.us?.split(" "))
	.filter((character) => character && !/[a-zA-Z0-9]/.test(character)) as string[];

const KEY_ATTRIBUTE_VALUES_INVOLVING_HANDEDNESS = ["Control", "Meta", "Shift"];
const KEY_ATTRIBUTE_VALUES = new Set([
	// Modifier
	"Alt", // Glyph modifier key
	"AltGraph", // Glyph modifier key
	"CapsLock", // Glyph modifier key
	"Control",
	"Fn",
	"FnLock",
	"Meta",
	"NumLock",
	"ScrollLock",
	"Shift",
	"Symbol",
	"SymbolLock",

	// Legacy modifier
	"Hyper",
	"Super",

	// White space
	"Enter", // Control character
	"Tab", // Control character

	// Navigation
	"ArrowDown",
	"ArrowLeft",
	"ArrowRight",
	"ArrowUp",
	"End",
	"Home",
	"PageDown",
	"PageUp",

	// Editing
	"Backspace", // Control character
	"Clear",
	"Copy",
	"CrSel",
	"Cut",
	"Delete", // Control character
	"EraseEof",
	"ExSel",
	"Insert",
	"Paste",
	"Redo",
	"Undo",

	// UI
	"Accept",
	"Again",
	"Attn",
	"Cancel",
	"ContextMenu",
	"Escape", // Control character
	"Execute",
	"Find",
	"Help",
	"Pause",
	"Play",
	"Props",
	"Select",
	"ZoomIn",
	"ZoomOut",

	// Device
	"BrightnessDown",
	"BrightnessUp",
	"Eject",
	"LogOff",
	"Power",
	"PowerOff",
	"PrintScreen",
	"Hibernate",
	"Standby",
	"WakeUp",

	// IME composition keys
	"AllCandidates",
	"Alphanumeric",
	"CodeInput",
	"Compose",
	"Convert",
	"Dead",
	"FinalMode",
	"GroupFirst",
	"GroupLast",
	"GroupNext",
	"GroupPrevious",
	"ModeChange",
	"NextCandidate",
	"NonConvert",
	"PreviousCandidate",
	"Process",
	"SingleCandidate",

	// Korean-specific
	"HangulMode",
	"HanjaMode",
	"JunjaMode",

	// Japanese-specific
	"Eisu",
	"Hankaku",
	"Hiragana",
	"HiraganaKatakana",
	"KanaMode",
	"KanjiMode",
	"Katakana",
	"Romaji",
	"Zenkaku",
	"ZenkakuHankaku",

	// Common function
	"F1",
	"F2",
	"F3",
	"F4",
	"F5",
	"F6",
	"F7",
	"F8",
	"F9",
	"F10",
	"F11",
	"F12",
	"F13",
	"F14",
	"F15",
	"F16",
	"F17",
	"F18",
	"F19",
	"F20",
	"F21",
	"F22",
	"F23",
	"F24",
	"Soft1",
	"Soft2",
	"Soft3",
	"Soft4",
	"Soft5",
	"Soft6",
	"Soft7",
	"Soft8",
	"Soft9",
	"Soft10",
	"Soft11",
	"Soft12",
	"Soft13",
	"Soft14",
	"Soft15",
	"Soft16",
	"Soft17",
	"Soft18",
	"Soft19",
	"Soft20",
	"Soft21",
	"Soft22",
	"Soft23",
	"Soft24",

	// Multimedia
	"ChannelDown",
	"ChannelUp",
	"Close",
	"MailForward",
	"MailReply",
	"MailSend",
	"MediaClose",
	"MediaFastForward",
	"MediaPause",
	"MediaPlay",
	"MediaPlayPause",
	"MediaRecord",
	"MediaRewind",
	"MediaStop",
	"MediaTrackNext",
	"MediaTrackPrevious",
	"New",
	"Open",
	"Print",
	"Save",
	"SpellCheck",

	// Multimedia numpad
	"Digit11",
	"Digit12",

	// Audio
	"AudioBalanceLeft",
	"AudioBalanceRight",
	"AudioBassBoostDown",
	"AudioBassBoostToggle",
	"AudioBassBoostUp",
	"AudioFaderFront",
	"AudioFaderRear",
	"AudioSurroundModeNext",
	"AudioTrebleDown",
	"AudioTrebleUp",
	"AudioVolumeDown",
	"AudioVolumeUp",
	"AudioVolumeMute",
	"MicrophoneToggle",
	"MicrophoneVolumeDown",
	"MicrophoneVolumeUp",
	"MicrophoneVolumeMute",

	// Speech
	"SpeechCorrectionList",
	"SpeechInputToggle",

	// Application
	"LaunchApplication1",
	"LaunchApplication2",
	"LaunchCalendar",
	"LaunchContacts",
	"LaunchMail",
	"LaunchMediaPlayer",
	"LaunchMusicPlayer",
	"LaunchPhone",
	"LaunchScreenSaver",
	"LaunchSpreadsheet",
	"LaunchWebBrowser",
	"LaunchWebCam",
	"LaunchWordProcessor",

	// Browser
	"BrowserBack",
	"BrowserFavorites",
	"BrowserForward",
	"BrowserHome",
	"BrowserRefresh",
	"BrowserSearch",
	"BrowserStop",

	// Mobile phone
	"AppSwitch",
	"Call",
	"Camera",
	"CameraFocus",
	"EndCall",
	"GoBack",
	"GoHome",
	"HeadsetHook",
	"LastNumberRedial",
	"Notification",
	"MannerMode",
	"VoiceDial",

	// TV
	"TV",
	"TV3DMode",
	"TVAntennaCable",
	"TVAudioDescription",
	"TVAudioDescriptionMixDown",
	"TVAudioDescriptionMixUp",
	"TVContentsMenu",
	"TVDataService",
	"TVInput",
	"TVInputComponent1",
	"TVInputComponent2",
	"TVInputComposite1",
	"TVInputComposite2",
	"TVInputHDMI1",
	"TVInputHDMI2",
	"TVInputHDMI3",
	"TVInputHDMI4",
	"TVInputVGA1",
	"TVMediaContext",
	"TVNetwork",
	"TVNumberEntry",
	"TVPower",
	"TVRadioService",
	"TVSatellite",
	"TVSatelliteBS",
	"TVSatelliteCS",
	"TVSatelliteToggle",
	"TVTerrestrialAnalog",
	"TVTerrestrialDigital",
	"TVTimer",

	// Media controls
	"AVRInput",
	"AVRPower",
	"ColorF0Red",
	"ColorF1Green",
	"ColorF2Yellow",
	"ColorF3Blue",
	"ColorF4Grey",
	"ColorF5Brown",
	"ClosedCaptionToggle",
	"Dimmer",
	"DisplaySwap",
	"DVR",
	"Exit",
	"FavoriteClear0",
	"FavoriteClear1",
	"FavoriteClear2",
	"FavoriteClear3",
	"FavoriteRecall0",
	"FavoriteRecall1",
	"FavoriteRecall2",
	"FavoriteRecall3",
	"FavoriteStore0",
	"FavoriteStore1",
	"FavoriteStore2",
	"FavoriteStore3",
	"Guide",
	"GuideNextDay",
	"GuidePreviousDay",
	"Info",
	"InstantReplay",
	"Link",
	"ListProgram",
	"LiveContent",
	"Lock",
	"MediaApps",
	"MediaAudioTrack",
	"MediaLast",
	"MediaSkipBackward",
	"MediaSkipForward",
	"MediaStepBackward",
	"MediaStepForward",
	"MediaTopMenu",
	"NavigateIn",
	"NavigateNext",
	"NavigateOut",
	"NavigatePrevious",
	"NextFavoriteChannel",
	"NextUserProfile",
	"OnDemand",
	"Pairing",
	"PinPDown",
	"PinPMove",
	"PinPToggle",
	"PinPUp",
	"PlaySpeedDown",
	"PlaySpeedReset",
	"PlaySpeedUp",
	"RandomToggle",
	"RcLowBattery",
	"RecordSpeedNext",
	"RfBypass",
	"ScanChannelsToggle",
	"ScreenModeNext",
	"Settings",
	"SplitScreenToggle",
	"STBInput",
	"STBPower",
	"Subtitle",
	"Teletext",
	"VideoModeNext",
	"Wink",
	"ZoomToggle",

	// Unidentified
	"Unidentified",
]);
