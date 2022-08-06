export function makeKeyboardModifiersBitfield(e: WheelEvent | PointerEvent | KeyboardEvent): number {
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

// This function is a naive, temporary solution to allow non-Latin keyboards to fall back on the physical QWERTY layout
export function getLatinKey(e: KeyboardEvent): string | null {
	const key = e.key.toLowerCase();
	const isPrintable = !ALL_PRINTABLE_KEYS.has(e.key);

	// Control characters (those which are non-printable) are handled normally
	if (!isPrintable) return key;

	// These non-Latin characters should fall back to the Latin equivalent at the key location
	const LAST_LATIN_UNICODE_CHAR = 0x024f;
	if (key.length > 1 || key.charCodeAt(0) > LAST_LATIN_UNICODE_CHAR) return keyCodeToKey(e.code);

	// Otherwise, this is a printable Latin character
	return e.key.toLowerCase();
}

export function keyCodeToKey(code: string): string | null {
	// Letters
	if (code.match(/^Key[A-Z]$/)) return code.replace("Key", "").toLowerCase();

	// Numbers
	if (code.match(/^Digit[0-9]$/)) return code.replace("Digit", "");
	if (code.match(/^Numpad[0-9]$/)) return code.replace("Numpad", "");

	// Function keys
	if (code.match(/^F[1-9]|F1[0-9]|F20$/)) return code.replace("F", "").toLowerCase();

	// Other characters
	if (SPECIAL_CHARACTERS[code]) return SPECIAL_CHARACTERS[code];

	return null;
}

const SPECIAL_CHARACTERS: Record<string, string> = {
	BracketLeft: "[",
	BracketRight: "]",
	Backslash: "\\",
	Slash: "/",
	Period: ".",
	Comma: ",",
	Equal: "=",
	Minus: "-",
	Quote: "'",
	Semicolon: ";",
	NumpadEqual: "=",
	NumpadDivide: "/",
	NumpadMultiply: "*",
	NumpadSubtract: "-",
	NumpadAdd: "+",
	NumpadDecimal: ".",
} as const;

const ALL_PRINTABLE_KEYS = new Set([
	// Modifier
	"Alt",
	"AltGraph",
	"CapsLock",
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
	"Enter",
	"Tab",
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
	"Backspace",
	"Clear",
	"Copy",
	"CrSel",
	"Cut",
	"Delete",
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
	"Escape",
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
	"Soft1",
	"Soft2",
	"Soft3",
	"Soft4",
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
	"Key11",
	"Key12",
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
]);
