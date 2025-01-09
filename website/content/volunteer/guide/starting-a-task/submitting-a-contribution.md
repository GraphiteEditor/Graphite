+++
title = "Submitting a contribution"

[extra]
order = 3 # Page number after chapter intro
+++

Collaboration is a key part of real-world software engineering. Graphite follows some basic procedures to keep the process smooth and efficient. You will want to familiarize yourself with these guidelines to save yourself and Graphite maintainers time and confusion.

This assumes you understand enough about how Git works to utilize commits, branches, and multiple remotes. If you're new to Git, you will need to learn those topics on your own, but a good starting point is [this portion](https://youtu.be/vUzIeg8frh4?t=237) of the Graphite intro webcast which recommends installing the [Git Graph](https://marketplace.visualstudio.com/items?itemName=mhutchie.git-graph) extension for VS Code to visualize your Git history and branches.

## Git branch name

Before making your first commit, create a new branch with a name that describes what it's about. Aim for short but sufficiently descriptive. Kebab-case (using hyphens between words) is our usual convention. Don't include a prefix like `feature/` or `fix/` which just adds visual noise. An example like `fix-path-tool-selection-history` is fine, but almost too long.

Rename it if you already made a branch with a different name. Create a new branch if you've been committing to `master` or another existing branch. If your branch is specifically called `master`, it becomes harder to work with during code reviews.

After you push your branch to GitHub then open a PR, you won't be able to change its name. But please don't close a PR and open a new one just because the branch name isn't optimal. Just keep these tips in mind for the next time.

## Pull request

Once you have gotten your code far enough along that you are confident you'll be able to complete it, open a pull request (PR). You might also do this earlier if a maintainer requests to see your code in order to assist you.

Later on when you are building larger features, a PR should be opened once you have meaningful progress. That way, it can be kept safe on GitHub and other maintainers can check in to see your status so your work is less of a mystery.

Here's the important part: when you open a PR, it should be marked as a draft unless it is currently ready for review. The left image shows how to open a new PR as a draft, and the right image shows how to convert an existing PR to a draft.

<p><img src="https://static.graphite.rs/content/volunteer/guide/draft-pr.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Screenhots showing GitHub's &quot;Create pull request (arrow) > Create draft pull request&quot; and &quot;Still in progress? Convert to draft&quot; buttons" /></p>

<center><em>Open a new PR as a draft / convert an existing PR to a draft</em></center>

You should mark it as ready for review and ping a maintainer when you believe your code implements the needed functionality and doesn't introduce any new bugs or broken features.

## Title and description

Your PR title will become the commit message of your feature's commit in the project Git history. It should aim to concisely but descriptively summarize what your PR does. We use sentence case and imperative mood ("Fix X bug", "Add Y feature", "Make Z faster").

If you are working on a task from the `#✅code-todo-list` Discord channel, you should right-click the exact message, select "Copy Message Link", and paste that into your PR description.

If you are working on a task with a GitHub issue, please be certain to include that issue number in the description. GitHub requires the format "Closes #123", "Fixes #123", or "Resolves #123". If there are multiple issues, you have to fully repeat this trigger word for each one. If there is no issue, remove the pre-filled "Closes #" description text.

When the PR gets merged, any issue referenced by the trigger word will be automatically closed. That isn't desirable for [tracking issues](https://github.com/GraphiteEditor/Graphite/issues?q=is%3Aissue+is%3Aopen+in%3Atitle+%22tracking+issue%22), so you should instead use "Part of #123" which isn't a trigger word. After the PR merges, please edit the description to change "Part of" to "Closes" so the tracking issue links to the PR without it having gotten closed.

As a bonus, it can be helpful for maintainers if you take a few minutes to write about what you changed and include relevant screenshots or video clips.

If you have concerns about a certain approach you took or if a certain part of your code is as clean as it could be, you can leave comments on lines of your own code from the "Files changed" tab after opening the PR.

## Comment on the issue

For any issue referenced in your PR (including tracking issues), we need you to leave a comment on issue. It doesn't matter what you write. You can just say "I opened PR #456" or something to that effect. This is only necessary because we will need to assign that issue to you upon merging your PR, but GitHub only allows assignments to those who have commented.

That way you get credit for your work and we can keep our closed issues cleanly organized. For consistency, a closed issue should have an assignee if it was resolved by a PR. Otherwise, only duplicate or invalid issues should be closed without an assignee.

We don't commonly assign issues while a PR is still in progress, only upon landing the PR. That's because PRs often get abandoned and we don't want an assignment blocking someone else from picking up the work.

## Code review etiquette

It is your responsibility to build the editor, thoroughly test your work, and employ common sense to avoid wasting a maintainer's time in needing to point out obvious flaws. It is not uncommon for inexperienced contributors to request review when their code entirely fails to implement the task at hand, or breaks surrounding functionality in a way that should have been immediately apparent. This doesn't leave a good impression and can frustrate maintainers.

