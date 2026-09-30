use std::path::Path;

fn main() {
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
    let input = std::io::stdin();
    let result = match (args.next(), args.next(), args.next()) {
        (Some(option), Some(path), None) if option.to_str() == Some("--invocation") => {
            tos_command::source_native_cli::run(Path::new(&path), input.lock())
        }
        _ => Err(tos_command::source_command::SourceCommandError::Invalid(
            "native invocation argument",
        )),
    };
    match result {
        Ok(value) => println!("{}", value),
        Err(error) => {
            eprintln!("selected native owner refused: {error:?}");
            println!(
                "{{\"schema_version\":\"tos_local_source_command_error_v1\",\"error\":\"{}\"}}",
                match error {
                    tos_command::source_command::SourceCommandError::Invalid(_) => "ValueError",
                    tos_command::source_command::SourceCommandError::Conflict(_) =>
                        "JournalConflict",
                    tos_command::source_command::SourceCommandError::Denied(_) => "PermissionError",
                    _ => "Unsupported",
                }
            );
            std::process::exit(2);
        }
    }
}
