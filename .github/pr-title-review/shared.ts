// Shared by the PR title review's three steps: `gather.ts` and `report.ts`, which use the GitHub token,
// and `review.ts`, which holds the review service's API key without a GitHub token

import fs from "fs";
import path from "path";

// Problems the review may name in a title, each shown in the comment exactly as written here, since it can't add wording of its own
export const ISSUES = {
	verb_not_imperative: 'It should begin with an imperative verb like "Add" or "Fix", rather than past tense ("Added"), third person ("Adds"), or a noun phrase.',
	verb_is_user_action: 'Its opening verb should say what the change does to the software, rather than something a user does (like "Drag").',
	verb_is_system_behavior: 'Its opening verb should say what the change does to the software, rather than something the software itself does or becomes able to do (like "Render").',
	fix_without_fix_verb: 'As a bug fix, it should usually begin with "Fix", followed by the broken behavior or by what was broken and how it now behaves.',
	new_node_form: "As a PR that mainly adds nodes, it should take the form \"New node: 'Name'\", or \"New nodes: 'X', 'Y', and 'Z'\" for several.",
	node_name_unquoted: "Node names should be in single quotes, like 'Combine Paths'.",
	too_long: "It's too long to read as one headline. Most titles are 40 to 110 characters.",
	too_vague: "It's too vague to tell what the change does.",
	lists_changes: "It strings together too many unrelated changes to read as a headline.",
	describes_mechanism: "It describes how the change works, rather than what users get from it.",
	inaccurate: "It claims something the changes in this PR don't do.",
	formatting: 'It should be in sentence case without a trailing period, and without prefixes like "feat:", issue numbers, or emoji.',
	spelling: "It has a misspelling or a non-American English spelling.",
} as const;
export type IssueCode = keyof typeof ISSUES;

// The "still_approved" and "unchanged" outcomes keep the verdict of an earlier run for the same title, which passed or failed.
// An "overridden" outcome passes a title that the review would have rejected, since one of the repository's admins set it.
export const OUTCOMES = ["approved", "still_approved", "overridden", "rejected", "limited", "error", "unchanged", "superseded", "unavailable"] as const;
export type Outcome = (typeof OUTCOMES)[number];

// Outcomes that say nothing lasting about the title, so the next run for the same title tries again rather than carrying the failure over.
// An "unavailable" review failed before anything was paid for, unlike an "error".
export const RETRIED_OUTCOMES: readonly Outcome[] = ["superseded", "unavailable"];
export const PASSING_OUTCOMES: readonly Outcome[] = ["approved", "still_approved", "overridden"];

export type Decision = {
	outcome: Outcome;
	issues: IssueCode[];
	suggestions: string[];
	// For a limited outcome, when a title edit can be reviewed again (in milliseconds since the Unix epoch), or 0 if unknown
	retryAfter: number;
};

// What `gather.ts` hands to `review.ts`: either a decision already made without a review, or the messages to review.
// The suggestions message, without the PR's changes, compares the title with the previous review's suggestions when there are any.
export type TitleReview = { title: string; userMessage: string; suggestionsMessage?: string; passesRegardless?: boolean };
export type ReviewRequest = { decision: Decision } | TitleReview;

export type PullRequestEvent = {
	action: string;
	// The fields that an "edited" action changed
	changes?: { title?: { from: string } };
	// Whoever opened, retitled, or pushed to the PR
	sender: { login: string };
	pull_request: {
		number: number;
		title: string;
		user: { login: string };
		author_association: string;
		changed_files: number;
		head: { ref: string };
		base: { repo: { default_branch: string } };
	};
};

export type IssueComment = { id: number; body?: string; user: { login: string; type: string } | null };

export const COMMENT_MARKER = "<!-- pr-title-review -->";

// Longer suggestions are dropped, since titles stay under this
const MAX_SUGGESTION_LENGTH = 140;

export function requireEnvironment(name: string): string {
	const value = process.env[name];
	if (!value) throw new Error(`The ${name} environment variable is missing`);
	return value;
}

export function readEvent(): PullRequestEvent {
	return JSON.parse(fs.readFileSync(requireEnvironment("GITHUB_EVENT_PATH"), "utf8"));
}

// Files passed between the steps, in the runner's temporary directory, which the PR's content never reaches as code
export const REQUEST_PATH = path.join(requireEnvironment("RUNNER_TEMP"), "pr-title-review-request.json");
export const DECISION_PATH = path.join(requireEnvironment("RUNNER_TEMP"), "pr-title-review-decision.json");

export function makeDecision(outcome: Outcome, details?: Partial<Decision>): Decision {
	return { outcome, issues: [], suggestions: [], retryAfter: 0, ...details };
}

