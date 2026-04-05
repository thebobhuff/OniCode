---
name: gh_pr_merge
description: Merge a GitHub Pull Request
---

# GitHub PR Merge

Merge a pull request on GitHub using the `gh` CLI.

## Usage

```bash
gh pr merge <pr-number> --squash --delete-branch
```

## Parameters

- `pr_number` — PR number to merge (required)
- `method` — Merge method: merge, squash, rebase (default: squash)
- `delete_branch` — Delete branch after merge (default: true)

## Environment

Requires `gh` CLI to be installed and authenticated.
