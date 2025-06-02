const VISIBILITY_COVERAGE_FRACTION = 0.25;

window.addEventListener("DOMContentLoaded", () => {
	const players = document.querySelectorAll("[data-auto-play]");
	players.forEach((player) => {
		if (!(player instanceof HTMLVideoElement)) return;

		let loaded = false;

		new IntersectionObserver((entries) => {
			entries.forEach((entry) => {
				if (!loaded && entry.intersectionRatio > VISIBILITY_COVERAGE_FRACTION) {
					player.removeAttribute("preload");
					player.setAttribute("autoplay", "");

					loaded = true;
				};
			});
		}, { threshold: VISIBILITY_COVERAGE_FRACTION })
		.observe(player);
	});
});
