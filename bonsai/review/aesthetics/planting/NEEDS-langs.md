# Langs: translation route needed

The direction needs the PR1 translation route for the existing accepted works:
add a separately sourced witness for a specified language, cut and align each
existing original place to whole translated sentences, retain the edition,
translator dates, source, page, rights and provenance, compare against the
source, and make a separate second-reader gate possible. Support partial
coverage without presenting it as complete. A recorded exhaustive search must
remain distinguishable from an actual witness and from work not yet searched.

The current kit gate requires `entry.witnesses` to contain only the original
and exposes no translation command (`kit.py --help`; `kit.py` around lines
688–707, at ea79117d3). The line's report already lists this route for PR1.
The langs direction will prepare source-checked language files before that
route lands; its source search currently awaits restoration of the mandated
built-in browser, whose host reports unavailable.
