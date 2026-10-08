//! Explicit retained laboratory evidence selection; no provider invocation.
use std::{collections::BTreeMap,io::Write,path::PathBuf};
use tos_compiler::{lexical_derivatives::morphology_result as census,research_execution::ResearchExecution};
const HELP:&str="tos zarathustra-morphology-census-result --source-root ABS --inspect-raw ABS [--max-seconds 1..600]\n  OR --source-root ABS --artifact-root ABS --run EXPERIMENT/RUN/variant-A\n     --runtime-manifest ABS --runner ABS --source-packet ABS --local-output-root ABS\n     --check|--build [--generation NAME --scratch-bytes RESERVED_BYTES]\n\nRecompute all source-free census aggregates, or verify the complete retained run and record a result. Explicit files may be restored copies; their exact execution digests remain mandatory. No provider or runtime is executed. Historical check preserves original provenance. Build requires a separate output root and distinct generation; matching replay preserves the receipt and conflicts refuse.\n";
pub fn run_if_requested(args:&[String],out:&mut dyn Write,err:&mut dyn Write)->Option<i32>{
    if args.first().map(String::as_str)!=Some("zarathustra-morphology-census-result"){return None;}
    if args.len()==2&&matches!(args[1].as_str(),"--help"|"-h"){return Some(if out.write_all(HELP.as_bytes()).is_ok(){0}else{2});}
    let result=(||->Result<serde_json::Value,String>{
        let mut options=BTreeMap::new();let mut mode=None;let mut it=args.iter().skip(1);
        while let Some(key)=it.next(){if matches!(key.as_str(),"--check"|"--build"){if mode.replace(key=="--build").is_some(){return Err("choose one action".into());}continue;}
            if !["--source-root","--inspect-raw","--artifact-root","--run","--runtime-manifest","--runner","--source-packet","--local-output-root","--generation","--scratch-bytes","--max-seconds"].contains(&key.as_str()){return Err(format!("unknown option {key}"));}
            let value=it.next().ok_or("option value required")?;if options.insert(key.as_str(),value.as_str()).is_some(){return Err(format!("duplicate option {key}"));}}
        let get=|key:&str|options.get(key).copied().ok_or_else(||format!("explicit {key} required"));
        let path=|key:&str|->Result<PathBuf,String>{let p=PathBuf::from(get(key)?);if !p.is_absolute(){return Err(format!("absolute {key} required"));}Ok(p)};
        let root=path("--source-root")?;let seconds=options.get("--max-seconds").unwrap_or(&"180").parse().map_err(|_|"invalid seconds")?;
        let ctx=if mode==Some(true){ResearchExecution::new_with_scratch(&root,seconds,get("--scratch-bytes")?.parse().map_err(|_|"invalid scratch bytes")?)?}else{if options.contains_key("--scratch-bytes"){return Err("read does not reserve output bytes".into());}ResearchExecution::new(&root,seconds)?};
        if options.contains_key("--inspect-raw"){if mode.is_some()||options.keys().any(|k|!["--source-root","--inspect-raw","--max-seconds"].contains(k)){return Err("raw inspection accepts only explicit source root, raw file and time bound".into());}return census::inspect(&ctx,&path("--inspect-raw")?);}
        let artifact=path("--artifact-root")?;let runtime=path("--runtime-manifest")?;let runner=path("--runner")?;let packet=path("--source-packet")?;let output=path("--local-output-root")?;
        census::run(&ctx,census::Options{build:mode.ok_or("action required")?,artifact_root:&artifact,run:get("--run")?,runtime_manifest:&runtime,runner:&runner,source_packet:&packet,output_root:&output,generation:options.get("--generation").copied()})
    })();
    Some(match result{Ok(v)=>if writeln!(out,"{}",serde_json::to_string_pretty(&v).unwrap()).is_ok(){0}else{2},Err(e)=>{let _=writeln!(err,"zarathustra-morphology-census-result refused: {e}");1}})
}
