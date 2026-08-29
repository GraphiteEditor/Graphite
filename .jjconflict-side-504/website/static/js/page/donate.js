const statisticsRequest = fetch("https://graphite.art/donate/sponsorship-stats").then((response) => response.json());

window.addEventListener("DOMContentLoaded", () => {
	setUpAmountPresets();
	showStatistics();
});

function setUpAmountPresets() {
	const amountField = /** @type {HTMLInputElement | null} */ (document.querySelector("[data-donation-amount]"));
	const presetButtons = /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-amount]"));
	const githubOneTime = /** @type {HTMLAnchorElement | null} */ (document.querySelector("[data-github-one-time]"));
	if (!amountField || presetButtons.length === 0) return;

	const syncAmount = () => {
		presetButtons.forEach((button) => button.setAttribute("aria-pressed", String(button.dataset.amount === amountField.value)));

		if (!githubOneTime) return;

		const amount = Number(amountField.value);
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
}

async function showStatistics() {
	const element = /** @type {HTMLElement | null} */ (document.querySelector("[data-statistics]"));
	const recurringElement = /** @type {HTMLElement | null} */ (document.querySelector("[data-statistics-recurring]"));
	const membersElement = /** @type {HTMLElement | null} */ (document.querySelector("[data-statistics-members]"));
	const oneTimeElement = /** @type {HTMLElement | null} */ (document.querySelector("[data-statistics-one-time]"));
	const donorsElement = /** @type {HTMLElement | null} */ (document.querySelector("[data-statistics-donors]"));
	if (!element || !recurringElement || !membersElement || !oneTimeElement || !donorsElement) return;

	try {
		const json = await statisticsRequest;
		if (!json || !json.recurring || !json.one_time_prior_12_month_sum) throw new Error();

		const number = (/** @type {number} */ value) => Math.round(value).toLocaleString("en-US", { useGrouping: "min2" });
		const dollars = (/** @type {number} */ value) => `$${number(value)}`;

		recurringElement.innerText = dollars(parseInt(json.recurring.cents) / 100);
		membersElement.innerText = number(json.recurring.count);
		oneTimeElement.innerText = dollars(parseInt(json.one_time_prior_12_month_sum.cents) / 100);
		donorsElement.innerText = number(json.one_time_prior_12_month_sum.count);

		// Force repaint to work around Safari bug <https://bugs.webkit.org/show_bug.cgi?id=286403> (remove this and its data attribute when the bug is fixed and widely deployed)
		element.style.transform = "scale(1)";
	} catch {
		element.remove();
	}
}
