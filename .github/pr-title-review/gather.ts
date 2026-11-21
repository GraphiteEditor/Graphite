// Decides whether the PR's title needs a fresh review, carrying over the previous verdict or applying the rate limits when it doesn't,
// and gathers what the review reads: the title, the main branch's recent titles, and the PR's changed files and diff

import crypto from "crypto";
import fs from "fs";
import {
	type ReviewRequest,
	type TitleReview,
	REQUEST_PATH,
	describeError,
	github,
	githubPaginate,
	listReviewComments,
	makeDecision,
	readEvent,
	readSuggestions,
	requireEnvironment,
	singleLine,
} from "./shared.ts";

// Reviews allowed per PR before they slow down, and before they stop for the day, plus a daily limit for each outside contributor across their PRs
const MINUTE = 60_000;
const BURST_WINDOW = 5 * MINUTE;
const BURST_LIMIT = 5;
const DAILY_WINDOW = 24 * 60 * MINUTE;
const DAILY_LIMIT = 20;
const DAILY_LIMIT_PER_CONTRIBUTOR = 40;
// Beyond this many pages of runs in a day, reviews stop rather than miscount from a partial history
const MAX_RUN_PAGES = 10;
// The workflow step that fails only when a run reached a verdict against its title, by the end of its name
const VERDICT_STEP_NAME = "Fail for the title's verdict";
const MAX_VERDICT_LOOKUPS = 5;

// How much of the PR's changes accompany the title, which keeps the message under about 20,000 tokens
const FILE_LIST_CHARACTER_BUDGET = 12_000;
const DIFF_CHARACTER_BUDGET = 40_000;
const FILE_DIFF_CHARACTER_LIMIT = 8_000;
const NAME_CHARACTER_LIMIT = 150;
const RECENT_TITLES_COUNT = 40;
// Files whose diffs say little about the change, so they're listed without an excerpt
const UNINFORMATIVE_FILES = /(^|\/)(Cargo\.lock|package-lock\.json|third-party-licenses\.txt)$|\.(svg|graphite)$/;

type WorkflowRun = { id: number; workflow_id: number; display_title: string; conclusion: string | null; created_at: string; actor: { login: string } | null };
type ChangedFile = { filename: string; previous_filename?: string; status: string; additions: number; deletions: number; patch?: string };

const repository = requireEnvironment("GITHUB_REPOSITORY");
const event = readEvent();
const pullRequest = event.pull_request;

// Org members skip the limits, on their own PRs and when retitling someone else's (which takes the team's access)
const byTeam = pullRequest.author_association === "MEMBER" || event.sender.login !== pullRequest.user.login;

let request: ReviewRequest;
try {
	request = await gather();
} catch (error) {
	console.log(`::error::Gathering the title's history and changes failed: ${describeError(error)}`);
	request = { decision: makeDecision("unavailable") };
}

fs.writeFileSync(REQUEST_PATH, JSON.stringify(request));

