use std::path::Path;

fn main() {
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
