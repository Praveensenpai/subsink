---
name: tayori
description: >-
  Sends aesthetic Telegram notifications, approval requests, and completion notices to the user's phone via Tayori (便り).
  Use 'tayori ask' when awaiting user approval or asking critical questions, 'tayori done' when a task finishes,
  and 'tayori alert' for important warnings or errors.
---

# `tayori` (便り) Notification Skill

Integrates AI coding assistants with Telegram notifications via the `tayori` CLI.

## When to Use
- **Awaiting Approval**: When pausing for human confirmation before running destructive actions or major migrations.
- **Task Milestones & Completion**: When finishing a multi-step task, build, or release so the user knows immediately.
- **Errors & Warnings**: High-priority errors that need immediate user attention.

## Usage

```bash
# 1. Ask for approval
tayori ask "I have prepared the migration. Should I proceed with deploying?"

# 2. Notify task completion
tayori done "Release v1.0 successfully published and CI is green!"

# 3. Send warning or error alert
tayori alert "Database connection failed after 3 retries" --level error

# 4. Pipe command output
cargo test 2>&1 | tayori pipe --title "Cargo Test"
```
