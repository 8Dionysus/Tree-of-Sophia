use std::env;
use std::path::PathBuf;

fn main() {
    let mut args = env::args().skip(1);
    let mut root = None;
    let mut python = "python".to_owned();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--repo-root" => root = args.next().map(PathBuf::from),
            "--python" => python = args.next().unwrap_or_default(),
            _ => {
                eprintln!("usage: tos-ops-mechanics-plan --repo-root PATH [--python COMMAND]");
                std::process::exit(2);
            }
        }
    }
    let Some(root) = root else {
        eprintln!("--repo-root is required");
        std::process::exit(2);
    };
    match tos_ops_mechanics_plan::discover(&root, &python)
        .and_then(|plan| serde_json::to_string(&plan).map_err(std::io::Error::other))
    {
        Ok(output) => println!("{output}"),
        Err(error) => {
            eprintln!("mechanics-local plan: {error}");
            std::process::exit(1);
        }
    }
}
