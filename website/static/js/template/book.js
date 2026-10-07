addEventListener("DOMContentLoaded", trackScrollHeadingInTOC);
addEventListener("DOMContentLoaded", listenForClickToOpenOrCloseTOC);

// Listen for scroll events and update the active section in the table of contents to match the visible content's heading
function trackScrollHeadingInTOC() {
	let /** @type {HTMLElement | undefined} */ activeTocHeading;

	const updateVisibleHeading = () => {
		const content = Array.from(document.querySelectorAll("[data-article] > *"));

		// Find the first element in `content` that is visible in the top of the viewport
		let firstVisible = content.find((element) => element.getBoundingClientRect().bottom >= 0);

		// Find the next heading
		let heading = firstVisible;
		while (heading && !heading.tagName.toLowerCase().match(/^h[1-6]$/)) {
			if (!heading.nextElementSibling) break;
			heading = heading.nextElementSibling;
		}

		// If the next heading isn't fully visible, use the previous heading
		if (heading && heading.getBoundingClientRect().bottom > window.innerHeight) {
			let prevHeading = firstVisible;
			while (prevHeading && !prevHeading.tagName.toLowerCase().match(/^h[1-6]$/)) {
				if (!prevHeading.previousElementSibling) break;
				prevHeading = prevHeading.previousElementSibling;
			}

			if (prevHeading && prevHeading.tagName.toLowerCase().match(/^h[1-6]$/)) heading = prevHeading;
		}

		// If the heading isn't an h1-h6, use the last heading
		if (!heading || !heading.tagName.toLowerCase().match(/^h[1-6]$/)) {
			const filtered = content.filter((element) => element.tagName.toLowerCase().match(/^h[1-6]$/));
			heading = filtered[filtered.length - 1];
		}

		// If there is no heading, use the first heading
		if (!heading) heading = content.find((element) => element.tagName.toLowerCase() === "h1");

		// Remove the existing active heading
		activeTocHeading?.classList.remove("active");
		activeTocHeading = undefined;

		// Exit if there are no headings
		if (!heading) return;

		// Set the new active heading
		const tocHeading = document.querySelector(`[data-contents-link="${heading.id}"]`)?.parentElement;
		if (tocHeading instanceof HTMLElement) {
			tocHeading.classList.add("active");
			activeTocHeading = tocHeading;
		}
	};

	addEventListener("scroll", updateVisibleHeading);
	updateVisibleHeading();
}

function listenForClickToOpenOrCloseTOC() {
	// Open the chapter selection if the user clicks the open button
	document.querySelector("[data-open-chapter-selection]")?.addEventListener("click", () => {
		// Wait until after the click-outside-the-panel event has been handled before opening the panel so it doesn't immediately get closed in the same call stack
		setTimeout(() => {
			document.querySelector("[data-chapters]")?.classList.add("open");
		});
	});

	// Close the chapter selection if the user clicks the close button
	document.querySelector("[data-close-chapter-selection]")?.addEventListener("click", () => {
		document.querySelector("[data-chapters]")?.classList.remove("open");
	});

	// Close the chapter selection if the user clicks outside of it
	document.querySelector("[data-main]")?.addEventListener("click", (e) => {
		const chapters = document.querySelector("[data-chapters]");
		if (chapters?.classList.contains("open") && e.target instanceof HTMLElement && !e.target.closest("[data-chapters]")) {
			chapters.classList.remove("open");
		}
	});
}
