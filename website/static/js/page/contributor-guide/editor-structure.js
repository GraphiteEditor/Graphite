document.addEventListener("DOMContentLoaded", () => {
	document.querySelectorAll("[data-tree-node]").forEach((toggle) => {
		toggle.addEventListener("click", (event) => {
			// Prevent link click from also toggling parent
			if (event.target instanceof HTMLElement && event.target.tagName.toLowerCase() === "a") return;

			const nestedList = toggle.parentElement?.querySelector("[data-nested]");
			if (nestedList) {
				toggle.classList.toggle("expanded");
				nestedList.classList.toggle("active");
			}
		});
	});

	// Expand the first level by default
	const firstLevel = document.querySelector("[data-structure-outline] [data-tree-node]");
	if (firstLevel instanceof HTMLElement) firstLevel.click();
});
