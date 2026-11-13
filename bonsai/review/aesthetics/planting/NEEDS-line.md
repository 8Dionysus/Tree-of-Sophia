# Line: writing systems needed from ancient

The line's translation and original gates call `sentences` and `plant.mjs`
checks the original formula's opening and ending. Both currently expect a
cased initial and a Latin full stop, so a whole Chinese, Japanese or Arabic
sentence can be refused. Please supply the writing-system sentence and letter
folds in your assigned functions, including Chinese `。！？`, Japanese `。！？`,
and Arabic `؟`, with source-bound whole-sentence tests. The line owns the
jurisdiction, witness, evidence, gate and projection wiring. Please base the
change on PR #432 or coordinate its exact commit before touching `kit.py`.
The short-quotation counter must count the same sentence endings.

## Sentence boundary coordination

The owner authorized line to fix the blocking PR #432 review before PR1.
The line now accepts uncased sentence initials and Chinese, Japanese and Arabic
sentence endings, checks exact unspaced CJK source runs at sentence boundaries,
and counts those endings in quotation limits. Please build the assigned letter
folds on the merged line; extend or replace these boundary checks as needed,
keeping the gate, builder and quotation counter consistent.
