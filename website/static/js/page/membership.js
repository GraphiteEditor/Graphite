// Several times the size the credits show it at, so it stays sharp on high-density screens
const AVATAR_SIZE = 256;

// Matches the Worker's wait before it will email the same address another sign-in link
const SIGN_IN_EMAIL_INTERVAL_MILLISECONDS = 60 * 1000;

// Matches how long the Worker's sign-in links work, which is as long as the button offers to resend one
const SIGN_IN_LINK_LIFETIME_MILLISECONDS = 15 * 60 * 1000;
const SIGN_IN_EMAIL_STORAGE_KEY = "graphite-membership-sign-in-email";

// Encoding the crop waits for this long a pause, rather than running on every step of a drag
const COMMIT_DELAY_MILLISECONDS = 150;

// Matches the Worker's limit, which is as many as fit in a row of pickers
const COLORS_LIMIT = 9;

// How far a press on a color travels before it's a drag rather than a click that opens the picker
const COLOR_DRAG_THRESHOLD_PIXELS = 4;

// Each added color starts as the first of these not already in its row, so it stands apart from the others
const ADDED_COLORS = ["#16323f", "#cd8f7a", "#d2697c", "#4fa58f", "#5b7bd5", "#e8a33d", "#7a5cff", "#803847", "#e5c299"];

// A chosen picture is framed in a square the member drags and zooms, and only that square is uploaded
window.addEventListener("DOMContentLoaded", () => {
	const fileInput = /** @type {HTMLInputElement | null} */ (document.querySelector("[data-credits-avatar-file]"));
	const preview = /** @type {HTMLImageElement | null} */ (document.querySelector("[data-credits-avatar]"));
	const removeCheckbox = /** @type {HTMLInputElement | null} */ (document.querySelector("[data-remove-avatar-checkbox]"));
	const cropper = /** @type {HTMLElement | null} */ (document.querySelector("[data-avatar-cropper]"));
	const frame = /** @type {HTMLElement | null} */ (document.querySelector("[data-crop-frame]"));
	const image = /** @type {HTMLImageElement | null} */ (document.querySelector("[data-crop-image]"));
	const zoomSlider = /** @type {HTMLInputElement | null} */ (document.querySelector("[data-crop-zoom]"));
	const fileStatus = document.querySelector("[data-credits-avatar-file-name]");
	const form = fileInput?.form;
	if (!fileInput || !preview || !cropper || !frame || !image || !zoomSlider || !form) return;

	let fileName = "";
	let zoom = 1;
	// The center of the cropped square, in the picture's own pixels
	let centerX = 0;
	let centerY = 0;
	let commitTimeout = 0;
	/** @type {{ pointerX: number, pointerY: number, centerX: number, centerY: number } | undefined} */
	let dragStart;

	const side = () => Math.min(image.naturalWidth, image.naturalHeight) / zoom;
	const pixelsPerImagePixel = () => frame.clientWidth / side();

	const commit = () => {
		window.clearTimeout(commitTimeout);
		commitTimeout = 0;

		const cropped = cropToFile(image, centerX - side() / 2, centerY - side() / 2, side(), fileName);
		if (!cropped) return;

		const transfer = new DataTransfer();
		transfer.items.add(cropped);
		fileInput.files = transfer.files;

		if (preview.src.startsWith("blob:")) URL.revokeObjectURL(preview.src);
		preview.src = URL.createObjectURL(cropped);
		preview.hidden = false;
	};

	// Keeps the square within the picture, then moves and scales the picture behind the frame to match
	const render = () => {
		const half = side() / 2;
		centerX = Math.min(Math.max(centerX, half), image.naturalWidth - half);
		centerY = Math.min(Math.max(centerY, half), image.naturalHeight - half);

		const scale = pixelsPerImagePixel();
		image.style.width = `${image.naturalWidth * scale}px`;
		image.style.translate = `${(half - centerX) * scale}px ${(half - centerY) * scale}px`;

		window.clearTimeout(commitTimeout);
		commitTimeout = window.setTimeout(commit, COMMIT_DELAY_MILLISECONDS);
	};

	const setZoom = (/** @type {number} */ value) => {
		zoom = Math.min(Math.max(value, Number(zoomSlider.min)), Number(zoomSlider.max));
		zoomSlider.value = String(zoom);
		render();
	};

	fileInput.addEventListener("change", async () => {
		const file = fileInput.files?.[0];
		if (fileStatus) fileStatus.textContent = file?.name || "No file selected";
		if (!file) return;

		// A picture the browser can't decode is sent as it is, for the server to turn down
		if (image.src.startsWith("blob:")) URL.revokeObjectURL(image.src);
		image.src = URL.createObjectURL(file);
		const decoded = await image.decode().then(
			() => true,
			() => false,
		);
		cropper.hidden = !decoded;
		if (!decoded) return;

		fileName = file.name;
		centerX = image.naturalWidth / 2;
		centerY = image.naturalHeight / 2;
		if (removeCheckbox) removeCheckbox.checked = false;

		setZoom(1);
		commit();
	});

	frame.addEventListener("pointerdown", (e) => {
		frame.setPointerCapture(e.pointerId);
		dragStart = { pointerX: e.clientX, pointerY: e.clientY, centerX, centerY };
	});
	frame.addEventListener("pointermove", (e) => {
		if (!dragStart) return;

		// The picture follows the pointer, so the square moves the opposite way across it
		centerX = dragStart.centerX - (e.clientX - dragStart.pointerX) / pixelsPerImagePixel();
		centerY = dragStart.centerY - (e.clientY - dragStart.pointerY) / pixelsPerImagePixel();
		render();
	});
	const endDrag = () => {
		dragStart = undefined;
	};
	frame.addEventListener("pointerup", endDrag);
	frame.addEventListener("pointercancel", endDrag);

	// The frame narrows with the page on small screens, which changes how far the picture sits from its edges
	new ResizeObserver(() => {
		if (!cropper.hidden) render();
	}).observe(frame);

	frame.addEventListener(
		"wheel",
		(e) => {
			e.preventDefault();
			setZoom(zoom * Math.exp(-e.deltaY / 500));
		},
		{ passive: false },
	);
	zoomSlider.addEventListener("input", () => setZoom(Number(zoomSlider.value)));

	// Arrow keys move the picture a twentieth of the frame, as dragging would
	frame.addEventListener("keydown", (e) => {
		/** @type {Record<string, [number, number]>} */
		const directions = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
		const direction = directions[e.key];
		if (!direction) return;

		e.preventDefault();
		centerX -= (direction[0] * side()) / 20;
		centerY -= (direction[1] * side()) / 20;
		render();
	});

	// The form is read after this event, so a crop still waiting to be encoded is finished first
	form.addEventListener("submit", () => {
		if (commitTimeout) commit();
	});
});

