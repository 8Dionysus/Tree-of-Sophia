# Review Checklist

## Purpose

Use this checklist when reviewing a shareable artifact that claims to sanitize sensitive technical material for a broader audience.

## When it applies

- logs, configs, diagnostics, reports, or examples may contain sensitive details
- the result is intended for public or wider internal sharing
- the review needs to confirm that sanitization preserved usefulness without leaking unsafe context

## Review checklist

- [ ] Secrets, tokens, private paths, topology clues, and unsafe operational details were explicitly considered.
- [ ] The shared artifact preserves the technical lesson without preserving sensitive raw detail.
- [ ] The sanitization level matches the intended audience.
- [ ] Raw sensitive detail was not left behind by accident.
- [ ] Remaining uncertainty or limits of sanitization are named clearly.

## Not a fit

- tasks whose main question is whether the underlying action should be allowed
- tasks that are actually about executing or previewing an operational change rather than preparing a safe shareable surface
