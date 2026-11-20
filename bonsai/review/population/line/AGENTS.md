# AGENTS.md

This card applies to the reading line of the Population bonsai under
`bonsai/review/population/line/`.

## Role

The line answers what the resolution and the planting leave open: are two
members, each grown from register rows, one bearer of responsibility or two;
and of a planted member, what kind of entry a row is, what the member's name is
in Latin script, when it lived and whether it lives. One packet
(`packets/p-NNNN/`) holds about twenty questions of one type with the evidence
of every row they are about. Two readers answer each packet independently; the
gate checks their verdicts mechanically; the line accepts what both agree on as
decisions, which `resolve.mjs` and `plant.mjs` read on their next run.

## Laws

1. A reader reads the rows, not the resolution's reasoning. The question names
   why it was asked (`reason`); the reader decides from the rows' own words and,
   where they do not settle it, from the sources the rows cite, an authority
   file (VIAF, GND, LCCN, ISNI, ORCID, Wikidata, BnF, DILA) or a scholarly
   source, each named in the verdict.
2. A wrong join of two people is worse than two members for one person. When
   the evidence does not settle it, the verdict is `undecidable`.
3. `same` needs evidence beyond the name: dates, works, an authority record,
   a teacher, a place, an office, or a text. A shared name never suffices.
4. A pseudonymous author is never `same` as the one whose name he writes under:
   that is `related` with `pseudonym_of`. A traditional or conventional author
   and a historical person are `related` with `conventional_for` unless the rows
   themselves identify them. The rows' words decide, not the parsed `kind`.
5. Every verdict quotes the rows: exact words of a field of a row the question
   holds. A quote the gate cannot find in the row refuses the verdict. A name
   or a year may come from a source instead, named in `sources`; never from
   memory alone.
6. An authority identifier a reader gives must be valid (the gate checks the
   syntax and the check digits) and must be the one the reader actually opened.
7. Write only your own `reader-<n>.jsonl` in your packet. Never open the other
   reader's file, never write `decisions.jsonl`, `accepted.json`, the queue or
   any file outside your packet; never run git, never install anything. Read
   the questions with `node ../../kit.mjs show p-NNNN`.
8. Stop when `node ../../kit.mjs gate p-NNNN --reader <n>` names no problem.
   Never run the gate without `--reader`: it shows the other reader's problems.

## Operator

```sh
node bonsai/lib/layers.mjs fetch bonsai/review/population   # the register rows and the parsed layer
node bonsai/review/population/resolve.mjs                   # keys, ledger, queue/identity.jsonl
node bonsai/review/population/line/kit.mjs packets          # cut new packets, planted members' questions first
node bonsai/review/population/line/kit.mjs queues           # a member's questions, after plant.mjs
node bonsai/review/population/line/kit.mjs packets --type dates   # or kind, latin, living
node bonsai/review/population/line/drive.mjs --parallel 8   # two codex readers per packet, gate, accept
node bonsai/review/population/resolve.mjs && node bonsai/review/population/plant.mjs
node bonsai/lib/cli.mjs check population
```

Packets accepted by the line change `decisions.jsonl`; after each round the
operator runs `resolve.mjs` and `plant.mjs` again and commits the decisions,
the keys, the ledger and the planted modules together. The next `packets`
retires a packet whose questions or evidence that run changed and cuts its
open questions anew. A `decision-conflict` (one decision joins what another
separates) is not cut into packets: the operator withdraws one of the two. `held` decisions (the
readers disagree, or both found it undecidable) wait for an operator or a third
reader; `kit.mjs status` counts them.
