//! Exact Unicode 16 tables from four owner-pinned UCD inputs. Network access and
//! host Unicode versions are deliberately absent from the generator contract.
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};
use tos_foundation::Digest256;
const VERSION: &str = "16.0.0";
const FILES: [(&str, &str); 4] = [
    (
        "UnicodeData",
        "ff58e5823bd095166564a006e47d111130813dcf8bf234ef79fa51a870edb48f",
    ),
    (
        "SpecialCasing",
        "8d5de354eef79f2395a54c9c7dcebbaf3d30fc962d0f85611ea97aa973a0c451",
    ),
    (
        "DerivedCoreProperties",
        "39d35161f2954497f69e08bdb9e701493f476a3d30222de20028feda36c1dabd",
    ),
    (
        "CaseFolding",
        "6f1f9c588eb4a5c718d9e8f93b782685e5c7fec872cf05e8e6878053599e09bb",
    ),
];
const TABLE_SHA: &str = "e9ab817aef6bf870fc8b855cffe7c06ac46aa6a68e6c259c75920f1bcfbb42e6";
type Result<T> = std::result::Result<T, Box<dyn Error>>;
fn bad(message: &str) -> Box<dyn Error> {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message).into()
}
fn cp(value: &str) -> Result<u32> {
    let x = u32::from_str_radix(value, 16)?;
    if x > 0x10ffff {
        return Err(bad("UCD codepoint exceeds Unicode range"));
    }
    Ok(x)
}
fn mapped(value: &str) -> Result<String> {
    value
        .split_whitespace()
        .map(|v| char::from_u32(cp(v)?).ok_or_else(|| bad("mapping is not a Unicode scalar")))
        .collect()
}
fn ascii_json(value: &Value) -> Result<String> {
    let raw = serde_json::to_string(value)?;
    let mut out = String::new();
    for ch in raw.chars() {
        if ch as u32 > 127 {
            for unit in ch.encode_utf16(&mut [0u16; 2]) {
                out.push_str(&format!("\\u{unit:04x}"));
            }
        } else {
            out.push(ch)
        }
    }
    Ok(out)
}
fn ranges(points: &BTreeSet<u32>) -> Vec<(u32, u32)> {
    let mut rows: Vec<(u32, u32)> = Vec::new();
    for &point in points {
        if let Some(last) = rows.last_mut() {
            if last.1 + 1 == point {
                last.1 = point;
                continue;
            }
        }
        rows.push((point, point));
    }
    rows
}
fn table(dir: &Path) -> Result<String> {
    let mut texts = BTreeMap::new();
    for (name, digest) in FILES {
        let path = dir.join(format!("{name}-{VERSION}.txt"));
        let m = fs::symlink_metadata(&path)?;
        if !m.is_file() || m.len() > 8 * 1024 * 1024 {
            return Err(bad("UCD input must be a bounded regular file"));
        }
        let raw = fs::read(path)?;
        if Digest256::of_bytes(&raw).to_hex() != digest {
            return Err(bad("UCD source digest differs"));
        }
        texts.insert(name, String::from_utf8(raw)?);
    }
    let mut lower = BTreeMap::<u32, String>::new();
    let mut decimal = BTreeMap::<u32, String>::new();
    let mut printable = BTreeSet::new();
    let mut interval = None;
    for line in texts["UnicodeData"].lines() {
        let row: Vec<_> = line.split(';').collect();
        if row.len() != 15 {
            return Err(bad("UnicodeData field count"));
        }
        let point = cp(row[0])?;
        if !row[13].is_empty() {
            lower.insert(point, mapped(row[13])?);
        }
        if !row[6].is_empty() {
            decimal.insert(point, row[6].to_owned());
        }
        let allowed = !row[2].starts_with(['C', 'Z']);
        if row[1].ends_with(", First>") {
            if interval.replace(point).is_some() {
                return Err(bad("nested UCD range"));
            }
        } else if row[1].ends_with(", Last>") {
            let first = interval.take().ok_or_else(|| bad("unopened UCD range"))?;
            if first > point {
                return Err(bad("backward UCD range"));
            }
            if allowed {
                printable.extend(first..=point);
            }
        } else if allowed {
            printable.insert(point);
        }
    }
    if interval.is_some() {
        return Err(bad("unterminated UCD range"));
    }
    printable.insert(32);
    let mut conditions = Vec::new();
    for line in texts["SpecialCasing"].lines() {
        let content = line.split('#').next().unwrap().trim();
        if content.is_empty() {
            continue;
        }
        let row: Vec<_> = content.split(';').map(str::trim).collect();
        if row.len() < 5 {
            return Err(bad("SpecialCasing field count"));
        }
        if !row[4].is_empty() {
            if !row[4]
                .split_whitespace()
                .any(|x| matches!(x, "tr" | "az" | "lt"))
            {
                conditions.push((row[0], row[1], row[4]));
            }
        } else {
            lower.insert(cp(row[0])?, mapped(row[1])?);
        }
    }
    if conditions != vec![("03A3", "03C2", "Final_Sigma")] {
        return Err(bad("unhandled contextual lowercase rule"));
    }
    lower.retain(|point, value| char::from_u32(*point).is_none_or(|ch| ch.to_string() != *value));
    let mut cased = BTreeSet::new();
    let mut ignorable = BTreeSet::new();
    for line in texts["DerivedCoreProperties"].lines() {
        let content = line.split('#').next().unwrap().trim();
        if content.is_empty() {
            continue;
        }
        let row: Vec<_> = content.split(';').map(str::trim).collect();
        if row.len() < 2 {
            return Err(bad("DerivedCoreProperties field count"));
        }
        let target = match row[1] {
            "Cased" => &mut cased,
            "Case_Ignorable" => &mut ignorable,
            _ => continue,
        };
        let mut bounds = row[0].split("..");
        let first = cp(bounds.next().unwrap())?;
        let last = bounds.next().map(cp).transpose()?.unwrap_or(first);
        if last < first || bounds.next().is_some() {
            return Err(bad("property range"));
        }
        target.extend(first..=last);
    }
    let mut fold = BTreeMap::<u32, String>::new();
    for line in texts["CaseFolding"].lines() {
        let row: Vec<_> = line
            .split('#')
            .next()
            .unwrap()
            .split(';')
            .map(str::trim)
            .collect();
        if row.len() > 2 && matches!(row[1], "C" | "F") {
            fold.insert(cp(row[0])?, mapped(row[2])?);
        }
    }
    let provenance=FILES.iter().map(|(name,digest)|json!({"url":format!("https://www.unicode.org/Public/{VERSION}/ucd/{name}.txt"),"sha256":digest})).collect::<Vec<_>>();
    // The historical producer labels remain byte-identical provenance in this v1
    // carrier. The maintained generator itself is this Rust command.
    let mut lines = vec![
        "// Generated by build_native_unicode.py. Do not edit. Unicode data: UNICODE-LICENSE.txt."
            .to_owned(),
        format!("export const nativeUnicodeVersion = \"{VERSION}\";"),
        "export const nativeUnicodeAlgorithm = \"tos-python-native-unicode-v1\";".to_owned(),
        format!(
            "export const nativeUnicodeSources = {} as const;",
            ascii_json(&json!(provenance))?
        ),
    ];
    for (name, rows) in [
        ("nativeLowerMappings", lower),
        ("nativeCasefoldMappings", fold),
        ("nativeDecimalMappings", decimal),
    ] {
        lines.push(format!(
            "export const {name}: ReadonlyArray<readonly [number, string]> = {};",
            ascii_json(&json!(rows.into_iter().collect::<Vec<_>>()))?
        ));
    }
    for (name, points) in [
        ("nativeCasedRanges", cased),
        ("nativeCaseIgnorableRanges", ignorable),
        ("nativePrintableRanges", printable),
    ] {
        lines.push(format!(
            "export const {name}: ReadonlyArray<readonly [number, number]> = {};",
            ascii_json(&json!(ranges(&points)))?
        ));
    }
    let result = lines.join("\n") + "\n";
    if Digest256::of_bytes(result.as_bytes()).to_hex() != TABLE_SHA {
        return Err(bad(
            "generated Unicode v1 carrier differs from independent pinned table",
        ));
    }
    Ok(result)
}
fn export(source: &str, name: &str) -> Result<Value> {
    let prefix = format!("export const {name}");
    let line = source
        .lines()
        .find(|line| line.starts_with(&prefix))
        .ok_or_else(|| bad("Unicode table absent"))?;
    let value = line
        .split_once(" = ")
        .ok_or_else(|| bad("Unicode table syntax"))?
        .1;
    Ok(serde_json::from_str(
        value.trim_end_matches(';').trim_end_matches(" as const"),
    )?)
}
fn rust_string(value: &str) -> String {
    let mut result = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' | '\\' => {
                result.push('\\');
                result.push(ch);
            }
            ch if (ch as u32) < 32 || ch == '\u{7f}' => {
                result.push_str(&format!("\\u{{{:x}}}", ch as u32))
            }
            ch => result.push(ch),
        }
    }
    result.push('"');
    result
}
fn rust_table(raw: &[u8]) -> Result<String> {
    if Digest256::of_bytes(raw).to_hex() != TABLE_SHA {
        return Err(bad("owner Unicode table digest differs"));
    }
    let source = std::str::from_utf8(raw)?;
    if export(source, "nativeUnicodeVersion")? != VERSION
        || export(source, "nativeUnicodeAlgorithm")? != "tos-python-native-unicode-v1"
    {
        return Err(bad("Unicode owner profile differs"));
    }
    let url = "https://www.unicode.org/Public/16.0.0/ucd/CaseFolding.txt";
    if !export(source, "nativeUnicodeSources")?
        .as_array()
        .ok_or_else(|| bad("provenance array"))?
        .contains(&json!({"url":url,"sha256":FILES[3].1}))
    {
        return Err(bad("casefold provenance differs"));
    }
    let mut lines = vec![
        "// Generated by generate_unicode.py from owner-pinned native-unicode.generated.ts."
            .to_owned(),
        format!("// Source SHA-256: {TABLE_SHA}"),
        "// Unicode 16.0.0; algorithm tos-python-native-unicode-v1.".to_owned(),
        format!("// Full default casefold: {url}"),
        format!("// CaseFolding source SHA-256: {}", FILES[3].1),
        "// Unicode data license: access/shared/UNICODE-LICENSE.txt.".to_owned(),
    ];
    for (name, key) in [
        ("LOWER", "nativeLowerMappings"),
        ("CASEFOLD", "nativeCasefoldMappings"),
    ] {
        lines.push(format!("const {name}: &[(u32, &str)] = &["));
        for row in export(source, key)?
            .as_array()
            .ok_or_else(|| bad("mapping array"))?
        {
            lines.push(format!(
                "    ({}, {}),",
                row[0].as_u64().ok_or_else(|| bad("mapping codepoint"))?,
                rust_string(row[1].as_str().ok_or_else(|| bad("mapping string"))?)
            ));
        }
        lines.push("];".to_owned());
    }
    for (name, key) in [
        ("CASED", "nativeCasedRanges"),
        ("CASE_IGNORABLE", "nativeCaseIgnorableRanges"),
    ] {
        lines.push(format!("const {name}: &[(u32, u32)] = &["));
        for row in export(source, key)?
            .as_array()
            .ok_or_else(|| bad("range array"))?
        {
            lines.push(format!("    ({}, {}),", row[0], row[1]));
        }
        lines.push("];".to_owned());
    }
    Ok(lines.join("\n") + "\n")
}
fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let mut root = None;
    let mut ucd = None;
    let mut output = None;
    let mut rust_output = None;
    let mut check = false;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--help" | "-h") => {
                println!(
                    "usage: tos-unicode-tables --root ROOT [--ucd-dir DIR] [--output TS] [--rust-output RS] [--check]\nWith --ucd-dir, build both carriers from pinned UCD. Otherwise convert the pinned TS carrier to Rust."
                );
                return Ok(());
            }
            Some("--root") => root = args.next().map(PathBuf::from),
            Some("--ucd-dir") => ucd = args.next().map(PathBuf::from),
            Some("--output") => output = args.next().map(PathBuf::from),
            Some("--rust-output") => rust_output = args.next().map(PathBuf::from),
            Some("--check") => check = true,
            _ => return Err(bad("unknown Unicode generator argument")),
        }
    }
    let root = root.ok_or_else(|| bad("--root is required"))?;
    let output = output.unwrap_or_else(|| root.join("access/shared/native-unicode.generated.ts"));
    let rust_output = rust_output
        .unwrap_or_else(|| root.join("rust/crates/tos-foundation/src/unicode_generated.rs"));
    let generated = ucd.as_deref().map(table).transpose()?;
    let raw = match &generated {
        Some(text) => text.as_bytes().to_vec(),
        None => fs::read(&output)?,
    };
    let rust = rust_table(&raw)?;
    if check {
        if let Some(text) = generated {
            if fs::read(&output)? != text.as_bytes() {
                return Err(bad("Unicode TS companion differs"));
            }
        }
        if fs::read(&rust_output)? != rust.as_bytes() {
            return Err(bad("Unicode Rust companion differs"));
        }
    } else {
        if let Some(text) = generated {
            fs::write(&output, text)?;
        }
        fs::write(&rust_output, rust)?;
    }
    println!("Unicode 16.0.0: exact pinned TS and Rust carriers verified");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("tos-unicode-tables: {error}");
        std::process::exit(2);
    }
}
