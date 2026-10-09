use std::path::Path;

fn main() {
    if std::env::args_os().len() == 2
        && std::env::args_os()
            .nth(1)
            .is_some_and(|arg| arg == "corpus-build")
    {
        let input = std::io::stdin();
        let code = tos_command::managed_native_original_cli::run_corpus_build(
            input.lock(),
            &mut std::io::stdout().lock(),
            &mut std::io::stderr().lock(),
        );
        std::process::exit(code);
    }

    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "corpus-projection-check")
    {
        let args = std::env::args_os().skip(2).take(9).collect::<Vec<_>>();
        let input = std::io::stdin();
        let code = if args.is_empty() {
            tos_command::managed_native_original_cli::run_corpus_projection_check(
                input.lock(),
                &mut std::io::stdout().lock(),
                &mut std::io::stderr().lock(),
            )
        } else {
            tos_command::managed_native_original_cli::run_corpus_projection_check_args(
                &args,
                &mut std::io::stdout().lock(),
                &mut std::io::stderr().lock(),
            )
        };
        std::process::exit(code);
    }

    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "corpus-assessed-candidate")
    {
        let args = std::env::args_os().skip(2).take(4).collect::<Vec<_>>();
        let code = tos_command::managed_native_original_cli::run_corpus_assessed_candidate_args(
            &args,
            &mut std::io::stdout().lock(),
            &mut std::io::stderr().lock(),
        );
        std::process::exit(code);
    }

    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "corpus-projection-query")
    {
        let args = std::env::args_os().skip(2).take(22).collect::<Vec<_>>();
        if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
            println!(
                "usage: tos-native-owner-command corpus-projection-query --request ABS_JSON (--claim-ref REF | --subject-ref REF | --object-ref REF | --normalized-ref REF | --predicate ID | --review-status STATUS | --visibility VISIBILITY) [SELECTOR ...] [--limit 1..100] [--pretty]"
            );
            return;
        }
        let code = tos_command::source_bibliographic_query_cli::run_args(
            args,
            &mut std::io::stdout().lock(),
            &mut std::io::stderr().lock(),
        );
        std::process::exit(code);
    }

    if std::env::args_os().nth(1).is_some_and(|arg| arg == "http") {
        let args = std::env::args_os()
            .skip(2)
            .take(11)
            .map(|v| v.into_string())
            .collect::<Result<Vec<_>, _>>();
        let result = args
            .map_err(|_| "HTTP arguments must be UTF-8".to_owned())
            .and_then(|args| tos_command::source_command_http::run(&args));
        if let Err(reason) = result {
            eprintln!("native source command HTTP refused: {reason}");
            std::process::exit(2);
        }
        return;
    }
    if std::env::args_os().len() == 2
        && std::env::args_os()
            .nth(1)
            .is_some_and(|arg| arg == "native-original-produce")
    {
        let input = std::io::stdin();
        let code = tos_command::managed_native_original_cli::run(
            input.lock(),
            &mut std::io::stdout().lock(),
            &mut std::io::stderr().lock(),
        );
        std::process::exit(code);
    }
    if std::env::args_os().len() == 2
        && std::env::args_os()
            .nth(1)
            .is_some_and(|a| a == "acquisition")
    {
        std::process::exit(tos_command::source_acquisition_cli::run());
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "source-payload")
    {
        if std::env::args_os().len() == 3
            && std::env::args_os()
                .nth(2)
                .is_some_and(|arg| arg == "--help" || arg == "-h")
        {
            print!("{}", tos_command::source_payload_import::HELP);
            return;
        }
        if std::env::args_os().len() != 2 {
            eprintln!("native source-payload refused: request must be supplied on stdin");
            std::process::exit(2);
        }
        std::process::exit(tos_command::source_payload_import::run());
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "corpus-r2")
    {
        if std::env::args_os().len() == 3
            && std::env::args_os()
                .nth(2)
                .is_some_and(|arg| arg == "--help" || arg == "-h")
        {
            print!("{}", tos_command::corpus_r2_cli::HELP);
            return;
        }
        if std::env::args_os().len() != 2 {
            eprintln!("native corpus-r2 refused: request must be supplied on stdin");
            std::process::exit(2);
        }
        std::process::exit(tos_command::corpus_r2_cli::run());
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "corpus-admit" || arg == "authored-bootstrap")
    {
        // One sentinel preserves the CLI's bounded argument refusal without
        // collecting an arbitrary process argument sequence.
        let args = std::env::args_os().skip(2).take(65).collect::<Vec<_>>();
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let git_signal = std::sync::atomic::AtomicI32::new(0);
        let run = if std::env::args_os()
            .nth(1)
            .is_some_and(|arg| arg == "authored-bootstrap")
        {
            tos_command::source_admission_cli::run_authored_bootstrap_shared_cancel
        } else {
            tos_command::source_admission_cli::run_shared_cancel
        };
        let result = run(
            &args,
            &cancelled,
            &git_signal,
            &mut std::io::stdout().lock(),
            &mut std::io::stderr().lock(),
        );
        // The CLI owns its shared bounded output, including refusal context.
        // Never add a second unbounded printer or a legacy fallback here.
        std::process::exit(result.unwrap_or(2));
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "capacity-fixture")
    {
        // One sentinel past the command's 64-argument bound preserves refusal
        // without collecting an unbounded process argument sequence.
        let args = std::env::args_os().skip(2).take(65).collect::<Vec<_>>();
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let git_signal = std::sync::atomic::AtomicI32::new(0);
        let result = tos_command::source_capacity_workload_cli::run_shared_cancel(
            &args,
            &cancelled,
            &git_signal,
            &mut std::io::stdout().lock(),
            &mut std::io::stderr().lock(),
        );
        std::process::exit(result.unwrap_or(2));
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "foundation")
    {
        // One sentinel beyond the command's 64-argument cap preserves refusal
        // without allocating a vector for an unbounded argument sequence.
        let args = std::env::args_os().skip(2).take(65).collect::<Vec<_>>();
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let git_signal = std::sync::atomic::AtomicI32::new(0);
        let result = tos_command::source_current_cut::foundation_command::run(
            &args,
            &cancelled,
            &git_signal,
            &mut std::io::stdout().lock(),
            &mut std::io::stderr().lock(),
        );
        match result {
            Ok(code) => std::process::exit(code),
            // The foundation boundary already attempted its static refusal
            // through the same bounded output writer. Never print it again.
            Err(_) => std::process::exit(2),
        }
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "source-catalog")
    {
        // One sentinel beyond the command's argument cap preserves refusal
        // without allocating for an unbounded process argument sequence.
        let args = std::env::args_os().skip(2).take(65).collect::<Vec<_>>();
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let git_signal = std::sync::atomic::AtomicI32::new(0);
        let result = tos_command::source_current_cut::source_catalog_cli::run(
            &args,
            &cancelled,
            &git_signal,
            &mut std::io::stdout().lock(),
            &mut std::io::stderr().lock(),
        );
        match result {
            Ok(code) => std::process::exit(code),
            Err(error) => {
                eprintln!("native source catalog refused: {}", error.public_reason());
                std::process::exit(2);
            }
        }
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "source-commands")
        && std::env::args_os()
            .nth(2)
            .is_some_and(|arg| arg == "--discover")
    {
        let selector = match std::env::args_os().len() {
            3 => Ok(None),
            5 if std::env::args_os()
                .nth(3)
                .is_some_and(|arg| arg == "--handler") =>
            {
                std::env::args_os()
                    .nth(4)
                    .unwrap()
                    .into_string()
                    .map(Some)
                    .map_err(|_| {
                        tos_command::source_command::SourceCommandError::Invalid(
                            "source discovery handler UTF-8",
                        )
                    })
            }
            _ => Err(tos_command::source_command::SourceCommandError::Invalid(
                "source discovery arguments",
            )),
        };
        match selector.and_then(|handler| {
            tos_command::source_native_cli::discover_commands(handler.as_deref())
        }) {
            Ok(catalog) => println!("{catalog}"),
            Err(error) => {
                eprintln!("source command discovery refused: {error:?}");
                std::process::exit(2);
            }
        }
        return;
    }
    if std::env::args_os().len() == 3
        && std::env::args_os()
            .nth(1)
            .is_some_and(|arg| arg == "source-commands")
        && std::env::args_os()
            .nth(2)
            .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        println!(
            "usage: tos-native-owner-command source-commands --invocation ABSOLUTE_INVOCATION < REQUEST_JSON\n       tos-native-owner-command source-commands --discover [--handler HANDLER_ID]\n\nExecute the existing selected native source-owner request. The protected invocation selects the actual owner configuration, corpus/software cuts, executable and workers. Request bytes cannot select or issue authority. No Python or source checkout is needed; the optional Owner role and explicit owner-provided inputs are required. See packaged access/contracts/source-commands.v1.json."
        );
        return;
    }
    if std::env::args_os().len() == 2
        && std::env::args_os()
            .nth(1)
            .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        println!(
            "usage: tos-native-owner-command source-commands --invocation ABSOLUTE_INVOCATION\n       tos-native-owner-command --invocation ABSOLUTE_INVOCATION\n       tos-native-owner-command http --help\n       tos-native-owner-command foundation --help\n       tos-native-owner-command source-catalog --help\n       tos-native-owner-command corpus-admit --help\n       tos-native-owner-command corpus-build < REQUEST_JSON\n       tos-native-owner-command corpus-r2 < REQUEST_JSON\n       tos-native-owner-command source-payload --help\n       tos-native-owner-command corpus-projection-check --repo-root ABS --software-commit HEAD --schema-worker-env NAME --limits-profile repo-validation-v1\n       tos-native-owner-command corpus-projection-check --request ABS_JSON\n       tos-native-owner-command corpus-assessed-candidate --request ABS_JSON\n       tos-native-owner-command corpus-projection-query --request ABS_JSON --claim-ref REF [SELECTOR ...] [--limit 1..100] [--pretty]\n       tos-native-owner-command acquisition < REQUEST_JSONL\n       tos-native-owner-command capacity-fixture --help\n       tos-native-owner-command backup|restore --help\n       tos-native-owner-command source-capture --help\n\nSource commands read their request from stdin and require the selected invocation.\nUse source-commands --discover [--handler HANDLER_ID] for implementation-only discovery without source or owner access.\nFoundation requires an explicit repository root and protected invocation.\nBackup and restore require explicit owner-selected database, store and tool inputs."
        );
        return;
    }

    if std::env::args_os()
        .nth(1)
        .is_some_and(|a| a == "backup" || a == "restore")
    {
        if std::env::args_os().len() == 3
            && std::env::args_os()
                .nth(2)
                .is_some_and(|a| a == "--help" || a == "-h")
        {
            print!("{}", tos_command::backup_recovery_cli::HELP);
            return;
        }
        let args = std::env::args_os()
            .skip(1)
            .map(|v| v.into_string())
            .collect::<Result<Vec<_>, _>>();
        let result = args
            .map_err(|_| "arguments must be UTF-8")
            .and_then(|args| tos_command::backup_recovery_cli::run(&args));
        match result {
            Ok(value) => println!("{value}"),
            Err(reason) => {
                eprintln!("native recovery refused: {reason}");
                std::process::exit(2);
            }
        }
        return;
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|a| a == "source-capture")
    {
        if std::env::args_os().len() == 3
            && std::env::args_os()
                .nth(2)
                .is_some_and(|a| a == "--help" || a == "-h")
        {
            print!("{}", tos_command::source_git_capture_cli::HELP);
            return;
        }
        let result =
            tos_command::source_git_capture_cli::run_os_args(std::env::args_os().skip(2).take(257));
        match result {
            Ok(value) => {
                // Only fixed byte-transport fields, digests and scalar counters.
                // Full capture metadata/payload and private paths are not output.
                let encoded = value.to_string();
                if encoded.len() > 2048 {
                    eprintln!("native source capture refused: output bound");
                    std::process::exit(2);
                }
                println!("{encoded}");
            }
            Err(reason) => {
                eprintln!("native source capture refused: {reason}");
                std::process::exit(2);
            }
        }
        return;
    }
    let mut args = std::env::args_os();
    let _program = args.next();
    // A public name for the same maintained owner entry; no second launcher,
    // request builder, authority selection or reset of the owner's cutoff.
    let first = args.next();
    let option = if first.as_ref().is_some_and(|arg| arg == "source-commands") {
        args.next()
    } else {
        first
    };
    let input = std::io::stdin();
    let result = match (option, args.next(), args.next()) {
        (Some(option), Some(path), None) if option.to_str() == Some("--invocation") => {
            tos_command::source_native_cli::run(Path::new(&path), input.lock())
        }
        _ => Err(tos_command::source_command::SourceCommandError::Invalid(
            "native invocation argument",
        )),
    };
    match result {
        Ok(value) => {
            println!("{}", value);
            if value
                .get("schema_version")
                .and_then(serde_json::Value::as_str)
                == Some("tos_native_corpus_restore_committed_refusal_v1")
            {
                std::process::exit(2);
            }
        }
        Err(error) => {
            eprintln!("selected native owner refused: {error:?}");
            println!(
                "{{\"schema_version\":\"tos_local_source_command_error_v1\",\"error\":\"{}\"}}",
                match error {
                    tos_command::source_command::SourceCommandError::Invalid(_) => "ValueError",
                    tos_command::source_command::SourceCommandError::Conflict(_) =>
                        "JournalConflict",
                    tos_command::source_command::SourceCommandError::Denied(_)
                    | tos_command::source_command::SourceCommandError::DeniedWithReason(_) =>
                        "PermissionError",
                    _ => "Unsupported",
                }
            );
            std::process::exit(2);
        }
    }
}
