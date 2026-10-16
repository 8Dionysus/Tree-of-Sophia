use std::io::{self, Read, Write};
use std::path::PathBuf;
use tos_ops_mechanics_plan::provider_controls::{self, MAX_BYTES};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let action = args.next().ok_or("missing provider control action")?;
    let mut root = None;
    let mut template = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" if root.is_none() => {
                root = Some(PathBuf::from(args.next().ok_or("missing root")?))
            }
            "--template" if template.is_none() => {
                template = Some(PathBuf::from(args.next().ok_or("missing template")?))
            }
            _ => return Err("unexpected provider control argument".into()),
        }
    }
    let value = match action.as_str() {
        "template" => {
            let files = provider_controls::template(&template.ok_or("missing template")?)?;
            serde_json::to_value(
                files
                    .into_iter()
                    .collect::<std::collections::BTreeMap<_, _>>(),
            )?
        }
        "materialize" => serde_json::to_value(provider_controls::materialize(
            &root.ok_or("missing root")?,
            &template.ok_or("missing template")?,
        )?)?,
        "verify" => {
            let mut raw = Vec::new();
            io::stdin()
                .take(MAX_BYTES as u64 + 1)
                .read_to_end(&mut raw)?;
            if raw.len() > MAX_BYTES {
                return Err("provider control request exceeds its byte limit".into());
            }
            provider_controls::verify_request(&root.ok_or("missing root")?, &raw)?;
            serde_json::Value::Null
        }
        _ => return Err("unexpected provider control action".into()),
    };
    let mut output = serde_json::to_vec(&value)?;
    output.push(b'\n');
    io::stdout().lock().write_all(&output)?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
