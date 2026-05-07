# Bounded lifecycle visibility

Local bring-up stays legible when the launch and stop surfaces are obvious.

## Keep visible

- one explicit launch command
- one explicit stop command
- unresolved warnings
- any cleanup-sensitive surfaces such as volumes or caches

## Anti-patterns

- hiding start in background helpers with no visible stop path
- launching multiple selectors without naming the chosen one
- treating a doctor pass as proof of post-start health
