window.addEventListener("DOMContentLoaded", () => {
	const amountField = /** @type {HTMLInputElement | null} */ (document.querySelector("[data-donation-amount]"));
	const presetButtons = /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-amount]"));
	const githubOneTime = /** @type {HTMLAnchorElement | null} */ (document.querySelector("[data-github-one-time]"));
	if (!amountField || presetButtons.length === 0) return;

	const syncAmount = () => {
		presetButtons.forEach((button) => button.setAttribute("aria-pressed", String(button.dataset.amount === amountField.value)));

		if (!githubOneTime) return;

		// GitHub accepts only whole-dollar amounts
		const amount = Math.round(Number(amountField.value));
		const suffix = Number.isFinite(amount) && amount > 0 ? `&amount=${amount}` : "";
		githubOneTime.href = `https://github.com/sponsors/GraphiteEditor/sponsorships?frequency=one-time${suffix}`;
	};

	presetButtons.forEach((button) => {
		button.addEventListener("click", () => {
			amountField.value = button.dataset.amount || "";
			syncAmount();
		});
	});
	amountField.addEventListener("input", syncAmount);

	syncAmount();
});