async function gather(): Promise<ReviewRequest> {
	const currentRun = await github<WorkflowRun>("GET", `/repos/${repository}/actions/runs/${requireEnvironment("GITHUB_RUN_ID")}`);
	const now = Date.now();

	if (await titleHasChanged()) return { decision: makeDecision("superseded") };

	// A title set by one of the repository's admins always passes, and is reviewed only to offer suggestions
	const setsTitle = event.action === "opened" || (event.action === "edited" && event.changes?.title !== undefined);
	if (setsTitle && (await senderIsAdmin())) return { ...(await buildTitleReview([])), passesRegardless: true };

	// A maintainer re-running the check forces a fresh review, outside the limits
	const forced = Number(process.env.GITHUB_RUN_ATTEMPT) > 1;
	const unlimited = forced || byTeam;

	const recentRuns = await listRecentRuns(currentRun.workflow_id, now - DAILY_WINDOW);
	if (!recentRuns && !unlimited) {
		console.log("::warning::This workflow has run too many times today to read its history in full");
		return { decision: makeDecision("unavailable") };
	}

	// Without the day's full history, the PR's own runs are found by its branch
	const isOwnRun = (run: WorkflowRun) => pullRequestNumber(run) === pullRequest.number && run.id !== currentRun.id;
	const ownRuns = (recentRuns || (await listBranchRuns(currentRun.workflow_id))).filter(isOwnRun);
	const earlierRuns = recentRuns && !ownRuns.some(isFinished) ? (await listBranchRuns(currentRun.workflow_id, now - DAILY_WINDOW)).filter(isOwnRun) : [];
	const earlierFinishedRun = earlierRuns.find(isFinished);

	// Carry over the previous verdict while the title is unchanged, as when commits are pushed or the description is edited
	const previousRun = forced ? undefined : await latestVerdictRun([...newestFirst(ownRuns), ...earlierRuns].filter(isFinished));
	if (previousRun?.display_title === currentRun.display_title) {
		console.log(`Carrying over the previous run's verdict (${previousRun.conclusion}) for the same title`);
		return { decision: makeDecision(previousRun.conclusion === "success" ? "still_approved" : "unchanged") };
	}

	// Taking one of the previous review's suggested titles as is costs nothing to approve, so it isn't held back by the limits
	const previousSuggestions = forced ? [] : await readPreviousSuggestions();
	if (previousSuggestions.includes(singleLine(pullRequest.title))) {
		console.log("Approving the title, since the previous review suggested it");
		return { decision: makeDecision("approved") };
	}

	if (!unlimited && recentRuns) {
		// Only the author's own reviews count, since the team's retitles of the PR aren't limited
		const reviewTimes = [...reviewRuns(ownRuns, earlierFinishedRun).filter(isByAuthor).map(createdTime), now].filter((time) => now - time < DAILY_WINDOW);
		if (reviewTimes.length > DAILY_LIMIT) return limited(reviewTimes, DAILY_LIMIT, previousSuggestions);

		const contributorTimes = [...contributorReviewRuns(recentRuns, currentRun).map(createdTime), now];
		if (contributorTimes.length > DAILY_LIMIT_PER_CONTRIBUTOR) return limited(contributorTimes, DAILY_LIMIT_PER_CONTRIBUTOR, previousSuggestions);

		// Slow down rapid title changes by waiting for the oldest recent ones to age out (a newer change's run queues behind this one)
		const recentReviewTimes = reviewTimes.filter((time) => now - time < BURST_WINDOW);
		if (recentReviewTimes.length > BURST_LIMIT) {
			const wait = recentReviewTimes[recentReviewTimes.length - BURST_LIMIT - 1] + BURST_WINDOW - now;
			console.log(`Waiting ${Math.ceil(wait / 1000)} seconds, since the title changed more than ${BURST_LIMIT} times in ${BURST_WINDOW / MINUTE} minutes`);
			await new Promise((resolve) => setTimeout(resolve, wait));

			if (await titleHasChanged()) return { decision: makeDecision("superseded") };
		}
	}

	return buildTitleReview(previousSuggestions);
}

async function senderIsAdmin(): Promise<boolean> {
	const path = `/repos/${repository}/collaborators/${encodeURIComponent(event.sender.login)}/permission`;
	const { permission } = await github<{ permission?: string }>("GET", path).catch(() => ({ permission: undefined }));
	return permission === "admin";
}

// The suggested titles in the PR's latest review comment, if it has any
async function readPreviousSuggestions(): Promise<string[]> {
	const comments = await listReviewComments(repository, pullRequest.number);
	return readSuggestions(comments[comments.length - 1]?.body || "");
}

// A newer title than this event's means a later run reviews it, and carrying over a verdict here would credit it to the wrong title
async function titleHasChanged(): Promise<boolean> {
	const livePullRequest = await github<{ title: string }>("GET", `/repos/${repository}/pulls/${pullRequest.number}`);
	return livePullRequest.title !== pullRequest.title;
}

// The previous suggestions are repeated in the limit's comment, so taking one as is still gets approved
function limited(reviewTimes: number[], limit: number, suggestions: string[]): ReviewRequest {
	// A title edit can be reviewed once enough earlier reviews leave the window to make room for it
	const oldestFirst = [...reviewTimes].sort((a, b) => a - b);
	const retryAfter = oldestFirst[oldestFirst.length - limit] + DAILY_WINDOW;
	return { decision: makeDecision("limited", { retryAfter, suggestions }) };
}

