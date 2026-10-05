//! Explicit reviewed-fragment assembly. Source and rights judgment stays with owners.
use serde_json::{Value as V, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read},
};
use tos_foundation::Digest256;
fn hash(b: &[u8]) -> String {
    Digest256::of_bytes(b).to_hex()
}
fn sorted(v: &V) -> V {
    match v {
        V::Object(fields) => {
            let ordered: BTreeMap<_, _> =
                fields.iter().map(|(k, v)| (k.clone(), sorted(v))).collect();
            V::Object(ordered.into_iter().collect())
        }
        V::Array(items) => V::Array(items.iter().map(sorted).collect()),
        _ => v.clone(),
    }
}
fn encode(v: &V) -> Vec<u8> {
    let mut b = serde_json::to_vec_pretty(&sorted(v)).unwrap();
    b.push(b'\n');
    b
}
fn fail(s: impl Into<String>) -> Result<V, String> {
    Err(s.into())
}
fn assemble(req: &V) -> Result<V, String> {
    let source = &req["source"];
    let passages = req["passages"].as_array().ok_or("Missing passages")?;
    let bindings = req["bindings"].as_array().ok_or("Missing bindings")?;
    let mut byid = BTreeMap::new();
    for p in passages {
        let id = p["id"].as_str().ok_or("Missing passage ID")?;
        if byid.insert(id, p).is_some() {
            return fail("Duplicate passage IDs");
        }
    }
    let mut bynode = BTreeMap::new();
    for b in bindings {
        let id = b["nodeId"].as_str().ok_or("Missing material binding")?;
        if bynode.insert(id, b).is_some() {
            return fail("Duplicate material bindings");
        }
        let ids = b["passageIds"]
            .as_array()
            .ok_or("Missing passage bindings")?;
        if ids.is_empty()
            || ids
                .iter()
                .any(|id| !byid.contains_key(id.as_str().unwrap_or("")))
        {
            return fail(format!("Unresolved passage binding: {id}"));
        }
    }
    for p in passages {
        match p["status"].as_str() {
            Some("available") => {
                let versions = p["versions"]
                    .as_object()
                    .ok_or("A displayed source unit must be complete and bilingual")?;
                if p["complete"] != true
                    || !versions.contains_key("ru")
                    || !versions.contains_key("en")
                {
                    return fail("A displayed source unit must be complete and bilingual");
                }
                let original = p["originalLanguage"].as_str();
                if p.get("originalLanguage").is_some()
                    && (!original.is_some_and(|s| {
                        (2..=3).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_lowercase())
                    }) || !versions.contains_key(original.unwrap_or("")))
                {
                    return fail("Missing or invalid original-language version");
                }
                if versions
                    .keys()
                    .any(|k| k != "ru" && k != "en" && Some(k.as_str()) != original)
                {
                    return fail("An additional version must declare its original language");
                }
                for (code, v) in versions {
                    let ps = v["paragraphs"].as_array().ok_or("Missing paragraphs")?;
                    let parts: Result<Vec<_>, _> = ps
                        .iter()
                        .map(|p| p.as_str().ok_or("Invalid paragraph"))
                        .collect();
                    let text = parts?.join("\n\n");
                    if text.trim().is_empty()
                        || hash(text.as_bytes()) != v["textSha256"].as_str().unwrap_or("")
                    {
                        return fail(format!(
                            "Text digest mismatch: {}/{code}",
                            p["id"].as_str().unwrap_or("")
                        ));
                    }
                    let uses = v["rights"]["uses"]
                        .as_array()
                        .ok_or("Video display basis is missing")?;
                    if !["local-reading", "video-display"]
                        .iter()
                        .all(|u| uses.iter().any(|v| v.as_str() == Some(u)))
                    {
                        return fail(format!(
                            "Video display basis is missing: {}/{code}",
                            p["id"].as_str().unwrap_or("")
                        ));
                    }
                }
            }
            Some("link-only") => {
                if p.get("versions").is_some() {
                    return fail("Unavailable passages cannot contain hidden text");
                }
            }
            _ => return fail("Unknown passage status"),
        }
    }
    let catalog = json!({"schema":"tos_demo_fragments_v1","audience":"local-reading-and-recorded-video","passages":passages,"bindings":bindings});
    let bytes = encode(&catalog);
    let digest = hash(&bytes);
    let catalog_ref =
        json!({"path":format!("assets/fragments-{}.json",&digest[..16]),"sha256":digest});
    let originals: BTreeMap<_, _> = source["nodes"]
        .as_array()
        .ok_or("Missing source nodes")?
        .iter()
        .map(|n| (n["id"].as_str().unwrap_or(""), n))
        .collect();
    let ids = [
        "work",
        "chapter-p3.r2",
        "moment",
        "chapter-p3.r13",
        "all-things",
        "same-life",
        "dossier",
    ];
    let mut needed: BTreeSet<&str> = ids.iter().copied().collect();
    for id in ids {
        if !originals.contains_key(id) || !bynode.contains_key(id) {
            return fail(format!(
                "Source navigation or fragment binding is missing: {id}"
            ));
        }
        let mut parent = &originals[id]["parentId"];
        let mut seen = BTreeSet::from([id]);
        while !parent.is_null() {
            let p = parent.as_str().ok_or("Invalid source parent closure")?;
            if !originals.contains_key(p) || !seen.insert(p) {
                return fail("Invalid source parent closure");
            }
            needed.insert(p);
            parent = &originals[p]["parentId"];
        }
    }
    let mut nodes = Vec::new();
    for id in needed {
        let original = originals[id];
        let binding = bynode.get(id);
        let mut refs = Vec::new();
        if let Some(binding) = binding {
            for pid in binding["passageIds"].as_array().unwrap() {
                let p = byid[pid.as_str().unwrap()];
                if p["status"] == "available" {
                    let versions = p["versions"].as_object().unwrap();
                    let mut codes = vec!["ru", "en"];
                    codes.extend(
                        versions
                            .keys()
                            .map(String::as_str)
                            .filter(|c| *c != "ru" && *c != "en"),
                    );
                    for c in codes {
                        let title = p["title"][c]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .or(p["title"]["en"].as_str())
                            .ok_or("Missing passage title")?;
                        refs.push(json!({"label":format!("{title} · {}",c.to_uppercase()),"ref":versions[c]["sourceUrl"]}));
                    }
                } else {
                    for l in p["links"].as_array().ok_or("Missing source links")? {
                        refs.push(json!({"label":l["label"],"ref":l["url"]}));
                    }
                }
            }
        }
        nodes.push(json!({"id":id,"kind":original["kind"],"parentId":original["parentId"],"title":original["title"],"body":binding.map(|b|b["context"].clone()).unwrap_or(json!({"ru":"Часть III книги.","en":"Part III of the book."})),"sourceRefs":refs,"sourceNote":{"ru":"Полные разделы в читалке подписаны именами авторов и переводчиков и точными изданиями. Оригинал и переводы сохраняют собственные границы абзацев.","en":"The reader credits each complete section to its author, translator and specific edition. Original texts and translations retain their own paragraph divisions."}}));
    }
    let mut library = json!({"schema":"tos_constructor_library_v1","rootId":source["rootId"],"nodes":nodes,"fragmentCatalog":catalog_ref,"displayProfile":"recorded-demo-selected-editions-v1"});
    library["fingerprint"] = json!(hash(&encode(&library)));
    Ok(json!({"catalog":catalog,"catalogText":String::from_utf8(bytes).unwrap(),"library":library}))
}