// The preview mirrors the form as it's edited, styled and animated by the same stylesheet and particles script as the credits
window.addEventListener("DOMContentLoaded", () => {
	const previewBox = /** @type {HTMLElement | null} */ (document.querySelector("[data-credits-preview]"));
	const entry = /** @type {HTMLElement | null} */ (document.querySelector("[data-credits-preview-entry]"));
	let previewName = /** @type {HTMLElement | null} */ (document.querySelector("[data-credits-preview-name]"));
	const previewAvatar = /** @type {HTMLImageElement | null} */ (document.querySelector("[data-credits-preview-avatar]"));
	const nameInput = /** @type {HTMLInputElement | null} */ (document.querySelector("[data-credits-name]"));
	const linkInput = /** @type {HTMLInputElement | null} */ (document.querySelector("[data-credits-link]"));
	const colorStyle = /** @type {HTMLSelectElement | null} */ (document.querySelector("[data-credits-color-style]"));
	const nameColorsRow = /** @type {HTMLElement | null} */ (document.querySelector(`[data-color-row="credits_colors"]`));
	const particleColorsRow = /** @type {HTMLElement | null} */ (document.querySelector(`[data-color-row="credits_particle_colors"]`));
	const particleColorsField = /** @type {HTMLElement | null} */ (document.querySelector("[data-particle-colors-field]"));
	const swatchTemplate = /** @type {HTMLTemplateElement | null} */ (document.querySelector("[data-swatch-template]"));
	const shapeRow = /** @type {HTMLElement | null} */ (document.querySelector("[data-shape-row]"));
	const shapeList = /** @type {HTMLElement | null} */ (document.querySelector("[data-shape-list]"));
	const shapeTemplate = /** @type {HTMLTemplateElement | null} */ (document.querySelector("[data-shape-template]"));
	const addShape = /** @type {HTMLButtonElement | null} */ (document.querySelector("[data-add-shape]"));
	const removeShape = /** @type {HTMLButtonElement | null} */ (document.querySelector("[data-remove-shape]"));
	const particleMotion = /** @type {HTMLSelectElement | null} */ (document.querySelector("[data-credits-particle-motion]"));
	const particleShapesField = /** @type {HTMLElement | null} */ (document.querySelector("[data-particle-shapes-field]"));
	const avatar = /** @type {HTMLImageElement | null} */ (document.querySelector("[data-credits-avatar]"));
	const removeAvatar = /** @type {HTMLInputElement | null} */ (document.querySelector("[data-remove-avatar-checkbox]"));
	const form = colorStyle?.form;
	if (!previewBox || !entry || !previewName || !nameInput || !colorStyle || !nameColorsRow || !swatchTemplate || !form) return;

	const shapeSelects = () => Array.from(/** @type {NodeListOf<HTMLSelectElement>} */ (shapeList?.querySelectorAll("[data-particle-shape]") || []));
	const shapeTemplateMenu = shapeTemplate?.content.querySelector("[data-particle-shape]");
	const shapeCount = shapeTemplateMenu instanceof HTMLSelectElement ? shapeTemplateMenu.options.length : 0;

	const shapeRemoval = removalMode(shapeRow, removeShape);
	const colorPopoverElement = /** @type {HTMLElement | null} */ (document.querySelector("[data-color-popover]"));
	const picker = colorPopoverElement ? colorPopover(colorPopoverElement) : undefined;

	// The name and the particles each keep their own colors, sharing the one picker
	const nameColors = colorRow(nameColorsRow, swatchTemplate, picker, () => update());
	const particleColors = particleColorsRow ? colorRow(particleColorsRow, swatchTemplate, picker, () => update()) : undefined;

	// Only members whose tier includes gradients have the buttons for keeping more than one name color
	const manyColors = Boolean(nameColors.addButton);

	// Each new shape copies its template, starting as the first one not already chosen
	const appendShape = () => {
		const choice = shapeTemplate?.content.firstElementChild?.cloneNode(true);
		const select = choice instanceof HTMLElement ? choice.querySelector("[data-particle-shape]") : undefined;
		if (!(choice instanceof HTMLElement) || !(select instanceof HTMLSelectElement) || !shapeList) return;

		const used = shapeSelects().map((other) => other.value);
		select.value = Array.from(select.options, (option) => option.value).find((shape) => !used.includes(shape)) || select.value;
		shapeList.append(choice);
	};

	const setProperty = (/** @type {string} */ property, /** @type {string | undefined} */ value) => {
		if (value) entry.style.setProperty(property, value);
		else entry.style.removeProperty(property);
	};

	const update = () => {
		if (!previewName) return;

		// Choosing a motion starts the particles with a shape, while None hides them but keeps the shapes for later
		const motion = particleMotion?.value || "";
		const particlesOn = motion !== "";
		if (particlesOn && shapeSelects().length === 0) appendShape();
		const shapes = shapeSelects().map((select) => select.value);

		const style = colorStyle.value;
		const fading = style !== "" && style !== "solid";

		// A fade needs two colors and shows all of them with the buttons for more, while a solid name shows just its first, beside the menu
		const fewest = fading ? 2 : style ? 1 : 0;
		const showsAll = manyColors && fading;
		while (nameColors.inputs().length < fewest) nameColors.append();
		const inputs = nameColors.inputs();
		inputs.forEach((input, index) => {
			if (input.parentElement) input.parentElement.hidden = !showsAll && index >= fewest;
			input.setAttribute("aria-label", `Color ${index + 1}`);
		});
		nameColorsRow.hidden = fewest === 0;
		nameColorsRow.parentElement?.classList.toggle("color-beside-menu", style === "solid");
		nameColors.showButtons(showsAll, fewest);

		// The particles take any number of colors, or none to leave each shape its own
		const particleInputs = particleColors?.inputs() || [];
		particleInputs.forEach((input, index) => input.setAttribute("aria-label", `Particle color ${index + 1}`));
		particleColors?.showButtons(true, 0);
		if (particleColorsField) particleColorsField.hidden = !particlesOn;

		// The picker closes along with the color it was editing, whether that was removed or hidden
		const editing = picker?.editing();
		if (editing && (!editing.isConnected || !editing.offsetParent)) picker?.close();

		// The name takes its first color, or fades through them all
		const colors = inputs.filter((input) => !input.parentElement?.hidden).map((input) => input.value);
		setProperty("--name-color", style ? colors[0] : undefined);
		setProperty("--name-fade-colors", fading ? colors.slice(1).join(", ") : undefined);
		setProperty("--name-color-count", fading ? String(colors.length) : undefined);
		setProperty("--name-color-bands", fading ? colorBands(colors) : undefined);
		setProperty("--particle-colors", particlesOn ? particleInputs.map((input) => input.value).join(", ") : undefined);
		entry.className = fading ? `name-fade ${style}` : "";

		// The last shape can't be removed, since None is what turns the particles off
		if (addShape) addShape.hidden = shapes.length >= shapeCount;
		if (removeShape) removeShape.hidden = shapes.length <= 1;
		if (shapes.length <= 1) shapeRemoval.set(false);
		if (particleShapesField) particleShapesField.hidden = !particlesOn;
		entry.dataset.nameParticleShapes = particlesOn ? shapes.join(",") : "";
		entry.dataset.nameParticleMotion = motion;

		// A linked name is a link in the credits, which underlines it
		const linked = Boolean(linkInput?.value.trim());
		if (linked !== (previewName.tagName === "A")) {
			const replacement = document.createElement(linked ? "a" : "span");
			replacement.className = "credits-name";
			replacement.toggleAttribute("data-credits-preview-name", true);
			previewName.replaceWith(replacement);
			previewName = replacement;
		}
		previewName.textContent = nameInput.value.trim() || "Your name";

		if (previewAvatar && avatar) {
			previewAvatar.src = avatar.src;
			previewAvatar.hidden = avatar.hidden || Boolean(removeAvatar?.checked);
		}
	};

	form.addEventListener("input", update);

	// Someone who mistakes the previewed name for the field is taken to the field
	entry.addEventListener("click", (e) => {
		if (!(e.target instanceof Element) || !e.target.closest("[data-credits-preview-name]")) return;

		nameInput.focus();
		nameInput.select();
	});

	addShape?.addEventListener("click", () => {
		shapeRemoval.set(false);
		appendShape();
		update();
	});

	// While removing, pressing a shape's menu takes it out instead of opening it
	shapeList?.addEventListener("mousedown", (e) => {
		if (!shapeRemoval.on() || !(e.target instanceof HTMLSelectElement)) return;

		e.preventDefault();
		e.target.parentElement?.remove();
		update();
	});
	shapeList?.addEventListener("keydown", (e) => {
		if (!shapeRemoval.on() || !(e.target instanceof HTMLSelectElement) || !["Enter", " "].includes(e.key)) return;

		e.preventDefault();
		e.target.parentElement?.remove();
		update();
	});

	// The picture's thumbnail changes when a crop is encoded, which no form event announces
	if (avatar) new MutationObserver(update).observe(avatar, { attributes: true, attributeFilter: ["src", "hidden"] });

	update();
	previewBox.hidden = false;
});