// Every run of this workflow since the given time, across all PRs, or undefined if there are too many to read
async function listRecentRuns(workflowId: number, since: number): Promise<WorkflowRun[] | undefined> {
	const created = encodeURIComponent(`>=${new Date(since).toISOString()}`);
	const query = `/repos/${repository}/actions/workflows/${workflowId}/runs?event=pull_request_target&created=${created}&per_page=100`;
	const runs: WorkflowRun[] = [];

	for (let page = 1; page <= MAX_RUN_PAGES; page++) {
		const { workflow_runs: pageRuns } = await github<{ workflow_runs: WorkflowRun[] }>("GET", `${query}&page=${page}`);
		runs.push(...pageRuns);

		// Runs created while paging shift the later pages, which can repeat a run
		if (pageRuns.length < 100) return [...new Map(runs.map((run) => [run.id, run])).values()];
	}

	return undefined;
}

// The latest runs for the PR's branch name, newest first, optionally from before a time. This is cheap and may only miss runs from before a branch rename.
async function listBranchRuns(workflowId: number, before?: number): Promise<WorkflowRun[]> {
	const created = before ? `&created=${encodeURIComponent(`<${new Date(before).toISOString()}`)}` : "";
	const branch = encodeURIComponent(pullRequest.head.ref);
	const query = `/repos/${repository}/actions/workflows/${workflowId}/runs?event=pull_request_target&branch=${branch}${created}&per_page=100`;

	const { workflow_runs: runs } = await github<{ workflow_runs: WorkflowRun[] }>("GET", query);
	return newestFirst(runs);
}

// The newest of the finished runs that reached a verdict on its title, skipping failures that didn't (such as when GitHub or the review service couldn't be reached)
async function latestVerdictRun(finishedRunsNewestFirst: WorkflowRun[]): Promise<WorkflowRun | undefined> {
	for (let i = 0; i < Math.min(finishedRunsNewestFirst.length, MAX_VERDICT_LOOKUPS); i++) {
		const run = finishedRunsNewestFirst[i];
		if (run.conclusion === "success") return run;

		const { jobs } = await github<{ jobs: { steps?: { name: string; conclusion: string | null }[] }[] }>("GET", `/repos/${repository}/actions/runs/${run.id}/jobs`);
		const steps = jobs.flatMap((job) => job.steps || []);
		if (steps.some((step) => step.name.endsWith(VERDICT_STEP_NAME) && step.conclusion === "failure")) return run;
	}

	return undefined;
}

function isByAuthor(run: WorkflowRun): boolean {
	return run.actor?.login === pullRequest.user.login;
}

// A PR's runs that reviewed a title or were limited from one, oldest first, which excludes those that carried over the previous verdict for the same title
function reviewRuns(runs: WorkflowRun[], earlierFinishedRun?: WorkflowRun): WorkflowRun[] {
	const oldestFirst = newestFirst(runs).reverse();
	const reviews: WorkflowRun[] = [];
	let latestFinishedRun = earlierFinishedRun;

	for (let i = 0; i < oldestFirst.length; i++) {
		const run = oldestFirst[i];
		if (run.display_title !== latestFinishedRun?.display_title) reviews.push(run);
		if (isFinished(run)) latestFinishedRun = run;
	}

	return reviews;
}

// The PR author's reviews across all PRs in the window
function contributorReviewRuns(recentRuns: WorkflowRun[], currentRun: WorkflowRun): WorkflowRun[] {
	const runsByPullRequest = new Map<number | undefined, WorkflowRun[]>();
	for (let i = 0; i < recentRuns.length; i++) {
		const run = recentRuns[i];
		if (run.id === currentRun.id) continue;

		const number = pullRequestNumber(run);
		runsByPullRequest.set(number, [...(runsByPullRequest.get(number) || []), run]);
	}

	return [...runsByPullRequest.values()].flatMap((runs) => reviewRuns(runs)).filter(isByAuthor);
}