If you don't actually understand what is intended with your feature/fix and why this is meaningful to a user of Graphite, spend time becoming that user and understanding the context. [Learning](/learn) at least the basics of using Graphite is important. Then ask questions in Discord if you're still confused about specific edge cases or the wording of the task.

It is also common for larger tasks to enter a round of review to confirm the direction is correct before you go back and polish the remaining details of the implementation. It's good to be in touch with the team to decide on when is the right time for this kind of preliminary review. It can save you effort reworking problems if you misunderstand the goals, or if the exact details of the requirements were never well-defined and you'll need to iterate on the design together with the team. Don't feel that every part of your PR needs to be 100% finished before requesting feedback, but also be clear so you aren't taking a maintainer away from other work to point out that you are obviously nowhere near done.

## Self-review

Before marking your PR as ready for review, you should do a self-review. That means reading over the diff of all your changes to ensure they are correct, complete, and lacking frivolous changes like unintended whitespace alterations, leftover debugging code, or commented-out lines. Read over it with a fine-toothed comb so maintainers don't have to nitpick as much. It is only fair that your first code reviewer should be yourself, so you catch the obvious flaws first.

## Passing CI

Upon pushing a commit to your PR's branch, CI will need to build and test your code. PRs from forks will have to wait until a maintainer approves the CI run. If you're uncertain, run `cargo test --all-features` on your machine or ask a maintainer to trigger CI for you.

You also have to pass `cargo fmt` and `cargo clippy` locally before your PR can be merged.

Your goal is for the check called "Editor: Dev & CI / build (pull_request)" to pass with a ✅. If it fails with a ❌, you will need to investigate. If you need access to the build logs, ask a maintainer to provide them. Occasionally, other checks may fail, but you likely won't be responsible for fixing those and they can be ignored.

## Keeping your work up-to-date

Be sure to start your work from the latest commit on the `master` branch by pulling (`git pull`) with `master` checked out when you begin coding.

As time goes on and `master` accumulates new commits, your branch will become outdated. It has to be synced up with `master` before your PR can be merged. Sometimes there will be conflicts that you need to resolve, which you can find learning resources for online. Enabling Git's three-way diff conflict style with `git config --global merge.conflictstyle diff3` can make this process easier.

When your branch can be updated with `master` without conflicts, you can click the "Update branch" button below the CI status. If you click the dropdown button beside it, you can choose instead to update with a rebase. If this can be done without conflicts, this is preferred because it maintains a clean, linear history for your branch.

<p><img src="https://static.graphite.rs/content/volunteer/guide/update-branch-with-rebase.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="Screenhots showing GitHub's &quot;Update with rebase&quot; button" /></p>

Be sure to pull the rebased, or updated-with-a-merge-commit, branch after you or a maintainer updates it (or pushes other commits to it) to ensure you are working on the latest code.

## Review process

Assuming you have done what's explained above, a maintainer will aim to review your PR within a few days if possible. Feel free to send reminders because PRs can get overlooked.

As a rule of thumb, at this stage you are about 50% done with your work. The other 50% of your time will be spent responding to feedback and making (sometimes significant) changes.

There are two parts to the review process, QA and code review, which occur separately:

- Quality assurance (QA): A build of your code will be opened and tested to ensure it implements the requested functionality and doesn't introduce regressions. This is not a substitute for your own testing, but it is a necessary line of defense against overlooked issues. This is usually performed by Keavon, the founder and product designer, whose eye for detail keeps the app polished and consistent. Maintainers (and only maintainers) have the ability to invoke CI by commenting "!build" on your PR which will produce a build link. That is a unique link hosting a build of your PR's current code.
- Code review: The code will be checked for flawed approaches, pitfalls, confusing logic, [style guide](../code-quality-guidelines) adherence, sufficient comments and tests, and general quality. A review may be left through GitHub or your PR may have commits added to it. Feel free to read the diffs of those commits to understand what was changed so you can learn from that feedback. Direct commits are often faster than leaving dozens of comments. These can range from nitpicks to larger improvements. Our process is to collaborate on PRs as a team to write the best code possible, meaning your PR won't always be exclusively written by you.

When changes are requested, the maintainer will usually mark the PR as a draft again while awaiting your updates. It is your responsibility to mark it as ready for review again once you've addressed the feedback.

- If a PR is a draft, the ball is in your court to move it forward.
- If it's marked as ready for review, it means there is nothing more for you to do until the maintainer has time to review it.

After any number of back-and-forth cycles, a maintainer (usually Keavon who often gives the final say) will merge your PR. All your commits will be squashed into a single new commit on the `master` branch. This keeps the Git history linear and easy to follow.

Congratulations on landing your successful contribution! Ping `@Keavon` on Discord to be given the "Code Contributor" role.
