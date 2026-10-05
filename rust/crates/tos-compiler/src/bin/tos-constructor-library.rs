#[path = "../constructor_library.rs"]
mod constructor_library;

use std::{
    env,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process,
};

const DEFAULT_SOURCE: &str = "/srv/AbyssOS/Tree-of-Sophia";
const DEFAULT_DEMO: &str =
    "/srv/abyss-machine/storage/artifacts/tos-eternal-return-demo-20260910/demo-data.json";
const DEFAULT_OUTPUT: &str =
    "/srv/abyss-machine/storage/artifacts/tos-tree-constructor-20260910/library.json";

struct Args {
    repo: PathBuf,
    demo: PathBuf,
    output: PathBuf,
    builder: Option<PathBuf>,
    check: bool,
}

fn usage() -> &'static str {
    "Usage: tos-constructor-library [--source-repo PATH] [--demo-packet PATH] [--output PATH] [--builder-repo PATH] [--check]"
}

fn parse_args() -> Result<Args, String> {
    let mut repo = PathBuf::from(DEFAULT_SOURCE);
    let mut demo = PathBuf::from(DEFAULT_DEMO);
    let mut output = PathBuf::from(DEFAULT_OUTPUT);
    let mut builder = None;
    let mut check = false;
    let mut args = env::args_os().skip(1);
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--source-repo") => {
                repo = PathBuf::from(args.next().ok_or("--source-repo requires a path")?)
            }
            Some("--demo-packet") => {
                demo = PathBuf::from(args.next().ok_or("--demo-packet requires a path")?)
            }
            Some("--output") => {
                output = PathBuf::from(args.next().ok_or("--output requires a path")?)
            }
            Some("--builder-repo") => {
                builder = Some(PathBuf::from(
                    args.next().ok_or("--builder-repo requires a path")?,
                ))
            }
            Some("--check") => check = true,
            Some("--help") | Some("-h") => {
                println!("{}", usage());
                process::exit(0);
            }
            _ => return Err(format!("unknown argument {:?}\n{}", argument, usage())),
        }
    }
    Ok(Args {
        repo,
        demo,
        output,
        builder,
        check,
    })
}

fn absolute(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path))
    }
}

fn resolved(path: &Path) -> Result<PathBuf, String> {
    let absolute = absolute(path)?;
    if absolute.exists() {
        return fs::canonicalize(&absolute)
            .map_err(|error| format!("{}: {error}", absolute.display()));
    }
    let mut current = absolute.as_path();
    let mut suffix = Vec::<OsString>::new();
    loop {
        if current.exists() {
            let mut result = fs::canonicalize(current)
                .map_err(|error| format!("{}: {error}", current.display()))?;
            for component in suffix.iter().rev() {
                if component == "." {
                    continue;
                }
                if component == ".." {
                    result.pop();
                } else {
                    result.push(component);
                }
            }
            return Ok(result);
        }
        let name = current
            .file_name()
            .ok_or_else(|| format!("Cannot resolve output path {}", absolute.display()))?;
        suffix.push(name.to_os_string());
        current = current
            .parent()
            .ok_or_else(|| format!("Cannot resolve output path {}", absolute.display()))?;
    }
}

fn is_within(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

fn main_result() -> Result<(), String> {
    let args = parse_args()?;
    let repo = fs::canonicalize(&args.repo)
        .map_err(|error| format!("{}: {error}", args.repo.display()))?;
    if !repo.is_dir() {
        return Err("Source repository is not a directory".into());
    }
    let builder = match args.builder {
        Some(path) => {
            fs::canonicalize(&path).map_err(|error| format!("{}: {error}", path.display()))?
        }
        None => repo.clone(),
    };
    let demo = fs::canonicalize(&args.demo)
        .map_err(|error| format!("{}: {error}", args.demo.display()))?;
    let output = resolved(&args.output)?;
    if output == demo {
        return Err("Output must not overwrite the previous demo packet".into());
    }
    for private_path in [&output, &demo] {
        if is_within(private_path, &repo) || is_within(private_path, &builder) {
            return Err("Private material must stay outside both source worktrees".into());
        }
    }

    let build = constructor_library::build(&repo, &demo)?;
    let bytes = build.output_bytes()?;
    if args.check {
        let existing =
            fs::read(&output).map_err(|error| format!("{}: {error}", output.display()))?;
        let existing = String::from_utf8(existing)
            .map_err(|error| format!("{}: {error}", output.display()))?;
        let expected = std::str::from_utf8(&bytes).map_err(|error| error.to_string())?;
        if constructor_library::normalize_newlines(existing) != expected {
            return Err("Existing library differs from reproducible output".into());
        }
        let metadata = fs::metadata(&output).map_err(|error| error.to_string())?;
        if metadata.mode() & 0o077 != 0 {
            return Err("Private library permissions are too broad".into());
        }
    } else {
        let parent = output.parent().ok_or("Output path has no parent")?;
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&output)
            .map_err(|error| format!("{}: {error}", output.display()))?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
        file.write_all(&bytes).map_err(|error| error.to_string())?;
    }
    let status = if args.check { "checked" } else { "built" };
    let summary = build.summary_bytes(status, &output, &bytes)?;
    println!(
        "{}",
        String::from_utf8(summary).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn main() {
    if let Err(error) = main_result() {
        eprintln!("tos-constructor-library: {error}");
        process::exit(2);
    }
}
