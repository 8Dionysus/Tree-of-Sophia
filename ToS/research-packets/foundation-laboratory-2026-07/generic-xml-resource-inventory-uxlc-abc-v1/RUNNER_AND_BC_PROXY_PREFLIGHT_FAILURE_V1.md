# Runner and B+C proxy preflight failure — v1

Observed: 2026-08-30  
Phase: first candidate execution after the original freeze  
Source candidate execution reached: no  
Public synthetic files materialized before stop: 49

## Failure

The first run stopped on the public-synthetic `PC1-uxlc-shape` B+C candidate.
Two independent implementation mistakes were exposed:

1. `run_experiment.py` assumed GNU `time` wrote only one line. On a non-zero
   child exit it prepended `Command exited with non-zero status …`, so the
   timing parser tried to convert that text to a float.
2. The B+C projection mapped lxml elements by Python proxy `id()`. A later
   traversal may materialize another proxy for the same underlying libxml2
   node, so the provider element could not find its B resource ID and raised a
   `KeyError`.

The original frozen hashes were:

- builder: `fe8e3408…e592c`;
- runner: `56e744b0…1ad75`;
- consumer: `e8d9f2b0…d4f46d`;
- evaluator: `47fa0a00…92d18`.

## Consequence

This is an orchestration/identity-implementation failure, not a candidate
verdict. No source-derived candidate was built, no evaluator ran and no public
contract or UXLC Item was changed. The 49 partial public-synthetic outputs are
not used as evidence until the revised runner rebuilds every registered output
twice.

## Bounded correction

- parse the last GNU `time` line while retaining the child exit code;
- key lxml node maps by the element proxy's node-aware equality/hash behavior,
  not Python object address;
- apply the same parent-map correction in the independent consumer;
- point the evaluator to a new revisioned freeze receipt;
- re-freeze every changed executable before rerunning.

This failure remains tracked so a later green comparison cannot erase the
fact that the first run stopped before source execution.
