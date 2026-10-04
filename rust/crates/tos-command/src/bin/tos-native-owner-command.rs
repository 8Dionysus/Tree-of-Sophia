use std::path::Path;

fn main() {
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
    if std::env::args_os().len() == 3
        && std::env::args_os()
            .nth(1)
            .is_some_and(|arg| arg == "source-commands")
        && std::env::args_os()
            .nth(2)
            .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        println!("usage: tos-native-owner-command source-commands --invocation ABSOLUTE_INVOCATION < REQUEST_JSON\n\nExecute the existing selected native source-owner request. The protected invocation selects the actual owner configuration, corpus/software cuts, executable and workers. Request bytes cannot select or issue authority. No Python or source checkout is needed; the optional Owner role and explicit owner-provided inputs are required. See packaged access/contracts/source-commands.v1.json.");
        return;
    }
    if std::env::args_os().len() == 2
        && std::env::args_os()
            .nth(1)
            .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        println!(
            "usage: tos-native-owner-command source-commands --invocation ABSOLUTE_INVOCATION\n       tos-native-owner-command --invocation ABSOLUTE_INVOCATION\n       tos-native-owner-command foundation --help\n       tos-native-owner-command backup|restore --help\n\nSource commands read their request from stdin and require the selected invocation.\nFoundation requires an explicit repository root and protected invocation.\nBackup and restore require explicit owner-selected database, store and tool inputs."
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
                    | tos_command::source_command::SourceCommandError::DeniedWithReason(_) => "PermissionError",
                    _ => "Unsupported",
                }
            );
            std::process::exit(2);
        }
    }
}