fn load(path: &std::path::Path) -> Result<(V, V), String> {
    let b = std::fs::read(path).map_err(|e| e.to_string())?;
    if b.len() > 8_000_000 {
        return Err("Input exceeds the 8 MB reader bound".into());
    }
    let v = serde_json::from_slice(&b).map_err(|e| e.to_string())?;
    let receipt = json!({"name":path.file_name().and_then(|n|n.to_str()).ok_or("Invalid input filename")?,"sha256":hash(&b),"bytes":b.len()});
    Ok((v, receipt))
}
fn cli(args: &[String]) -> Result<V, String> {
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    let mut source = None;
    let mut passages_paths = Vec::new();
    let mut bindings_path = None;
    let mut output = None;
    let mut receipt = None;
    let mut index = 0;
    while index < args.len() {
        let key = &args[index];
        index += 1;
        if key == "--passages" {
            let start = index;
            while index < args.len() && !args[index].starts_with("--") {
                passages_paths.push(PathBuf::from(&args[index]));
                index += 1;
            }
            if index == start {
                return Err("--passages requires input paths".into());
            }
            continue;
        }
        let value = args.get(index).ok_or("Missing option value")?;
        index += 1;
        match key.as_str() {
            "--source-library" => source = Some(PathBuf::from(value)),
            "--bindings" => bindings_path = Some(PathBuf::from(value)),
            "--output" => output = Some(PathBuf::from(value)),
            "--receipt" => receipt = Some(PathBuf::from(value)),
            _ => return Err(format!("Unknown option: {key}")),
        }
    }
    let source = source.ok_or("--source-library is required")?;
    let bindings_path = bindings_path.ok_or("--bindings is required")?;
    let output = output.ok_or("--output is required")?;
    let absolute = if output.is_absolute() {
        output
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(output)
    };
    let mut output = PathBuf::new();
    for component in absolute.components() {
        use std::path::Component;
        match component {
            Component::ParentDir => {
                output.pop();
            }
            Component::CurDir => {}
            Component::RootDir => output.push("/"),
            Component::Normal(part) => {
                output.push(part);
                if output.exists() {
                    output = output.canonicalize().map_err(|e| e.to_string())?;
                }
            }
            Component::Prefix(_) => return Err("Unsupported output path prefix".into()),
        }
    }
    let receipt_path = receipt.ok_or("--receipt is required")?;
    if passages_paths.is_empty() {
        return Err("--passages is required".into());
    }
    if output.join("library.json").exists() || output.join("constructor.html").exists() {
        return Err(
            "Refusing to replace an existing release; select a new immutable directory".into(),
        );
    }
    let (source, source_receipt) = load(&source)?;
    let mut passages = Vec::new();
    let mut inputs = Vec::new();
    for path in passages_paths {
        let (data, r) = load(&path)?;
        passages.extend(
            data.as_array()
                .or_else(|| data["passages"].as_array())
                .ok_or("Missing passages")?
                .iter()
                .cloned(),
        );
        inputs.push(r);
    }
    let (data, binding_receipt) = load(&bindings_path)?;
    let bindings = data
        .as_array()
        .or_else(|| data["bindings"].as_array())
        .ok_or("Missing bindings")?;
    let result = assemble(&json!({"source":source,"passages":passages,"bindings":bindings}))?;
    std::fs::create_dir_all(output.join("assets")).map_err(|e| e.to_string())?;
    let catalog_path = result["library"]["fragmentCatalog"]["path"]
        .as_str()
        .ok_or("Missing catalog output path")?;
    std::fs::write(
        output.join(catalog_path),
        result["catalogText"].as_str().unwrap().as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    let library_path = output.join("library.json");
    std::fs::write(&library_path, encode(&result["library"])).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&library_path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    let receipt = json!({"schema":"tos_demo_fragment_assembly_v1","input_library":source_receipt,"inputs":inputs,"bindings_input":binding_receipt,"output":output.to_str().ok_or("Invalid output path")?,"catalog":result["library"]["fragmentCatalog"],"source_navigation_count":result["library"]["nodes"].as_array().unwrap().len(),"material_binding_count":bindings.len(),"available_passages":passages.iter().filter(|p|p["status"]=="available").map(|p|p["id"].clone()).collect::<Vec<_>>(),"link_only_passages":passages.iter().filter(|p|p["status"]=="link-only").map(|p|p["id"].clone()).collect::<Vec<_>>(),"historical_private_payload_copied":false,"corpus_modified":false,"limits":"Mechanical assembly only; source quality and rights reasoning require their separate review records."});
    if let Some(parent) = receipt_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(receipt_path, encode(&receipt)).map_err(|e| e.to_string())?;
    let mut summary = receipt;
    summary.as_object_mut().unwrap().remove("inputs");
    summary.as_object_mut().unwrap().remove("input_library");
    Ok(summary)
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!(
            "usage: tos-constructor-fragments --source-library FILE --passages FILE... --bindings FILE --output FRESH_DIRECTORY --receipt FILE\n       tos-constructor-fragments --assemble (bounded JSON stdin)"
        );
        return;
    }
    let outcome = if args.is_empty() || args == ["--assemble"] {
        (|| {
            let mut raw = Vec::new();
            io::stdin()
                .take(24_000_001)
                .read_to_end(&mut raw)
                .map_err(|e| e.to_string())?;
            if raw.len() > 24_000_000 {
                return Err("Input exceeds the 24 MB assembly bound".into());
            }
            let req: V = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
            assemble(&req)
        })()
    } else {
        cli(&args)
    };
    match outcome {
        Ok(v) => println!("{}", v),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1)
        }
    }
}
