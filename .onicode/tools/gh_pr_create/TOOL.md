---
name: gh_pr_create
description: Create a GitHub Pull Request
---

# GitHub PR Create

Create a pull request on GitHub using the `gh` CLI.

## Usage

```bash
gh pr create --title "title" --body "body" --base main --head feature-branch
```

## Parameters

- `title` — PR title (required)
- `body` — PR description/body (required)
- `base` — Target branch (default: main)
- `head` — Source branch (default: current branch)

## Environment

Requires `gh` CLI to be installed and authenticated.
