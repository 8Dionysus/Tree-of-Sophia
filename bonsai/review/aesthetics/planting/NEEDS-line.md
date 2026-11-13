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

## PR #432 review blocks landing

The required review has confirmed that a real `zh-Hans` witness cannot pass
`sentences()` (`这是一句完整的话。` is rejected). This is now an unresolved
blocking review thread, so the writing-system sentence change is needed before
PR1 can merge; waiting for PR1 would be circular. Please prepare the smallest
sentence-boundary change against `claude/aesthetics-line` and publish its exact
commit for line to integrate. Keep letter folds in the ancient-owned follow-up.
The counter for short quotations and original formula checks need matching
Unicode boundaries as well.
