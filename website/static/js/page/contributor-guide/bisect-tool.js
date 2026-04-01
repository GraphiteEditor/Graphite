document.addEventListener("DOMContentLoaded", () => {
	const REPO = "GraphiteEditor/Graphite";
	const API = "https://api.github.com";

	// =========
	// API LAYER
	// =========

	const cache = new Map();
	let rateLimitRemaining = -1;
	let rateLimitReset = 0;

	async function fetchJSON(/** @type {string} */ url) {
		if (cache.has(url)) return cache.get(url);

		const response = await fetch(url);

		// Track rate limit
		const remaining = response.headers.get("X-RateLimit-Remaining");
		const reset = response.headers.get("X-RateLimit-Reset");
		if (remaining) rateLimitRemaining = parseInt(remaining);
		if (reset) rateLimitReset = parseInt(reset);
		updateRateLimitWarning();

		if (response.status === 404) {
			cache.set(url, undefined);
			return undefined;
		}

		if (response.status === 403) {
			const resetTime = rateLimitReset ? new Date(rateLimitReset * 1000).toLocaleTimeString() : undefined;
			const suffix = resetTime ? ` Resets at ${resetTime}.` : "";
			throw new Error(`GitHub API rate limit exceeded.${suffix}`);
		}

		if (!response.ok) {
			const body = await response.json().catch(() => undefined);
			throw new Error(body?.message || `GitHub API error: ${response.status} ${response.statusText}`);
		}

		const data = await response.json();
		cache.set(url, data);
		return data;
	}

	async function fetchCommitList(/** @type {string | undefined} */ since, /** @type {string | undefined} */ until, /** @type {number | undefined} */ page) {
		let url = `${API}/repos/${REPO}/commits?sha=master&per_page=100`;
		if (since) url += `&since=${since}`;
		if (until) url += `&until=${until}`;
		if (page && page > 1) url += `&page=${page}`;
		return fetchJSON(url);
	}

	async function fetchDeployUrl(/** @type {string} */ sha) {
		const comments = await fetchJSON(`${API}/repos/${REPO}/commits/${sha}/comments`);
		if (!comments || !Array.isArray(comments)) return undefined;

		// Find bot comments, use the last one
		const botComments = comments.filter((c) => c.user && c.user.login === "github-actions[bot]");
		if (botComments.length === 0) return undefined;

		const lastComment = botComments[botComments.length - 1];
		const match = lastComment.body.match(/\|\s*(https:\/\/[^\s|]+)\s*\|/);
		return match ? match[1] : undefined;
	}

	// ==============
	// DOM REFERENCES
	// ==============

	const tool = document.querySelector(".bisect-tool");
	if (!tool) return;

	const phases = {
		// eslint-disable-next-line quotes
		setup: tool.querySelector('[data-phase="setup"]'),
		// eslint-disable-next-line quotes
		bisect: tool.querySelector('[data-phase="bisect"]'),
	};

	const elements = {
		messageBox: tool.querySelector("[data-message-box]"),

		hashInput: tool.querySelector("[data-input='hash']"),
		dateInput: tool.querySelector("[data-input='date']"),
		commitHash: tool.querySelector("[data-commit-hash]"),
		commitDate: tool.querySelector("[data-commit-date]"),
		startButton: tool.querySelector("[data-start-button]"),

		stepLabel: tool.querySelector("[data-step-label]"),
		commitInfo: tool.querySelector("[data-commit-info]"),
		progressInfo: tool.querySelector("[data-progress-info]"),
		testBuildButton: tool.querySelector("[data-test-build-button]"),
		issuePresentButton: tool.querySelector("[data-issue-present-button]"),
		issueAbsentButton: tool.querySelector("[data-issue-absent-button]"),
		goBackButton: tool.querySelector("[data-go-back-button]"),
		findings: tool.querySelector(".findings"),
		bisectActions: tool.querySelector(".bisect-actions"),
	};

	// =====
	// STATE
	// =====

	/**
	 * @typedef {{ sha: string, date: Date, message: string }} Commit
	 * @typedef {{ goodIndex: number, badIndex: number, currentIndex: number, stepCount: number, bisectPhase: string, boundaryOffset: number, boundarySearching: boolean }} HistorySnapshot
	 */

	let mode = "regression"; // "regression" or "feature"
	let /** @type {Commit[]} */ commits = []; // Ordered oldest-first
	let goodIndex = -1; // Index where issue is absent (older side)
	let badIndex = -1; // Index where issue is present (newer side)
	let currentIndex = -1;
	let /** @type {string | undefined} */ currentDeployUrl;
	let stepCount = 0;
	let /** @type {HistorySnapshot[]} */ history = []; // Snapshots for undo
	let bisectPhase = "boundary"; // "boundary" or "binary"
	let boundaryOffset = 1; // For exponential boundary search
	let boundarySearching = false; // Whether we're in exponential backward search
	let startIndex = -1; // Where user started

	// =======
	// HELPERS
	// =======

	function commitToHtml(/** @type {Commit} */ commit) {
		const shortHash = (/** @type {string} */ sha) => sha.slice(0, 7);
		const commitUrl = (/** @type {string} */ sha) => `https://github.com/${REPO}/commit/${sha}`;

		const hash = `<a href="${commitUrl(commit.sha)}" target="_blank" rel="noopener">${shortHash(commit.sha)}</a>`;
		const date = commit.date.toISOString().slice(0, 10);
		const message = messageToHtml(commit.message);
		return `<strong>${hash}</strong> (${date}): ${message}`;
	}

	function messageToHtml(/** @type {string} */ message) {
		if (!message) return "";
		const escaped = message.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
		const prMatch = message.match(/\(#(\d+)\)$/);
		if (prMatch) return escaped.replace(`(#${prMatch[1]})`, `(<a href="https://github.com/${REPO}/pull/${prMatch[1]}" target="_blank" rel="noopener">#${prMatch[1]}</a>)`);
		return escaped;
	}

	function parseCommits(/** @type {any[]} */ apiCommits) {
		return apiCommits.map((/** @type {any} */ c) => ({
			sha: c.sha,
			date: new Date(c.commit.committer.date),
			message: c.commit.message.split("\n")[0],
		}));
	}

	function setDisabled(/** @type {Element | null} */ element, /** @type {boolean} */ disabled) {
		element?.classList.toggle("disabled", disabled);
	}

	function isDisabled(/** @type {Element | null} */ element) {
		return element?.classList.contains("disabled") ?? false;
	}

	function showPhase(/** @type {string} */ name) {
		Object.entries(phases).forEach(([key, phase]) => {
			phase?.classList.toggle("active", key === name);
		});
	}

	function showMessage(/** @type {string} */ html) {
		if (elements.messageBox) elements.messageBox.innerHTML = html;
		elements.messageBox?.classList.add("visible");
	}

	function hideMessage() {
		elements.messageBox?.classList.remove("visible");
	}

	function updateRateLimitWarning() {
		if (rateLimitRemaining >= 0 && rateLimitRemaining < 15) {
			const resetTime = rateLimitReset ? new Date(rateLimitReset * 1000).toLocaleTimeString() : "unknown";
			const plural = rateLimitRemaining === 1 ? "" : "s";
			showMessage(`<strong>API rate limit:</strong> ${rateLimitRemaining} request${plural} remaining. Resets at ${resetTime}.`);
		}
	}

	// ======================
	// COMMIT LIST MANAGEMENT
	// ======================

	async function loadCommitsAroundDate(/** @type {Date} */ targetDate) {
		const windowDays = 30;
		const since = new Date(targetDate.getTime() - windowDays * 24 * 60 * 60 * 1000).toISOString();
		const until = new Date(targetDate.getTime() + windowDays * 24 * 60 * 60 * 1000).toISOString();

		// Paginate to load all commits in the window (API returns max 100 per page)

		let /** @type {any[]} */ allRaw = [];
		let page = 1;

		while (true) {
			const raw = await fetchCommitList(since, until, page);
			if (!raw || raw.length === 0) break;
			allRaw = allRaw.concat(raw);
			if (raw.length < 100) break;
			page++;
		}

		if (allRaw.length === 0) {
			throw new Error("No commits found near that date. Try a different date.");
		}

		// GitHub returns newest-first, reverse to oldest-first
		const fetched = parseCommits(allRaw);
		fetched.reverse();

		commits = fetched;
	}

	async function extendCommitsBackward() {
		if (commits.length === 0) return false;

		const oldest = commits[0];
		const until = new Date(oldest.date.getTime() - 1000).toISOString();
		const raw = await fetchCommitList(undefined, until, undefined);
		if (!raw || raw.length === 0) return false;

		let fetched = parseCommits(raw);
		fetched.reverse();

		const existingShas = new Set(commits.map((c) => c.sha));
		fetched = fetched.filter((c) => !existingShas.has(c.sha));
		if (fetched.length === 0) return false;

		commits = [...fetched, ...commits];

		// Adjust indices to account for prepended commits
		const shift = fetched.length;
		if (goodIndex >= 0) goodIndex += shift;
		if (badIndex >= 0) badIndex += shift;
		if (currentIndex >= 0) currentIndex += shift;
		if (startIndex >= 0) startIndex += shift;

		return true;
	}

	function findCommitIndex(/** @type {string} */ sha) {
		return commits.findIndex((c) => c.sha.startsWith(sha) || sha.startsWith(c.sha));
	}

	// ============
	// BISECT LOGIC
	// ============

	function pushHistory() {
		history.push({
			goodIndex,
			badIndex,
			currentIndex,
			stepCount,
			bisectPhase,
			boundaryOffset,
			boundarySearching,
		});
		elements.goBackButton?.classList.remove("hidden");
	}

	function popHistory() {
		const snap = history.pop();
		if (!snap) return;
		goodIndex = snap.goodIndex;
		badIndex = snap.badIndex;
		currentIndex = snap.currentIndex;
		stepCount = snap.stepCount;
		bisectPhase = snap.bisectPhase;
		boundaryOffset = snap.boundaryOffset;
		boundarySearching = snap.boundarySearching;
		elements.goBackButton?.classList.remove("hidden");
	}

	async function presentCommit(/** @type {number} */ index) {
		currentIndex = index;
		const commit = commits[index];
		const deployUrl = await fetchDeployUrl(commit.sha);
		currentDeployUrl = deployUrl;

		if (elements.stepLabel) elements.stepLabel.innerHTML = `<strong>Bisect step ${stepCount + 1}</strong>`;
		if (elements.commitInfo) {
			elements.commitInfo.innerHTML = commitToHtml(commit);
		}

		if (goodIndex >= 0 && badIndex >= 0) {
			const remaining = badIndex - goodIndex;
			const stepsLeft = Math.max(1, Math.ceil(Math.log2(remaining)));
			if (elements.progressInfo) {
				elements.progressInfo.innerHTML = `<em>${remaining} commit${remaining === 1 ? "" : "s"} in range, ~${stepsLeft} step${stepsLeft === 1 ? "" : "s"} remaining</em>`;
			}
		} else {
			if (elements.progressInfo) elements.progressInfo.innerHTML = "<em>Locating starting point</em>";
		}

		if (deployUrl) {
			setDisabled(elements.testBuildButton, false);
			if (elements.testBuildButton) elements.testBuildButton.textContent = "Test this build";
		} else {
			setDisabled(elements.testBuildButton, true);
			if (elements.testBuildButton) elements.testBuildButton.textContent = "No build available";
		}

		// Set mode-specific button labels
		if (mode === "regression") {
			if (elements.issuePresentButton) elements.issuePresentButton.textContent = "Regression is present";
			if (elements.issueAbsentButton) elements.issueAbsentButton.textContent = "Regression is absent";
		} else {
			if (elements.issuePresentButton) elements.issuePresentButton.textContent = "Feature is present";
			if (elements.issueAbsentButton) elements.issueAbsentButton.textContent = "Feature is absent";
		}
	}

	async function handleUserResponse(/** @type {boolean} */ issuePresent) {
		pushHistory();
		stepCount++;

		if (bisectPhase === "boundary") {
			await handleBoundaryResponse(issuePresent);
			return;
		}

		// Binary search: narrow the range
		if (issuePresent) badIndex = currentIndex;
		else goodIndex = currentIndex;

		if (badIndex - goodIndex <= 1) showResult();
		else await doBinaryStep();
	}

	async function handleBoundaryResponse(/** @type {boolean} */ issuePresent) {
		// "present" means the feature/regression exists at this commit (bad/newer side)
		if (!boundarySearching) {
			// First step: user tested the starting commit
			if (issuePresent) {
				// Exists at starting commit, so it was introduced earlier. Search backward (doubling).
				badIndex = currentIndex;
				boundarySearching = true;
			} else {
				// Absent at starting commit. The newest commit should have it (user assumes master has it).
				goodIndex = currentIndex;
				badIndex = commits.length - 1;
				bisectPhase = "binary";
			}
		} else if (issuePresent) {
			badIndex = currentIndex;
			boundaryOffset *= 2;
		} else {
			goodIndex = currentIndex;
			bisectPhase = "binary";
		}

		if (bisectPhase === "binary") {
			await doBinaryStep();
			return;
		}

		// Continue boundary search backward
		await doBoundaryStep();
	}

	async function doBoundaryStep() {
		let targetIndex = startIndex - boundaryOffset;
		while (targetIndex < 0) {
			const extended = await extendCommitsBackward();
			if (!extended) {
				targetIndex = 0;
				break;
			}
			targetIndex = startIndex - boundaryOffset;
		}

		// If we've hit the oldest commit and it's still marked bad, we've exhausted history
		if (targetIndex <= 0 && badIndex === 0) {
			showResult();
			// Override the result message — we never confirmed a good baseline, so we can't pinpoint the introducing commit
			if (elements.progressInfo) {
				const label = mode === "regression" ? "regression" : "feature";
				elements.progressInfo.innerHTML = `<em>The ${label} was already present in the oldest available commit</em>`;
			}
			return;
		}

		await presentCommit(targetIndex);
	}

	async function doBinaryStep() {
		const mid = Math.floor((goodIndex + badIndex) / 2);

		// Try to find a testable commit near the midpoint
		let testIndex = mid;
		let offset = 0;
		while (testIndex > goodIndex && testIndex < badIndex) {
			const url = await fetchDeployUrl(commits[testIndex].sha);
			if (url) break;
			// Try alternating sides
			offset++;
			if (offset % 2 === 1) testIndex = mid + Math.ceil(offset / 2);
			else testIndex = mid - Math.ceil(offset / 2);
		}

		// If no testable commit found in range, show result as a range
		if (testIndex <= goodIndex || testIndex >= badIndex) {
			showResult();
			return;
		}

		await presentCommit(testIndex);
	}

	function showResult() {
		const heading = "Bisect complete";

		// Hide interactive elements, keep the bisect phase visible
		if (elements.progressInfo) elements.progressInfo.innerHTML = "";
		setDisabled(elements.testBuildButton, true);
		if (elements.testBuildButton instanceof HTMLElement) elements.testBuildButton.style.display = "none";
		if (elements.findings instanceof HTMLElement) elements.findings.style.display = "none";
		if (elements.bisectActions instanceof HTMLElement) elements.bisectActions.style.display = "none";
		if (history.length > 0) elements.goBackButton?.classList.remove("hidden");

		const label = mode === "regression" ? "regression" : "feature";
		const single = badIndex - goodIndex <= 1;

		if (elements.stepLabel) elements.stepLabel.innerHTML = `<strong>${heading}</strong>`;
		if (elements.progressInfo) {
			elements.progressInfo.innerHTML = single
				? `<em>The ${label} was introduced in the following commit</em>`
				: `<em>The ${label} was introduced in one of the following commits (not all have build links)</em>`;
		}

		const start = single ? badIndex : goodIndex + 1;
		let html = "";
		for (let i = start; i <= badIndex; i++) {
			const c = commits[i];
			html += single ? `${commitToHtml(c)}` : `<div>${commitToHtml(c)}</div>`;
		}
		if (elements.commitInfo) elements.commitInfo.innerHTML = html;
	}

	// ==============
	// EVENT HANDLERS
	// ==============

	// Toggle start input visibility
	function syncStartInputVisibility() {
		// eslint-disable-next-line quotes
		const selected = tool?.querySelector('input[name="start-method"]:checked');
		const method = selected instanceof HTMLInputElement ? selected.value : "date";
		elements.hashInput?.classList.toggle("hidden", method !== "hash");
		elements.dateInput?.classList.toggle("hidden", method !== "date");
	}
	syncStartInputVisibility();
	// eslint-disable-next-line quotes
	tool.querySelectorAll('input[name="start-method"]').forEach((radio) => {
		radio.addEventListener("change", syncStartInputVisibility);
	});

	// Start bisect
	elements.startButton?.addEventListener("click", async () => {
		if (isDisabled(elements.startButton)) return;
		hideMessage();
		// eslint-disable-next-line quotes
		const modeInput = tool.querySelector('input[name="bisect-mode"]:checked');
		// eslint-disable-next-line quotes
		const methodInput = tool.querySelector('input[name="start-method"]:checked');
		if (!(modeInput instanceof HTMLInputElement) || !(methodInput instanceof HTMLInputElement)) return;
		mode = modeInput.value;
		const method = methodInput.value;

		try {
			setDisabled(elements.startButton, true);

			if (method === "hash") {
				const hash = elements.commitHash instanceof HTMLInputElement ? elements.commitHash.value.trim() : "";
				if (!/^[0-9a-fA-F]{7,40}$/.test(hash)) {
					throw new Error("Please enter a valid commit hash (7-40 hex characters).");
				}

				// Fetch the commit to get its date
				const commitData = await fetchJSON(`${API}/repos/${REPO}/commits/${hash}`);
				if (!commitData) {
					throw new Error("Commit not found. Check the hash and try again.");
				}

				const commitDate = new Date(commitData.commit.committer.date);
				await loadCommitsAroundDate(commitDate);

				startIndex = findCommitIndex(hash);
				if (startIndex < 0) {
					throw new Error("Commit not found in the master branch history.");
				}
			} else {
				const dateStr = elements.commitDate instanceof HTMLInputElement ? elements.commitDate.value : "";
				if (!dateStr) {
					throw new Error("Please select a date.");
				}

				const date = new Date(dateStr + "T12:00:00Z");
				if (date > new Date()) {
					throw new Error("Date cannot be in the future.");
				}

				await loadCommitsAroundDate(date);

				// Find the commit closest to the selected date
				let closestIndex = 0;
				let closestDiff = Infinity;
				for (let i = 0; i < commits.length; i++) {
					const diff = Math.abs(commits[i].date.getTime() - date.getTime());
					if (diff < closestDiff) {
						closestDiff = diff;
						closestIndex = i;
					}
				}
				startIndex = closestIndex;
			}

			// Reset state
			goodIndex = -1;
			badIndex = -1;
			currentIndex = -1;
			currentDeployUrl = undefined;
			stepCount = 0;
			history = [];
			bisectPhase = "boundary";
			boundaryOffset = 1;
			boundarySearching = false;
			elements.goBackButton?.classList.remove("hidden");
			if (elements.testBuildButton instanceof HTMLElement) elements.testBuildButton.style.display = "";
			if (elements.findings instanceof HTMLElement) elements.findings.style.display = "";
			if (elements.bisectActions instanceof HTMLElement) elements.bisectActions.style.display = "";

			// Show bisect phase and present the starting commit
			showPhase("bisect");
			await presentCommit(startIndex);
		} catch (err) {
			if (err instanceof Error) showMessage(err.message);
		} finally {
			setDisabled(elements.startButton, false);
		}
	});

	// Test build button
	elements.testBuildButton?.addEventListener("click", () => {
		if (isDisabled(elements.testBuildButton)) return;
		if (currentDeployUrl) {
			window.open(currentDeployUrl, "_blank", "noopener");
		}
	});

	// Issue response buttons
	function onIssueResponse(/** @type {Element | null} */ button, /** @type {boolean} */ issuePresent) {
		button?.addEventListener("click", async () => {
			if (isDisabled(button)) return;
			hideMessage();
			try {
				await handleUserResponse(issuePresent);
			} catch (err) {
				if (err instanceof Error) showMessage(err.message);
			}
		});
	}
	onIssueResponse(elements.issuePresentButton, true);
	onIssueResponse(elements.issueAbsentButton, false);

	// Go back
	elements.goBackButton?.querySelector("a")?.addEventListener("click", async () => {
		hideMessage();

		if (history.length === 0) {
			showPhase("setup");
			return;
		}

		// Restore interactive elements that may have been hidden by showResult
		if (elements.testBuildButton instanceof HTMLElement) elements.testBuildButton.style.display = "";
		if (elements.findings instanceof HTMLElement) elements.findings.style.display = "";
		if (elements.bisectActions instanceof HTMLElement) elements.bisectActions.style.display = "";
		popHistory();
		await presentCommit(currentIndex);
	});
});
