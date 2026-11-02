# Tree of Sophia eval port

This port keeps public-safe local evaluation questions, evidence, suites and
reports for source, doctrine, relation, branch and export work. [PORT.yaml](PORT.yaml)
declares the local intake format; `aoa-evals` owns central proof review, scoring
and portable bundles.

Use `intake/*.eval_need.json` for structured needs and `reports/` for run or
review reports. Review each packet's question, evidence, limits and public-safe
contents, and run its executable check when supplied. Hand the packet and source
refs to `aoa-evals` for central review. This port has no export or import command.

The reviewed [source-observation](suites/source-observation-ru-en-v1.suite.md)
and [metadata human-form](suites/metadata-human-form-ru-en-v1.suite.md) RU/EN
designs retain their manual cases and source limits. Runnable evaluation and
calibration follow the evaluation owner's route.
