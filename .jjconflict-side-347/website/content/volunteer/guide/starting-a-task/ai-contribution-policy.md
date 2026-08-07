+++
title = "AI contribution policy"

[extra]
order = 4 # Page number after chapter intro
+++

Many open source projects including Graphite have begun to be spammed with an ever-increasing flood of low-quality PRs written partly or wholly by AI. These harm the project by wasting the time of maintainers and preventing PRs by genuine contributors from receiving timely review. We aim to be reasonable and understanding to contributors who put in the effort, but it has become necessary to set some strict rules against low-effort PRs.

## Acceptable usage

- Non-agent AI tools may **assist** with debugging and tab-completion of single lines of code you would have otherwise written yourself. This does not require disclosure.
- AI chat tools (not agents) may help you **generate** small (sub-40 line) snippets of code that you manually copy and paste, provided that you carefully review every line to ensure it is consistent with how you would have written it yourself. This requires disclosure.

## Unacceptable usage

- AI slop, "vibe-coded", or agent-written PRs are strictly forbidden and may be treated as malicious spam attacks against the project, resulting in a ban.
- PR description text and replies to reviewers must be written by you, not AI. If your English is imperfect, just try your best; it is better than AI babble.

## Required disclosure

- Graphite has **zero-tolerance** for contributing undisclosed AI-generated content.
- A detailed, human-written description must accompany every line of material that you did not personally write using your own brain. It should justify why each line is correct and appropriate. This should be prepared ahead of time and written as self-review comments on the GitHub PR's diff immediately after the PR is opened or new code is pushed.
