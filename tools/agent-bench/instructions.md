## Checks

The project's checks: {check}. Automatic fixes and formatting: {fix}. Tests: {test}.
When you finish, the files you changed must be formatted, must add no new check findings, and the tests must pass.
{hooks}
## Shell

Run one simple command per Bash call (no pipes, `;`, `&&`, heredocs, `cd` or `python -c`). Allowed commands: {allowed}. Use the Edit and Write tools to change files. Work only inside this directory, do not commit, and touch only what the task needs.
<!-- hooks: the {hooks} line of the hook conditions -->
After every edit, a hook runs `{hook_lint}` on the edited file and reports its findings to you (nothing when the file is clean); when you finish, {fix} run automatically.