// The PR number that begins each run's name (see `run-name` in the workflow)
function pullRequestNumber(run: WorkflowRun): number | undefined {
	const match = run.display_title.match(/^#(\d+): /);
	return match ? Number(match[1]) : undefined;
}

function isFinished(run: WorkflowRun): boolean {
	return run.conclusion === "success" || run.conclusion === "failure";
}

function newestFirst(runs: WorkflowRun[]): WorkflowRun[] {
	return [...runs].sort((a, b) => createdTime(b) - createdTime(a));
}

function createdTime(run: WorkflowRun): number {
	return Date.parse(run.created_at);
}

// Both messages are prepared here, since the review step has no GitHub token to fetch the PR's changes
async function buildTitleReview(previousSuggestions: string[]): Promise<TitleReview> {
	const [files, recentTitles] = await Promise.all([githubPaginate<ChangedFile>(`/repos/${repository}/pulls/${pullRequest.number}/files`, 5), recentMainBranchTitles()]);

	// Unguessable tag names stop untrusted text from closing its block early to pose as instructions
	const suffix = crypto.randomBytes(4).toString("hex");
	const block = (name: string, content: string) => `<${name}_${suffix}>\n${content}\n</${name}_${suffix}>`;
	const titleBlocks = [block("pr_title", pullRequest.title), block("recent_titles", recentTitles.join("\n"))];
	const instruction = `Review the PR title in the pr_title_${suffix} block.`;

	const userMessage = [...titleBlocks, block("changed_files", describeFiles(files)), block("diff", excerptDiff(files)), instruction].join("\n\n");
	if (previousSuggestions.length === 0) return { title: pullRequest.title, userMessage };

	const suggestionsMessage = [...titleBlocks, block("previous_suggestions", previousSuggestions.join("\n")), instruction].join("\n\n");
	return { title: pullRequest.title, userMessage, suggestionsMessage };
}

function describeFiles(files: ChangedFile[]): string {
	const lines: string[] = [];
	let length = 0;

	for (let i = 0; i < files.length; i++) {
		const { filename, previous_filename, status, additions, deletions } = files[i];
		const name = previous_filename ? `${shorten(previous_filename)} -> ${shorten(filename)}` : shorten(filename);
		const line = `${status} ${name} (+${additions} -${deletions})`;

		if (length + line.length > FILE_LIST_CHARACTER_BUDGET) break;
		lines.push(line);
		length += line.length + 1;
	}

	const unlisted = pullRequest.changed_files - lines.length;
	if (unlisted > 0) lines.push(`[${unlisted} more files aren't listed]`);

	return lines.join("\n");
}

// Each file's diff in turn, cut short where needed to fit the budget
function excerptDiff(files: ChangedFile[]): string {
	const excerpts: string[] = [];
	let remaining = DIFF_CHARACTER_BUDGET;
	let omitted = 0;

	for (let i = 0; i < files.length; i++) {
		const { filename, patch } = files[i];
		if (!patch || UNINFORMATIVE_FILES.test(filename)) continue;

		const header = `File: ${shorten(filename)}\n`;
		const limit = Math.min(FILE_DIFF_CHARACTER_LIMIT, remaining - header.length);
		if (limit <= 0) {
			omitted += 1;
			continue;
		}

		const excerpt = patch.length > limit ? `${patch.slice(0, limit)}\n[The rest of this file's diff is left out]` : patch;
		excerpts.push(`${header}${excerpt}`);
		remaining -= header.length + excerpt.length;
	}

	if (omitted > 0) excerpts.push(`[The diffs of ${omitted} more files are left out]`);
	return excerpts.join("\n\n") || "[No text diff is available]";
}

async function recentMainBranchTitles(): Promise<string[]> {
	const branch = encodeURIComponent(pullRequest.base.repo.default_branch);
	const commits = await github<{ commit: { message: string } }[]>("GET", `/repos/${repository}/commits?sha=${branch}&per_page=${RECENT_TITLES_COUNT}`);

	// Drops the PR number that squash merges append
	return commits.map(({ commit }) =>
		shorten(
			commit.message
				.split("\n")[0]
				.replace(/\s*\(#\d+\)$/, "")
				.trim(),
		),
	);
}

function shorten(text: string): string {
	return text.length > NAME_CHARACTER_LIMIT ? `${text.slice(0, NAME_CHARACTER_LIMIT)}...` : text;
}
