// Comments on the PR with the title review's feedback, then tells the workflow whether to pass or fail the check that approves the title.
// The decision was made from untrusted input, so the comment uses only this file's wording plus suggested titles inside code blocks.

import fs from "fs";
import {
	type Decision,
	COMMENT_MARKER,
	DECISION_PATH,
	ISSUES,
	PASSING_OUTCOMES,
	RETRIED_OUTCOMES,
	codeBlock,
	describeError,
	github,
	listReviewComments,
	makeDecision,
	parseDecision,
	readEvent,
	requireEnvironment,
} from "./shared.ts";

const ANNOTATIONS = {
	approved: "The PR title is approved",
	still_approved: "The PR title is approved",
	overridden: "The PR title is accepted as an admin set it, with suggestions in the PR's comments",
	rejected: "The PR title needs revising, as the PR's comments explain",
	limited: "The PR title wasn't reviewed because of the daily limit",
	error: "The PR title couldn't be reviewed because of an error",
	unchanged: "The PR title's earlier review still stands, as the PR's comments explain",
	superseded: "The PR title changed during this run, so a newer run reviews it",
	unavailable: "The PR title couldn't be reviewed this time, so the next push or title edit tries again",
};

const repository = requireEnvironment("GITHUB_REPOSITORY");
const pullRequestNumber = readEvent().pull_request.number;

const decision = readDecision();
const retried = RETRIED_OUTCOMES.includes(decision.outcome);
const passed = PASSING_OUTCOMES.includes(decision.outcome);

// New feedback replaces earlier comments so the latest is at the bottom of the conversation, while carried-over and retried outcomes leave them be
if (decision.outcome !== "unchanged" && decision.outcome !== "still_approved" && !retried) {
	try {
		await replaceComments(decision);
	} catch (error) {
		console.log(`::warning::Updating the review's comments failed: ${describeError(error)}`);
	}
}

console.log(`::${passed ? "notice" : "error"}::${ANNOTATIONS[decision.outcome]}`);

// The workflow fails a different step for a verdict against the title than for an outcome to retry, which later runs read back to tell them apart
const result = passed ? "approved" : retried ? "retry" : "verdict";
fs.appendFileSync(requireEnvironment("GITHUB_OUTPUT"), `result=${result}\n`);

function readDecision(): Decision {
	try {
		return parseDecision(JSON.parse(fs.readFileSync(DECISION_PATH, "utf8"))) || makeDecision("unavailable");
	} catch {
		return makeDecision("unavailable");
	}
}

async function replaceComments(decision: Decision) {
	const comments = await listReviewComments(repository, pullRequestNumber);
	await Promise.all(comments.map(({ id }) => github("DELETE", `/repos/${repository}/issues/comments/${id}`)));

	if (decision.outcome !== "approved") await github("POST", `/repos/${repository}/issues/${pullRequestNumber}/comments`, { body: commentBody(decision) });
}

function commentBody(decision: Decision): string {
	if (decision.outcome === "limited") {
		const time = decision.retryAfter > 0 ? ` after ${new Date(decision.retryAfter).toISOString().slice(0, 16).replace("T", " ")} UTC` : " later";
		const limit = `${COMMENT_MARKER}\n**This PR's title wasn't reviewed** because of the daily limit on reviews. It can be reviewed again when the title is edited${time}.`;
		if (decision.suggestions.length === 0) return limit;

		return [limit, "The titles suggested earlier are still accepted right away when used exactly:", ...decision.suggestions.map(codeBlock)].join("\n\n");
	}

	if (decision.outcome === "error") {
		return `${COMMENT_MARKER}\n**This PR's title couldn't be reviewed** because of an error. It will be reviewed again when the title is edited, or when a maintainer re-runs this check.`;
	}

	const issues = decision.issues.length > 0 ? decision.issues.map((code) => `- ${ISSUES[code]}`) : ["- It doesn't follow the project's conventions for titles."];
	const suggestions = decision.suggestions.length > 0 ? ["Suggested titles:", ...decision.suggestions.map(codeBlock)] : [];

	// An admin's title stands, so its feedback is only advice
	if (decision.outcome === "overridden") {
		return [`${COMMENT_MARKER}\n**This PR's title is accepted** as an admin set it. The review would otherwise have noted:`, issues.join("\n"), ...suggestions].join("\n\n");
	}

	return [`${COMMENT_MARKER}\n**This PR's title needs revising** to follow the project's conventions:`, issues.join("\n"), ...suggestions, "<sub>Editing the title reviews it again.</sub>"].join(
		"\n\n",
	);
}
