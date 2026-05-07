# Risk surface matrix

This reference helps the agent name the operational surface before mutation.

## Common surfaces

- infra as code
- container or deployment orchestration
- identity / secret flow
- traffic routing
- stateful data / migration
- network boundary

## Practical rule

Name the surface before apply. Small diffs can still be high-risk when they
touch routing, identity, or stateful data.
