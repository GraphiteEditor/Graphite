window.addEventListener("DOMContentLoaded", initializeFundraisingBar);

function initializeFundraisingBar() {
	const VISIBILITY_COVERAGE_FRACTION = 0.5;

	let loaded = false;

	const fundraising = document.querySelector("[data-fundraising]");
	const bar = fundraising.querySelector("[data-fundraising-bar]");
	const dynamicPercent = fundraising.querySelector("[data-fundraising-percent] [data-dynamic]")
	const dynamicGoal = fundraising.querySelector("[data-fundraising-goal] [data-dynamic]")
	if (!(fundraising instanceof HTMLElement && bar instanceof HTMLElement && dynamicPercent instanceof HTMLElement && dynamicGoal instanceof HTMLElement)) return;

	const setFundraisingGoal = async () => {
		const request = await fetch("https://graphite.rs/fundraising-goal");
		/** @type {{ percentComplete: number, targetValue: number }} */
		const data = await request.json();

		fundraising.classList.remove("loading");
		bar.style.setProperty("--fundraising-percent", `${data.percentComplete}%`);
		dynamicPercent.textContent = data.percentComplete;
		dynamicGoal.textContent = data.targetValue;

		loaded = true;
	};
	new IntersectionObserver((entries) => {
		entries.forEach((entry) => {
			if (!loaded && entry.intersectionRatio > VISIBILITY_COVERAGE_FRACTION) setFundraisingGoal();
		});
	}, { threshold: VISIBILITY_COVERAGE_FRACTION })
		.observe(fundraising);
}