export async function github<T>(method: string, path: string, body?: unknown): Promise<T> {
	// Only reads are retried, since a repeated write could post a comment twice
	const attempts = method === "GET" ? 3 : 1;

	for (let attempt = 1; ; attempt++) {
		const response = await fetch(`${process.env.GITHUB_API_URL || "https://api.github.com"}${path}`, {
			method,
			headers: {
				Accept: "application/vnd.github+json",
				Authorization: `Bearer ${requireEnvironment("GITHUB_TOKEN")}`,
				"X-GitHub-Api-Version": "2022-11-28",
				"User-Agent": "graphite-pr-title-review",
			},
			body: body === undefined ? undefined : JSON.stringify(body),
		}).catch(() => undefined);

		if (response?.status === 204) return undefined as T;
		if (response?.ok) return (await response.json()) as T;

		const temporary = !response || response.status >= 500;
		if (!temporary || attempt >= attempts) throw new Error(`GitHub's API ${response ? `responded with status ${response.status}` : "couldn't be reached"} for ${method} ${path.split("?")[0]}`);

		await new Promise((resolve) => setTimeout(resolve, 2000 * attempt));
	}
}

export async function githubPaginate<T>(path: string, maxPages: number): Promise<T[]> {
	const separator = path.includes("?") ? "&" : "?";
	const items: T[] = [];

	for (let page = 1; page <= maxPages; page++) {
		const pageItems = await github<T[]>("GET", `${path}${separator}per_page=100&page=${page}`);
		items.push(...pageItems);
		if (pageItems.length < 100) break;
	}

	return items;
}

export function sanitizeIssues(issues: unknown): IssueCode[] {
	if (!Array.isArray(issues)) return [];

	const known = issues.filter((issue): issue is IssueCode => typeof issue === "string" && Object.hasOwn(ISSUES, issue));
	return [...new Set(known)];
}

// Keeps up to three suggested titles that each read as one plain headline, without links or mentions
export function sanitizeSuggestions(suggestions: unknown, currentTitle?: string): string[] {
	if (!Array.isArray(suggestions)) return [];

	const cleaned = suggestions
		.filter((suggestion): suggestion is string => typeof suggestion === "string")
		// Control and invisible formatting characters (like bidirectional overrides) become spaces before whitespace is collapsed
		.map((suggestion) =>
			suggestion
				.replace(/[\p{Cc}\p{Cf}]/gu, " ")
				.replace(/\s+/g, " ")
				.trim(),
		)
		.filter((suggestion) => suggestion.length >= 10 && suggestion.length <= MAX_SUGGESTION_LENGTH)
		.filter((suggestion) => /^[\p{Lu}'"]/u.test(suggestion) && !/[.!?]\s+\p{Lu}/u.test(suggestion) && !/:\/\/|www\.|@/i.test(suggestion))
		.filter((suggestion) => suggestion !== currentTitle?.trim());

	return [...new Set(cleaned)].slice(0, 3);
}

// Validates a decision read from a file, since its issues and suggestions came from the review's response
export function parseDecision(value: unknown): Decision | undefined {
	if (typeof value !== "object" || value === null) return undefined;

	const { outcome, issues, suggestions, retryAfter } = value as Record<string, unknown>;
	if (typeof outcome !== "string" || !(OUTCOMES as readonly string[]).includes(outcome)) return undefined;

	return {
		outcome: outcome as Outcome,
		issues: sanitizeIssues(issues),
		suggestions: sanitizeSuggestions(suggestions),
		retryAfter: typeof retryAfter === "number" && Number.isFinite(retryAfter) ? retryAfter : 0,
	};
}

// This workflow's comments on a PR, oldest first
export async function listReviewComments(repository: string, pullRequestNumber: number): Promise<IssueComment[]> {
	const comments = await githubPaginate<IssueComment>(`/repos/${repository}/issues/${pullRequestNumber}/comments`, 30);
	return comments.filter(isReviewComment);
}

// Only comments by GitHub Actions count, since anyone could post one that imitates the marker
function isReviewComment(comment: IssueComment): boolean {
	return comment.user?.login === "github-actions[bot]" && comment.user.type === "Bot" && comment.body?.startsWith(COMMENT_MARKER) === true;
}

// Nothing in a code block renders as a link, mention, image, or HTML, and readers get a copy button
export function codeBlock(text: string): string {
	const backtickRuns = text.match(/`+/g) || [];
	const longestRun = Math.max(0, ...backtickRuns.map((run) => run.length));
	const fence = "`".repeat(Math.max(3, longestRun + 1));

	return `${fence}text\n${text}\n${fence}`;
}

// The suggested titles in a review comment, from the code blocks that `codeBlock` writes
export function readSuggestions(commentBody: string): string[] {
	const blocks = commentBody.replace(/\r\n/g, "\n").matchAll(/^(`{3,})text\n(.*)\n\1$/gm);
	return sanitizeSuggestions([...blocks].map((match) => match[2]));
}

// Makes a message safe to print as one log line, where a line break followed by `::` would start a workflow command
export function singleLine(message: string): string {
	return message.replace(/\s+/g, " ").trim();
}

export function describeError(error: unknown): string {
	return singleLine(error instanceof Error ? error.message : String(error));
}