// After a sign-in link is requested, the button counts down until the Worker would send that address another, timed in this browser so it says nothing about who's a member
window.addEventListener("DOMContentLoaded", () => {
	const form = document.querySelector("[data-email-sign-in]");
	const input = form?.querySelector("[data-sign-in-address]");
	const button = form?.querySelector("[data-sign-in-button]");
	if (!(form instanceof HTMLFormElement) || !(input instanceof HTMLInputElement) || !(button instanceof HTMLButtonElement)) return;

	const label = button.textContent;
	const normalized = () => input.value.trim().toLowerCase();
	form.addEventListener("submit", () => storeSignInEmail({ address: normalized() }));

	// The countdown starts once the Worker has replied with its confirmation, which comes after it timed the request
	const requested = readSignInEmail();
	const confirmation = document.querySelector(`[data-notice="sign-in-link-sent"]`);
	const confirmed = confirmation instanceof HTMLElement && !confirmation.hidden;
	if (requested && confirmed && requested.sentAt === undefined) {
		requested.sentAt = Date.now();
		storeSignInEmail(requested);
	}
	if (!requested || requested.sentAt === undefined) return;

	const { address, sentAt } = requested;
	const resendableAt = sentAt + SIGN_IN_EMAIL_INTERVAL_MILLISECONDS;
	const expiresAt = sentAt + SIGN_IN_LINK_LIFETIME_MILLISECONDS;
	if (Date.now() >= expiresAt) {
		forgetSignInEmail();
		return;
	}
	if (confirmed || Date.now() < resendableAt) input.value = address;

	// Another address can be sent to right away, since the wait is for each address
	const update = () => {
		const resending = normalized() === address && Date.now() < expiresAt;
		const seconds = resending ? Math.ceil((resendableAt - Date.now()) / 1000) : 0;

		button.disabled = seconds > 0;
		button.classList.toggle("arrow", seconds <= 0);
		if (seconds > 0) button.textContent = `Resend in ${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
		else button.textContent = resending ? "Resend sign-in link" : label;
	};

	input.addEventListener("input", update);
	update();

	// Once the earlier link expires, even with the page left open, the button asks for a new one as if for the first time
	setTimeout(() => {
		forgetSignInEmail();
		update();
	}, expiresAt - Date.now());

	if (Date.now() >= resendableAt) return;

	const timer = setInterval(() => {
		update();
		if (Date.now() >= resendableAt) clearInterval(timer);
	}, 250);
});

// Saving waits for the form to differ from how it loaded, so undoing every change disables it again
window.addEventListener("DOMContentLoaded", () => {
	const form = /** @type {HTMLFormElement | null} */ (document.querySelector("[data-credits-form]"));
	const saveButton = form?.querySelector("[data-save-changes]");
	if (!form || !(saveButton instanceof HTMLButtonElement)) return;

	// A chosen picture is compared by its details, since each new crop is a new file, while an empty file input reads as nothing
	const contents = () => {
		const describe = (/** @type {FormDataEntryValue} */ value) => {
			if (typeof value === "string") return value;
			return value.size > 0 ? `${value.name} ${value.size} ${value.lastModified}` : "";
		};
		return JSON.stringify(Array.from(new FormData(form), ([name, value]) => [name, describe(value)]));
	};

	// Taken after the preview has run, since it can add the colors a stored style needs
	const loaded = contents();
	const refresh = () => {
		saveButton.disabled = contents() === loaded;
	};

	// Colors and shapes are added, removed, and reordered without form events, and a crop shows itself only through the picture's address
	form.addEventListener("input", refresh);
	form.addEventListener("change", refresh);
	new MutationObserver(refresh).observe(form, { childList: true, subtree: true, attributes: true, attributeFilter: ["src"] });

	refresh();
});

/**
 * A row of colors, each added with its plus button, taken out while its minus button is pressed, dragged into order, and edited in the page's picker.
 * @param {HTMLElement} row
 * @param {HTMLTemplateElement} template
 * @param {ReturnType<typeof colorPopover> | undefined} picker
 * @param {() => void} changed
 */
function colorRow(row, template, picker, changed) {
	const swatches = row.querySelector("[data-color-swatches]") || row;
	const addButton = /** @type {HTMLButtonElement | null} */ (row.querySelector("[data-add-color]"));
	const removeButton = /** @type {HTMLButtonElement | null} */ (row.querySelector("[data-remove-color]"));
	const removal = removalMode(row, removeButton);

	const inputs = () => Array.from(/** @type {NodeListOf<HTMLInputElement>} */ (swatches.querySelectorAll("[data-color-picker]")));

	// Each new color copies the template, named for this row's field and starting as the first color not already in it
	const append = () => {
		const swatch = template.content.firstElementChild?.cloneNode(true);
		const input = swatch instanceof HTMLElement ? swatch.querySelector("[data-color-picker]") : undefined;
		if (!(swatch instanceof HTMLElement) || !(input instanceof HTMLInputElement)) return;

		const used = inputs().map((other) => other.value);
		input.name = row.dataset.colorRow || input.name;
		input.value = ADDED_COLORS.find((color) => !used.includes(color)) || input.value;
		swatches.append(swatch);
	};

	// A full row has no plus button, which leaves its place for the last color, and the minus stops at the fewest the row needs
	const showButtons = (/** @type {boolean} */ shown, /** @type {number} */ fewest) => {
		const count = inputs().length;
		if (addButton) addButton.hidden = !shown || count >= COLORS_LIMIT;
		if (removeButton) {
			removeButton.hidden = !shown;
			removeButton.disabled = count <= fewest;
		}
		if (!shown || count <= fewest) removal.set(false);
	};

	addButton?.addEventListener("click", () => {
		removal.set(false);
		append();
		changed();
	});

	// Dragging a color slides it into place among the others, while a press that doesn't move still opens its picker
	/** @type {{ swatch: HTMLElement, startX: number, moved: boolean } | undefined} */
	let drag;
	let suppressClick = false;

	const dragMove = (/** @type {PointerEvent} */ e) => {
		if (!drag || (!drag.moved && Math.abs(e.clientX - drag.startX) < COLOR_DRAG_THRESHOLD_PIXELS)) return;
		const dragged = drag.swatch;
		const others = Array.from(swatches.children).filter((other) => other !== dragged && other instanceof HTMLElement && !other.hidden);
		if (others.length === 0) return;

		if (!drag.moved) picker?.close();
		drag.moved = true;
		dragged.classList.add("dragged");
		row.classList.add("dragging");

		// It goes before the first other color with its middle past the pointer, or last when there's none
		const next = others.find((other) => {
			const box = other.getBoundingClientRect();
			return e.clientX < box.left + box.width / 2;
		});
		if (next ? next === dragged.nextElementSibling : !dragged.nextElementSibling) return;

		if (next) next.before(dragged);
		else swatches.append(dragged);
		changed();
	};

	// Listened for on the window, since a swatch that moves in the page can lose its hold on the pointer
	const dragEnd = () => {
		suppressClick = Boolean(drag?.moved);
		drag?.swatch.classList.remove("dragged");
		row.classList.remove("dragging");
		drag = undefined;

		window.removeEventListener("pointermove", dragMove);
		window.removeEventListener("pointerup", dragEnd);
		window.removeEventListener("pointercancel", dragEnd);
	};

	swatches.addEventListener("pointerdown", (e) => {
		if (!(e instanceof PointerEvent)) return;
		suppressClick = false;
		const swatch = e.target instanceof HTMLInputElement ? e.target.parentElement : undefined;
		if (removal.on() || !swatch || e.button !== 0) return;

		drag = { swatch, startX: e.clientX, moved: false };
		window.addEventListener("pointermove", dragMove);
		window.addEventListener("pointerup", dragEnd);
		window.addEventListener("pointercancel", dragEnd);
	});

	swatches.addEventListener("click", (e) => {
		if (!(e.target instanceof HTMLInputElement)) return;

		// A drag ends with a click, which would otherwise open the picker
		if (suppressClick) {
			suppressClick = false;
			e.preventDefault();
			return;
		}

		e.preventDefault();

		// While removing, clicking a color takes it out instead of opening its picker
		if (removal.on()) {
			e.target.parentElement?.remove();
			changed();
			return;
		}

		// The page's own picker opens in place of the browser's
		picker?.toggle(e.target);
	});

	return { inputs, append, showButtons, addButton };
}

/**
 * The colors twice over as bands with crisp edges, matching how the Worker writes them for the credits.
 * @param {string[]} colors
 */
function colorBands(colors) {
	const sequence = [...colors, ...colors];
	const position = (/** @type {number} */ index) => `${Math.round((index / sequence.length) * 10000) / 100}%`;
	return sequence.map((color, index) => `${color} ${position(index)} ${position(index + 1)}`).join(", ");
}

/**
 * A minus button that stays pressed while it's on, crossing out each item in its row under the pointer until one is clicked away.
 * @param {HTMLElement | null} row
 * @param {HTMLButtonElement | null} button
 */
function removalMode(row, button) {
	let active = false;
	const set = (/** @type {boolean} */ on) => {
		active = on;
		row?.classList.toggle("removing", on);
		button?.setAttribute("aria-pressed", String(on));
	};
	button?.addEventListener("click", () => set(!active));

	return { on: () => active, set };
}

/**
 * A square of saturation and brightness beside a hue slider, above the hex code to type or copy, editing one color input at a time.
 * @param {HTMLElement} popover
 */
function colorPopover(popover) {
	const area = popover.querySelector("[data-saturation-value]");
	const hueSlider = popover.querySelector("[data-hue-slider]");
	const hexField = popover.querySelector("[data-hex-field]");

	/** @type {HTMLInputElement | undefined} */
	let editing;
	// Held apart from the hex code, so the hue survives passing through grays and black
	let hue = 0;
	let saturation = 0;
	let value = 0;

	const clamp = (/** @type {number} */ fraction) => Math.min(Math.max(fraction, 0), 1);

	const render = () => {
		popover.style.setProperty("--hue", String(hue));
		popover.style.setProperty("--saturation", String(saturation));
		popover.style.setProperty("--value", String(value));
		hueSlider?.setAttribute("aria-valuenow", String(Math.round(hue)));
		area?.setAttribute("aria-valuetext", `Saturation ${Math.round(saturation * 100)}%, brightness ${Math.round(value * 100)}%`);
		if (hexField instanceof HTMLInputElement && document.activeElement !== hexField) hexField.value = hsvToHex(hue, saturation, value);
	};

	// The input announces the change like the browser's picker would, so the form follows it
	const commit = () => {
		const hex = hsvToHex(hue, saturation, value);
		if (editing && editing.value !== hex) {
			editing.value = hex;
			editing.dispatchEvent(new Event("input", { bubbles: true }));
		}
		render();
	};

	const close = (returnFocus = false) => {
		if (returnFocus) editing?.focus();
		editing?.classList.remove("editing");
		editing = undefined;
		popover.hidden = true;
	};

	// Opens below the color, kept within its field so it doesn't spill out of the panel
	const open = (/** @type {HTMLInputElement} */ input) => {
		editing?.classList.remove("editing");
		editing = input;
		editing.classList.add("editing");
		[hue, saturation, value] = hexToHsv(input.value, hue);

		// The one picker moves into whichever row holds the color, so it opens below that row
		const holder = input.closest("[data-color-row]");
		if (holder && popover.parentElement !== holder) holder.append(popover);
		popover.hidden = false;
		render();

		// A row beside the menu is narrower than the popover, which then reaches back under the menu
		const swatch = input.parentElement;
		const row = popover.offsetParent;
		const field = row?.parentElement;
		if (swatch && row && field) {
			const fieldStart = field.getBoundingClientRect().left - row.getBoundingClientRect().left;
			popover.style.left = `${Math.max(fieldStart, Math.min(swatch.offsetLeft, row.clientWidth - popover.offsetWidth))}px`;
		}
		if (area instanceof HTMLElement) area.focus();
	};

	/**
	 * Follows a pointer pressed on the element until it lets go, wherever it goes in the meantime.
	 * @param {Element | null} element
	 * @param {(x: number, y: number) => void} move
	 */
	const followPointer = (element, move) => {
		if (!(element instanceof HTMLElement)) return;
		const report = (/** @type {PointerEvent} */ e) => {
			const box = element.getBoundingClientRect();
			move(clamp((e.clientX - box.left) / box.width), clamp((e.clientY - box.top) / box.height));
			commit();
		};
		element.addEventListener("pointerdown", (e) => {
			element.setPointerCapture(e.pointerId);
			report(e);
		});
		element.addEventListener("pointermove", (e) => {
			if (element.hasPointerCapture(e.pointerId)) report(e);
		});
	};
	followPointer(area, (x, y) => {
		saturation = x;
		value = 1 - y;
	});
	followPointer(hueSlider, (_x, y) => {
		hue = y * 360;
	});

	// Arrow keys step by a hundredth of the way, or a tenth with Shift held
	area?.addEventListener("keydown", (e) => {
		if (!(e instanceof KeyboardEvent)) return;
		const step = e.shiftKey ? 0.1 : 0.01;
		/** @type {Record<string, [number, number]>} */
		const directions = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, 1], ArrowDown: [0, -1] };
		const direction = directions[e.key];
		if (!direction) return;

		e.preventDefault();
		saturation = clamp(saturation + direction[0] * step);
		value = clamp(value + direction[1] * step);
		commit();
	});
	hueSlider?.addEventListener("keydown", (e) => {
		if (!(e instanceof KeyboardEvent)) return;
		/** @type {Record<string, number>} */
		const directions = { ArrowUp: -1, ArrowLeft: -1, ArrowDown: 1, ArrowRight: 1 };
		const direction = directions[e.key];
		if (!direction) return;

		e.preventDefault();
		hue = clamp((hue + direction * (e.shiftKey ? 36 : 3.6)) / 360) * 360;
		commit();
	});

	// A typed code takes effect once it's complete, accepting three or six digits with or without the number sign
	if (hexField instanceof HTMLInputElement) {
		hexField.addEventListener("input", () => {
			const hex = parseHex(hexField.value);
			if (!hex) return;

			[hue, saturation, value] = hexToHsv(hex, hue);
			commit();
		});
		hexField.addEventListener("focus", () => hexField.select());
		hexField.addEventListener("blur", render);

		// Enter would otherwise submit the form
		hexField.addEventListener("keydown", (e) => {
			if (e.key !== "Enter") return;
			e.preventDefault();
			close(true);
		});
	}

	// Pressing anywhere outside the picker and its color closes it, as does Escape
	document.addEventListener("pointerdown", (e) => {
		if (editing && e.target instanceof Node && !popover.contains(e.target) && e.target !== editing) close();
	});
	document.addEventListener("keydown", (e) => {
		if (editing && e.key === "Escape") close(true);
	});

	return {
		editing: () => editing,
		close,
		toggle: (/** @type {HTMLInputElement} */ input) => (input === editing ? close() : open(input)),
	};
}

/**
 * @param {number} hue In degrees.
 * @param {number} saturation From 0 to 1.
 * @param {number} value From 0 to 1.
 */
function hsvToHex(hue, saturation, value) {
	const channel = (/** @type {number} */ offset) => {
		const position = (offset + hue / 60) % 6;
		return value - value * saturation * Math.max(0, Math.min(position, 4 - position, 1));
	};
	return `#${[5, 3, 1]
		.map((offset) =>
			Math.round(channel(offset) * 255)
				.toString(16)
				.padStart(2, "0"),
		)
		.join("")}`;
}

/**
 * A gray has no hue of its own, so it keeps the one given.
 * @param {string} hex
 * @param {number} grayHue
 * @returns {[number, number, number]}
 */
function hexToHsv(hex, grayHue) {
	const [red = 0, green = 0, blue = 0] = [1, 3, 5].map((start) => parseInt(hex.slice(start, start + 2), 16) / 255);
	const brightest = Math.max(red, green, blue);
	const range = brightest - Math.min(red, green, blue);

	let hue = grayHue;
	if (range > 0 && brightest === red) hue = (60 * ((green - blue) / range) + 360) % 360;
	else if (range > 0 && brightest === green) hue = 60 * ((blue - red) / range + 2);
	else if (range > 0) hue = 60 * ((red - green) / range + 4);

	return [hue, brightest === 0 ? 0 : range / brightest, brightest];
}

/**
 * A six-digit lowercase hex code from three or six digits, with or without the number sign.
 * @param {string} text
 */
function parseHex(text) {
	const digits = text.trim().replace(/^#/, "");
	if (!/^([\da-f]{3}|[\da-f]{6})$/i.test(digits)) return undefined;

	return `#${(digits.length === 3 ? digits.replace(/./g, "$&$&") : digits).toLowerCase()}`;
}

/**
 * Encodes synchronously so it can finish within a form's submit event.
 * @param {HTMLImageElement} image
 * @param {number} x
 * @param {number} y
 * @param {number} side
 * @param {string} name
 */
function cropToFile(image, x, y, side, name) {
	const size = Math.min(Math.round(side), AVATAR_SIZE);

	const canvas = document.createElement("canvas");
	canvas.width = size;
	canvas.height = size;
	const context = canvas.getContext("2d");
	if (!context) return undefined;

	context.imageSmoothingQuality = "high";
	context.drawImage(image, x, y, side, side, 0, 0, size, size);

	// Browsers that can't encode WebP give PNG instead, which the server also accepts
	const [header, base64] = canvas.toDataURL("image/webp", 0.9).split(",");
	const type = header.slice("data:".length, header.indexOf(";"));
	const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0));

	return new File([bytes], name, { type });
}

/**
 * The address this tab last requested a sign-in link for, and when the Worker confirmed it, if it has.
 * Storage can be blocked or throw, which just leaves the button without its countdown.
 * @returns {{ address: string, sentAt?: number } | undefined}
 */
function readSignInEmail() {
	try {
		const stored = JSON.parse(sessionStorage.getItem(SIGN_IN_EMAIL_STORAGE_KEY) || "{}");
		if (typeof stored.address !== "string") return undefined;

		return { address: stored.address, sentAt: typeof stored.sentAt === "number" ? stored.sentAt : undefined };
	} catch {
		return undefined;
	}
}

/**
 * @param {{ address: string, sentAt?: number }} request
 */
function storeSignInEmail(request) {
	try {
		sessionStorage.setItem(SIGN_IN_EMAIL_STORAGE_KEY, JSON.stringify(request));
	} catch {
		// Without storage, the next page load simply shows no countdown
	}
}

function forgetSignInEmail() {
	try {
		sessionStorage.removeItem(SIGN_IN_EMAIL_STORAGE_KEY);
	} catch {
		// Storage that can't be reached has nothing to forget
	}
}
