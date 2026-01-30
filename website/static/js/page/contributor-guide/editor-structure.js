document.addEventListener("DOMContentLoaded", () => {
	document.querySelectorAll(".tree-node").forEach((toggle) => {
		toggle.addEventListener("click", (event) => {
			// Prevent link click from also toggling parent
			if (event.target instanceof HTMLElement && event.target.tagName === "A") return;

			const nestedList = toggle.parentElement?.querySelector(".nested");
			if (nestedList) {
				toggle.classList.toggle("expanded");
				nestedList.classList.toggle("active");
			}
		});
	});

	// Expand the first level by default
	const firstLevel = document.querySelector(".structure-outline > ul > li > .tree-node");
	if (firstLevel instanceof HTMLElement) firstLevel.click();
});
