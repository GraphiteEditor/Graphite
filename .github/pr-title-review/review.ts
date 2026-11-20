// Requests the review of the PR title that `gather.ts` prepared. This is the only step with the API key, and it has no GitHub token.
// The model gets no tools, so instructions injected into the title or diff can only sway the decision, which `report.ts` renders inertly.

import fs from "fs";
import path from "path";
import { type Decision, type ReviewRequest, type TitleReview, DECISION_PATH, ISSUES, REQUEST_PATH, describeError, makeDecision, sanitizeIssues, sanitizeSuggestions } from "./shared.ts";

// The repository's `PR_TITLE_REVIEW_MODEL` Actions variable switches models without a commit
const MODEL = process.env.PR_TITLE_REVIEW_MODEL?.trim() || "claude-opus-5-5";
const EFFORT = "medium";
// Bounds each response's cost, thinking included
const MAX_TOKENS = 16_000;
const REQUEST_TIMEOUT = 4 * 60_000;
const MAX_ATTEMPTS = 3;

type Message = { model: string; stop_reason: string; content: { type: string; text?: string }[]; usage: unknown };
type Output = Record<string, unknown>;

// A failure before the review produced anything, which costs nothing to try again
class UnavailableError extends Error {}

const request: ReviewRequest = JSON.parse(fs.readFileSync(REQUEST_PATH, "utf8"));

// Whether a request was answered in this run, which was paid for
let answered = false;

let decision: Decision;
if ("decision" in request) {
	decision = request.decision;
} else {
	try {
		decision = await review(request);
	} catch (error) {
		console.log(`::error::The title review failed: ${describeError(error)}`);
		decision = makeDecision(error instanceof UnavailableError && !answered ? "unavailable" : "error");
	}

	// An admin set this title, so it passes, keeping any feedback as suggestions
	if (request.passesRegardless) decision = decision.outcome === "rejected" ? { ...decision, outcome: "overridden" } : makeDecision("approved");
}

fs.writeFileSync(DECISION_PATH, JSON.stringify(decision));
console.log(`Outcome: ${decision.outcome}`);

async function review(request: TitleReview): Promise<Decision> {
	const apiKey = process.env.ANTHROPIC_API_KEY;
	if (!apiKey) throw new UnavailableError("The ANTHROPIC_API_KEY secret isn't available to this workflow");

	const guidelines = fs.readFileSync(path.join(import.meta.dirname, "guidelines.md"), "utf8").trim();
	const issueCodes = Object.entries(ISSUES)
		.map(([code, description]) => `- \`${code}\`: ${description}`)
		.join("\n");
	const system = `${guidelines}\n\n## Issue codes\n\n${issueCodes}\n`;

	// A title compared with the previous review's suggestions is reviewed without the PR's changes, unless it says something they don't
	if (request.suggestionsMessage) {
		const output = await ask(apiKey, system, request.suggestionsMessage, responseSchema(true));
		if (output.verdict !== "needs_diff") return toDecision(output, request.title);

		console.log("The title says something the previous suggestions don't, so it's reviewed again with the PR's changes");
	}

	return toDecision(await ask(apiKey, system, request.userMessage, responseSchema(false)), request.title);
}

// Without the PR's changes, accuracy can't be judged, so the response asks for them instead
function responseSchema(comparesWithSuggestions: boolean) {
	return {
		type: "object",
		properties: {
			verdict: { type: "string", enum: comparesWithSuggestions ? ["approve", "revise", "needs_diff"] : ["approve", "revise"] },
			issues: { type: "array", items: { type: "string", enum: Object.keys(ISSUES).filter((code) => !comparesWithSuggestions || code !== "inaccurate") } },
			suggestions: { type: "array", items: { type: "string" } },
		},
		required: ["verdict", "issues", "suggestions"],
		additionalProperties: false,
	};
}

async function ask(apiKey: string, system: string, userMessage: string, schema: object): Promise<Output> {
	const message = await requestMessage(apiKey, system, userMessage, schema);
	answered = true;
	console.log(`Model: ${message.model}, usage: ${JSON.stringify(message.usage)}`);
	if (message.stop_reason !== "end_turn") throw new Error(`The review stopped early with the reason "${message.stop_reason}"`);

	const text = message.content
		.filter((contentBlock) => contentBlock.type === "text")
		.map((contentBlock) => contentBlock.text || "")
		.join("");
	let output: Output;
	try {
		output = JSON.parse(text);
	} catch {
		throw new Error("The review's response wasn't valid JSON");
	}

	// Logged as JSON strings so the response's text can't start a line that would be read as a workflow command
	console.log(`Verdict: ${JSON.stringify(output.verdict)}, issues: ${JSON.stringify(output.issues)}, suggestions: ${JSON.stringify(output.suggestions)}`);
	return output;
}

function toDecision(output: Output, title: string): Decision {
	if (output.verdict === "approve") return makeDecision("approved");
	if (output.verdict !== "revise") throw new Error("The review's verdict wasn't recognized");
	return makeDecision("rejected", { issues: sanitizeIssues(output.issues), suggestions: sanitizeSuggestions(output.suggestions, title) });
}

async function requestMessage(apiKey: string, system: string, userMessage: string, schema: object): Promise<Message> {
	const body = JSON.stringify({
		model: MODEL,
		max_tokens: MAX_TOKENS,
		// Continues on a fallback model if a safety classifier declines the request, as can happen with security-related code in a diff
		fallbacks: "default",
		output_config: { effort: EFFORT, format: { type: "json_schema", schema } },
		system,
		messages: [{ role: "user", content: userMessage }],
	});

	// Retry rate limits, overloads, server errors, and failures to connect, with backoff
	for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt++) {
		let response: Response | undefined;
		try {
			response = await fetch("https://api.anthropic.com/v1/messages", {
				method: "POST",
				headers: {
					"content-type": "application/json",
					"x-api-key": apiKey,
					"anthropic-version": "2026-10-21",
					"anthropic-beta": "server-side-fallback-2026-07-01",
				},
				body,
				signal: AbortSignal.timeout(REQUEST_TIMEOUT),
			});
		} catch (error) {
			// A request that timed out may still be running, and billed, so it isn't sent again
			if (error instanceof Error && error.name === "TimeoutError") throw new Error("The review request timed out");
			console.log(`Attempt ${attempt} couldn't reach the review service: ${describeError(error)}`);
		}

		if (response?.ok) return (await response.json()) as Message;

		if (response) {
			const error = await response.json().then(
				(errorBody) => errorBody?.error,
				() => undefined,
			);
			console.log(`Attempt ${attempt} failed with status ${response.status}: ${JSON.stringify(error)}`);

			// Other client errors, like a malformed request or an invalid key, won't succeed on a retry
			if (response.status !== 429 && response.status < 500) break;
		}

		if (attempt < MAX_ATTEMPTS) {
			const retryAfterSeconds = Number(response?.headers.get("retry-after")) || 0;
			await new Promise((resolve) => setTimeout(resolve, Math.min(60, Math.max(retryAfterSeconds, 10 * attempt)) * 1000));
		}
	}

	throw new UnavailableError("The review request failed");
}
