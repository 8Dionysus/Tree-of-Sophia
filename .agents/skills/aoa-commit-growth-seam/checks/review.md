# Review Checklist

## Purpose

Use this checklist when reviewing a change that claims to follow
`aoa-commit-growth-seam`.

## When it applies

- a bounded diff is ready to cross a local commit boundary
- the author claims the commit stayed narrow and honest
- the review needs to confirm that commit, validation, and stop-line posture
  stayed explicit

## Review checklist

- [ ] The exact bounded diff was isolated before the commit.
- [ ] Unrelated local changes were excluded or explicitly deferred.
- [ ] `commit_authorization_posture` was `authorized_now` before the local commit happened.
- [ ] The validation state was named honestly before the commit happened.
- [ ] The commit message matches the bounded unit rather than generic success language.
- [ ] The workflow stopped at the local commit boundary instead of widening silently into push, PR, or publish work.

## Not a fit

- work that still needs more coding or verification before any commit is honest
- routes whose real center is approval classification, publish choreography, or post-commit automation
