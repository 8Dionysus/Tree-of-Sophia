//! Bounded native entry point for the maintained philosophy projections.
//!
//! Product meaning and rendering remain in `tos-compiler`; this module owns
//! only the standalone operation, filesystem custody, validation routes, and
//! the sequential corpus-view composition.

use crate::philosophy_graph_views;
use crate::route_cards::RouteSources;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::Metadata;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tos_compiler::PhilosophySourceReadProfile;
use tos_compiler::research_execution::ResearchExecution;
use tos_compiler::source_philosophy_atlas::{self, AtlasLimits};
use tos_compiler::source_philosophy_graph::{self, GraphLimits};
use tos_compiler::source_philosophy_multilingual::{self, Multilingual, MultilingualLimits};
use tos_compiler::source_philosophy_post_planting::{self, AuditLimits, AuditPacket};
use tos_compiler::source_philosophy_views::{self, ViewLimits};
use tos_compiler::{Error as CompilerError, Result as CompilerResult};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, canonical_bytes_v1, parse_json,
};

const SOURCE_FILE_BYTES: u64 = 256 * 1024 * 1024;
const JSON_SCHEMA_BYTES: usize = 16 * 1024 * 1024;
const PRODUCT_RENDER_BYTES: usize = 256 * 1024 * 1024;
const OUTPUT_MODE: u32 = 0o644;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Product {
    Atlas,
    Views,
    Graph,
    Audit,
    Corpus,
}

impl Product {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "atlas" => Ok(Self::Atlas),
            "views" => Ok(Self::Views),
            "graph" => Ok(Self::Graph),
            "audit" => Ok(Self::Audit),
            "corpus" => Ok(Self::Corpus),
            _ => Err("philosophy product must be atlas, views, graph, audit, or corpus".into()),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Atlas => "atlas",
            Self::Views => "views",
            Self::Graph => "graph",
            Self::Audit => "audit",
            Self::Corpus => "corpus",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Build,
    Check,
    Validate,
}

impl Mode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "build" => Ok(Self::Build),
            "check" => Ok(Self::Check),
            "validate" => Ok(Self::Validate),
            _ => Err("philosophy product mode must be build, check, or validate".into()),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Check => "check",
            Self::Validate => "validate",
        }
    }
}

struct Options {
    product: Product,
    mode: Mode,
    source_root: PathBuf,
    output_root: PathBuf,
    max_seconds: u64,
    scratch_bytes: u64,
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut product = None;
    let mut mode = None;
    let mut source_root = None;
    let mut output_root = None;
    let mut max_seconds = None;
    let mut scratch_bytes = None;
    let mut index = 0;
    while index < args.len() {
        let option = args[index].as_str();
        index += 1;
        let value = args
            .get(index)
            .ok_or_else(|| format!("missing value for {option}"))?;
        index += 1;
        match option {
            "--philosophy-product" => set_once(&mut product, Product::parse(value)?, option)?,
            "--mode" => set_once(&mut mode, Mode::parse(value)?, option)?,
            "--source-root" => set_once(&mut source_root, PathBuf::from(value), option)?,
            "--output-root" => set_once(&mut output_root, PathBuf::from(value), option)?,
            "--max-seconds" => {
                let number = value.parse::<u64>().map_err(|_| "invalid max-seconds")?;
                set_once(&mut max_seconds, number, option)?;
            }
            "--scratch-bytes" => {
                let number = value.parse::<u64>().map_err(|_| "invalid scratch-bytes")?;
                set_once(&mut scratch_bytes, number, option)?;
            }
            _ => return Err(format!("unknown philosophy product argument: {option}")),
        }
    }
    let source_root = source_root.ok_or("--source-root is required")?;
    let max_seconds = max_seconds.unwrap_or(600);
    let scratch_bytes = scratch_bytes.unwrap_or(256 * 1024 * 1024);
    if !(1..=3600).contains(&max_seconds) {
        return Err("philosophy product max-seconds must be 1..3600".into());
    }
    if scratch_bytes == 0 {
        return Err("philosophy product scratch-bytes must be positive".into());
    }
    let output_root = output_root.unwrap_or_else(|| source_root.clone());
    for (label, path) in [("source-root", &source_root), ("output-root", &output_root)] {
        if !path.is_absolute()
            || path.as_os_str().len() > 4096
            || path
                .components()
                .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
        {
            return Err(format!(
                "--{label} must be a bounded canonical absolute directory path"
            ));
        }
    }
    let product = product.ok_or("--philosophy-product is required")?;
    let mode = mode.unwrap_or(Mode::Build);
    if product == Product::Corpus && mode != Mode::Build {
        return Err("the corpus philosophy batch supports build mode only".into());
    }
    if product == Product::Corpus && source_root != output_root {
        return Err(
            "the corpus philosophy batch must write into its selected private source view".into(),
        );
    }
    Ok(Options {
        product,
        mode,
        source_root,
        output_root,
        max_seconds,
        scratch_bytes,
    })
}

fn set_once<T>(slot: &mut Option<T>, value: T, name: &str) -> Result<(), String> {
    if slot.replace(value).is_some() {
        Err(format!("duplicate philosophy product argument: {name}"))
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileStamp {
    device: u64,
    inode: u64,
    length: u64,
    mode: u32,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}

impl FileStamp {
    fn from_metadata(metadata: &Metadata) -> Result<Self, String> {
        if metadata.file_type().is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
            return Err("philosophy product selected a symlink or non-file input".into());
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            mode: metadata.mode(),
            mtime: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
            ctime: metadata.ctime(),
            ctime_nsec: metadata.ctime_nsec(),
        })
    }

    fn matches_file(&self, metadata: &Metadata) -> bool {
        Self::from_metadata(metadata).is_ok_and(|now| now == *self)
    }

    fn is_file(self) -> bool {
        self.mode & libc::S_IFMT == libc::S_IFREG
    }
}

struct Observation {
    stamp: Option<FileStamp>,
    sha256: Option<String>,
    read_cap: Option<u64>,
}

#[derive(Clone)]
struct EnumerationFence {
    root: String,
    suffix: String,
    direct_files: bool,
    max_refs: usize,
    references: Vec<String>,
}

#[derive(Default)]
struct WorkAccounting {
    measured_adapter_units: u64,
    reserved_native_units: u64,
    source_payload_reads: u64,
    source_payload_bytes: u64,
    output_payload_reads: u64,
    output_payload_bytes: u64,
    source_rechecked_files: u64,
    output_rechecked_files: u64,
    enumerated_entries: u64,
}

struct ProductContext {
    operation: ResearchExecution,
    output: ResearchExecution,
    routes: RouteSources,
    output_routes: RouteSources,
    source_identity: (u64, u64),
    output_identity: (u64, u64),
    observations: BTreeMap<String, Observation>,
    output_observations: BTreeMap<String, Observation>,
    fences: Vec<EnumerationFence>,
    work: WorkAccounting,
}

impl ProductContext {
    fn new(options: &Options, cancelled: Arc<AtomicBool>) -> Result<Self, String> {
        let operation = ResearchExecution::new_philosophy_products_with_cancellation(
            &options.source_root,
            options.max_seconds,
            options.scratch_bytes,
            cancelled,
        )?;
        let mut routes = RouteSources::new_until(&options.source_root, operation.deadline())
            .map_err(|error| error.to_string())?;
        operation.tick(1)?;
        let source_identity = operation.root_identity()?;
        operation.tick(1)?;
        let route_root = routes
            .metadata(".")
            .map_err(|error| error.to_string())?
            .ok_or("philosophy source root disappeared after selection")?;
        if (route_root.dev(), route_root.ino()) != source_identity {
            return Err("philosophy source readers selected different root identities".into());
        }
        routes.verify_root().map_err(|error| error.to_string())?;
        operation.check()?;
        operation.tick(1)?;
        let output =
            operation.select_output_directory(&options.output_root, options.mode == Mode::Build)?;
        operation.tick(1)?;
        let output_identity = output.root_identity()?;
        let mut output_routes = RouteSources::new_until(&options.output_root, operation.deadline())
            .map_err(|error| error.to_string())?;
        operation.tick(1)?;
        let output_route_root = output_routes
            .metadata(".")
            .map_err(|error| error.to_string())?
            .ok_or("philosophy output root disappeared after selection")?;
        if (output_route_root.dev(), output_route_root.ino()) != output_identity {
            return Err("philosophy output readers selected different root identities".into());
        }
        output_routes
            .verify_root()
            .map_err(|error| error.to_string())?;
        if options.output_root == options.source_root && output_identity != source_identity {
            return Err("philosophy output root differs from the selected source root".into());
        }
        let mut work = WorkAccounting::default();
        work.measured_adapter_units = 5;
        Ok(Self {
            operation,
            output,
            routes,
            output_routes,
            source_identity,
            output_identity,
            observations: BTreeMap::new(),
            output_observations: BTreeMap::new(),
            fences: Vec::new(),
            work,
        })
    }

    fn check(&self) -> Result<(), String> {
        self.operation.check()?;
        self.routes.check().map_err(|error| error.to_string())?;
        self.output_routes
            .check()
            .map_err(|error| error.to_string())
    }

    fn io_check(&self) -> io::Result<()> {
        self.check().map_err(io::Error::other)
    }

    fn measured_tick(&mut self, units: u64) -> Result<(), String> {
        self.operation.tick(units)?;
        self.work.measured_adapter_units = self
            .work
            .measured_adapter_units
            .checked_add(units)
            .ok_or("philosophy adapter work counter overflow")?;
        Ok(())
    }

    fn reserve_native_work(&mut self, units: u64) -> Result<(), String> {
        self.operation.tick(units)?;
        self.work.reserved_native_units = self
            .work
            .reserved_native_units
            .checked_add(units)
            .ok_or("philosophy native work reservation overflow")?;
        Ok(())
    }

    fn validate_reference(reference: &str) -> Result<(), String> {
        let path = Path::new(reference);
        if !reference.starts_with("ToS/")
            || reference.contains('\\')
            || reference.contains('\0')
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err("philosophy product path is outside the ToS-owned tree".into());
        }
        Ok(())
    }

    fn exists(&mut self, reference: &str) -> Result<bool, String> {
        let mut access = self.access();
        access.exists(reference)
    }

    fn read_path(&mut self, reference: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
        let mut access = self.access();
        access.read(reference, max_bytes)
    }

    fn current_value(&mut self, reference: &str, max_bytes: usize) -> Result<Value, String> {
        let raw = self.read_path(reference, max_bytes)?;
        let value = serde_json::from_slice::<Value>(&raw)
            .map_err(|error| format!("parse {reference}: {error}"))?;
        if !value.is_object() {
            return Err(format!("{reference} must contain a JSON object"));
        }
        self.operation.check()?;
        Ok(value)
    }

    fn parse_authored_object(raw: &[u8], path: &str, max_bytes: usize) -> Result<Value, String> {
        let limits =
            JsonLimits::new(max_bytes, 96, 2_000_000, 4096).map_err(|error| error.to_string())?;
        let document = parse_json(raw, JsonMode::RequestLastWins, limits)
            .map_err(|error| format!("parse authored {path}: {error}"))?;
        let normalized = canonical_bytes_v1(
            document.root(),
            CanonicalProfile::SourceRecordDigestV1,
            limits,
        )
        .map_err(|error| error.to_string())?;
        let value = serde_json::from_slice::<Value>(&normalized)
            .map_err(|error| format!("decode authored {path}: {error}"))?;
        if !value.is_object() {
            return Err(format!("{path} must contain a JSON object"));
        }
        Ok(value)
    }

    fn authored_object(&mut self, reference: &str, max_bytes: usize) -> Result<Value, String> {
        let raw = self.read_path(reference, max_bytes)?;
        Self::parse_authored_object(&raw, reference, max_bytes)
    }

    fn scan_refs(
        &mut self,
        root: &str,
        suffix: &str,
        max_refs: usize,
        direct_files: bool,
    ) -> Result<Vec<String>, String> {
        let mut access = self.access();
        let (references, _) = access.scan(root, suffix, max_refs, direct_files)?;
        Ok(references)
    }

    fn view_refs(&mut self) -> Result<Vec<String>, String> {
        let references = self.scan_refs(
            source_philosophy_views::VIEW_ROOT,
            ".graph.md",
            AtlasLimits::default().max_nodes,
            true,
        )?;
        self.fences.push(EnumerationFence {
            root: source_philosophy_views::VIEW_ROOT.into(),
            suffix: ".graph.md".into(),
            direct_files: true,
            max_refs: AtlasLimits::default().max_nodes,
            references: references.clone(),
        });
        Ok(references)
    }

    fn finish_source_fences(&mut self) -> Result<(), String> {
        self.check()?;
        let fences = self.fences.clone();
        {
            let mut access = self.access();
            access.finish_observations()?;
            for fence in fences {
                let (current, _) = access.scan(
                    &fence.root,
                    &fence.suffix,
                    fence.max_refs,
                    fence.direct_files,
                )?;
                if current != fence.references {
                    return Err(format!(
                        "philosophy source enumeration changed: {}",
                        fence.root
                    ));
                }
            }
        }
        self.output_access().finish_observations()?;
        self.verify_roots()
    }

    fn verify_roots(&mut self) -> Result<(), String> {
        self.check()?;
        self.measured_tick(1)?;
        let source = self.operation.root_identity()?;
        self.measured_tick(1)?;
        let route = self
            .routes
            .metadata(".")
            .map_err(|error| error.to_string())?
            .ok_or("philosophy source root disappeared")?;
        if source != self.source_identity || (route.dev(), route.ino()) != self.source_identity {
            return Err("philosophy source root identity changed".into());
        }
        self.routes
            .verify_root()
            .map_err(|error| error.to_string())?;
        self.measured_tick(1)?;
        let output = self.output.root_identity()?;
        self.measured_tick(1)?;
        let output_route = self
            .output_routes
            .metadata(".")
            .map_err(|error| error.to_string())?
            .ok_or("philosophy output root disappeared")?;
        if output != self.output_identity
            || (output_route.dev(), output_route.ino()) != self.output_identity
        {
            return Err("philosophy output root identity changed".into());
        }
        self.output_routes
            .verify_root()
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn render_compact(&mut self, value: &Value, max_bytes: usize) -> Result<Vec<u8>, String> {
        let mut check = || self.io_check();
        let mut bytes = philosophy_graph_views::render_payload(value, max_bytes, &mut check)
            .map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        if bytes.len() > max_bytes {
            return Err("philosophy compact output exceeded its byte bound".into());
        }
        self.measured_tick((bytes.len() as u64).div_ceil(65_536))?;
        Ok(bytes)
    }

    fn current_raw(&mut self, reference: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
        if !Self::is_output_reference(reference) {
            return Err("philosophy product read left its fixed generated references".into());
        }
        self.output_access().read(reference, max_bytes)
    }

    fn write_output(&mut self, reference: &str, bytes: &[u8]) -> Result<(), String> {
        Self::validate_reference(reference)?;
        if !Self::is_output_reference(reference) {
            return Err("philosophy product write left its fixed generated references".into());
        }
        let units = (bytes.len() as u64).div_ceil(65_536).saturating_add(1);
        self.measured_tick(units)?;
        self.operation.check()?;
        self.output
            .write(reference, bytes, OUTPUT_MODE, false)
            .map_err(|error| format!("write {reference}: {error}"))
    }

    fn is_output_reference(reference: &str) -> bool {
        [
            source_philosophy_views::ATLAS_REF,
            source_philosophy_graph::VIEWS_REF,
            source_philosophy_graph::GRAPH_REF,
            source_philosophy_post_planting::AUDIT_JSON_REF,
            source_philosophy_post_planting::AUDIT_MARKDOWN_REF,
        ]
        .contains(&reference)
    }

    fn report(
        &self,
        product: Product,
        mode: Mode,
        outputs: &[&str],
        builder_limits: Value,
    ) -> Value {
        json!({
            "producer": "tos-ops-mechanics-plan native philosophy product adapter",
            "product": product.name(),
            "product_scope": if product == Product::Corpus {
                "native sequential atlas -> views -> graph batch; does not claim the whole corpus worker is native"
            } else {
                "one maintained native ToS philosophy product"
            },
            "mode": mode.name(),
            "status": match mode { Mode::Build => "written", Mode::Check => "matched", Mode::Validate => "validated" },
            "output_refs": outputs,
            "read_profile": {
                "source_inputs": "read-only ToS authored members and, for standalone views/graph/audit, the maintained selected source-root projections",
                "source_decoding_profile": "LegacyPythonJsonLoads consumer-side JSON representation where selected; original payload bytes and source-file SHA-256 custody remain raw",
                "current_output_reads": "check and validate read fixed product refs from the selected output root and recheck them through the same bounded operation",
                "source_payload_reads": self.work.source_payload_reads,
                "source_payload_bytes_returned": self.work.source_payload_bytes,
                "selected_output_product_reads": self.work.output_payload_reads,
                "selected_output_product_bytes_returned": self.work.output_payload_bytes,
                "source_final_sha256_rechecks": self.work.source_rechecked_files,
                "output_final_sha256_rechecks": self.work.output_rechecked_files,
                "enumerated_entries": self.work.enumerated_entries,
                "route_lookups": self.routes.operation_count(),
                "route_root_component_opens": self.routes.root_component_open_count(),
                "output_route_lookups": self.output_routes.operation_count(),
                "output_route_root_component_opens": self.output_routes.root_component_open_count()
            },
            "work_accounting": {
                "shared_measured_adapter_units": self.work.measured_adapter_units,
                "shared_measured_adapter_units_scope": "root and path selections, enumeration entries, returned payload block estimates, final hash block estimates, compact output block estimates, and selected output write block estimates; this is not a count of compiler parser or validator loop iterations",
                "shared_conservative_native_reservations": self.work.reserved_native_units,
                "native_builder_loops": "each existing compiler builder retains and enforces its own fixed local work or material limits; private loop counts are not reported as shared measured units",
                "accounting_scope": "the same bounded philosophy-product execution owns input reads, output writes, deadlines, cancellation, adapter selections, scan/render block debits, and any declared conservative native reservation"
            },
            "builder_limits": builder_limits,
            "shared_execution": self.operation.budget_report()
        })
    }
}

struct Access<'a> {
    operation: &'a ResearchExecution,
    routes: &'a mut RouteSources,
    observations: &'a mut BTreeMap<String, Observation>,
    fences: &'a mut Vec<EnumerationFence>,
    work: &'a mut WorkAccounting,
    output: bool,
}

impl Access<'_> {
    fn read(&mut self, reference: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
        let stamp = ProductContext::route_metadata_parts(
            self.operation,
            self.routes,
            self.observations,
            self.work,
            reference,
        )?
        .ok_or_else(|| format!("philosophy input is missing: {reference}"))?;
        if !stamp.is_file() {
            return Err(format!(
                "philosophy input is not a regular file: {reference}"
            ));
        }
        let cap = u64::try_from(max_bytes).map_err(|_| "philosophy read cap overflow")?;
        self.operation
            .tick(stamp.length.div_ceil(65_536).saturating_add(1))?;
        self.work.measured_adapter_units = self
            .work
            .measured_adapter_units
            .checked_add(stamp.length.div_ceil(65_536).saturating_add(1))
            .ok_or("philosophy adapter work counter overflow")?;
        let mut file = self
            .operation
            .source_file(reference, cap)
            .map_err(|error| format!("open {reference}: {error}"))?;
        let before = file.metadata().map_err(|error| error.to_string())?;
        if !stamp.matches_file(&before) {
            return Err(format!("philosophy input changed before read: {reference}"));
        }
        let raw = self
            .operation
            .read_file(&mut file, cap)
            .map_err(|error| format!("read {reference}: {error}"))?;
        self.operation
            .verify_file_unchanged(&file, &before)
            .map_err(|error| format!("verify {reference}: {error}"))?;
        let digest = Digest256::of_bytes(&raw).to_hex();
        match self.observations.get_mut(reference) {
            Some(observation) => {
                if observation.stamp != Some(stamp)
                    || observation
                        .sha256
                        .as_ref()
                        .is_some_and(|old| old != &digest)
                {
                    return Err(format!("philosophy input bytes drifted: {reference}"));
                }
                observation.sha256 = Some(digest);
                observation.read_cap = Some(observation.read_cap.map_or(cap, |old| old.min(cap)));
            }
            None => {
                self.observations.insert(
                    reference.to_owned(),
                    Observation {
                        stamp: Some(stamp),
                        sha256: Some(digest),
                        read_cap: Some(cap),
                    },
                );
            }
        }
        if self.output {
            self.work.output_payload_reads = self.work.output_payload_reads.saturating_add(1);
            self.work.output_payload_bytes = self
                .work
                .output_payload_bytes
                .checked_add(raw.len() as u64)
                .ok_or("philosophy output byte counter overflow")?;
        } else {
            self.work.source_payload_reads = self.work.source_payload_reads.saturating_add(1);
            self.work.source_payload_bytes = self
                .work
                .source_payload_bytes
                .checked_add(raw.len() as u64)
                .ok_or("philosophy source byte counter overflow")?;
        }
        self.operation.check()?;
        Ok(raw)
    }

    fn exists(&mut self, reference: &str) -> Result<bool, String> {
        if !reference.starts_with("ToS/philosophy/") {
            return Err("philosophy branch existence query left ToS/philosophy".into());
        }
        Ok(ProductContext::route_metadata_parts(
            self.operation,
            self.routes,
            self.observations,
            self.work,
            reference,
        )?
        .is_some())
    }

    fn enumerate(
        &mut self,
        root: &str,
        suffix: &str,
        max_refs: usize,
    ) -> Result<Vec<String>, String> {
        let (references, _) = self.scan(root, suffix, max_refs, false)?;
        self.fences.push(EnumerationFence {
            root: root.into(),
            suffix: suffix.into(),
            direct_files: false,
            max_refs,
            references: references.clone(),
        });
        Ok(references)
    }

    fn compiler_read(&mut self, reference: &str, max_bytes: usize) -> CompilerResult<Vec<u8>> {
        self.read(reference, max_bytes)
            .map_err(CompilerError::Source)
    }

    fn scan(
        &mut self,
        root: &str,
        suffix: &str,
        max_refs: usize,
        direct_files: bool,
    ) -> Result<(Vec<String>, usize), String> {
        let allowed_suffix = if direct_files {
            suffix == ".graph.md"
        } else {
            suffix == "graph-workbench/pre-canon-summary.json"
        };
        if !root.starts_with("ToS/philosophy/") || !allowed_suffix {
            return Err("philosophy enumeration escaped its source-owned root".into());
        }
        self.operation.check()?;
        self.operation.tick(1)?;
        self.work.measured_adapter_units = self
            .work
            .measured_adapter_units
            .checked_add(1)
            .ok_or("philosophy adapter work counter overflow")?;
        let prefix = format!("{root}/");
        let selected = self
            .routes
            .selected_paths(root, &|path, is_directory| {
                if path == root {
                    return is_directory;
                }
                let Some(relative) = path.strip_prefix(&prefix) else {
                    return false;
                };
                if direct_files {
                    !relative.contains('/') && relative.ends_with(suffix)
                } else {
                    is_directory || relative.ends_with(suffix)
                }
            })
            .map_err(|error| format!("enumerate {root}: {error}"))?;
        self.operation.check()?;
        self.operation
            .tick(selected.len() as u64)
            .map_err(|error| error.to_string())?;
        self.work.measured_adapter_units = self
            .work
            .measured_adapter_units
            .checked_add(selected.len() as u64)
            .ok_or("philosophy adapter work counter overflow")?;
        let selected_len = selected.len();
        let mut references = selected
            .into_iter()
            .filter(|path| {
                let Some(relative) = path.strip_prefix(&prefix) else {
                    return false;
                };
                relative.ends_with(suffix) && (!direct_files || !relative.contains('/'))
            })
            .collect::<Vec<_>>();
        if references.len() > max_refs {
            return Err("philosophy enumeration reference bound exceeded".into());
        }
        references.sort();
        for reference in &references {
            ProductContext::route_metadata_parts(
                self.operation,
                self.routes,
                self.observations,
                self.work,
                reference,
            )?;
        }
        self.routes
            .verify_root()
            .map_err(|error| error.to_string())?;
        self.work.enumerated_entries = self
            .work
            .enumerated_entries
            .checked_add(selected_len as u64)
            .ok_or("philosophy enumeration counter overflow")?;
        Ok((references, selected_len))
    }

    fn finish_observations(&mut self) -> Result<(), String> {
        let observations = self
            .observations
            .iter()
            .map(|(path, observation)| {
                (
                    path.clone(),
                    observation.stamp,
                    observation.sha256.clone(),
                    observation.read_cap,
                )
            })
            .collect::<Vec<_>>();
        for (reference, initial_stamp, expected_sha, cap) in observations {
            let current_stamp = ProductContext::route_metadata_parts(
                self.operation,
                self.routes,
                self.observations,
                self.work,
                &reference,
            )?;
            if current_stamp != initial_stamp {
                return Err(format!("philosophy input metadata drifted: {reference}"));
            }
            if let (Some(stamp), Some(expected_sha)) = (initial_stamp, expected_sha) {
                let limit = cap.unwrap_or(SOURCE_FILE_BYTES);
                let mut file = self
                    .operation
                    .source_file(&reference, limit)
                    .map_err(|error| format!("reopen {reference}: {error}"))?;
                let before = file.metadata().map_err(|error| error.to_string())?;
                if !stamp.matches_file(&before) {
                    return Err(format!(
                        "philosophy input changed before final hash: {reference}"
                    ));
                }
                let units = stamp.length.div_ceil(65_536).saturating_add(1);
                self.operation.tick(units)?;
                self.work.measured_adapter_units = self
                    .work
                    .measured_adapter_units
                    .checked_add(units)
                    .ok_or("philosophy adapter work counter overflow")?;
                let observed_sha = self
                    .operation
                    .hash_file(&mut file, limit)
                    .map_err(|error| format!("rehash {reference}: {error}"))?;
                self.operation
                    .verify_file_unchanged(&file, &before)
                    .map_err(|error| format!("verify final {reference}: {error}"))?;
                if observed_sha != expected_sha {
                    return Err(format!("philosophy input SHA-256 changed: {reference}"));
                }
                if self.output {
                    self.work.output_rechecked_files =
                        self.work.output_rechecked_files.saturating_add(1);
                } else {
                    self.work.source_rechecked_files =
                        self.work.source_rechecked_files.saturating_add(1);
                }
            }
        }
        Ok(())
    }
}

impl ProductContext {
    fn route_metadata_parts(
        operation: &ResearchExecution,
        routes: &mut RouteSources,
        observations: &mut BTreeMap<String, Observation>,
        work: &mut WorkAccounting,
        reference: &str,
    ) -> Result<Option<FileStamp>, String> {
        Self::validate_reference(reference)?;
        operation.tick(1)?;
        work.measured_adapter_units = work
            .measured_adapter_units
            .checked_add(1)
            .ok_or("philosophy adapter work counter overflow")?;
        operation.check()?;
        let metadata = routes
            .metadata(reference)
            .map_err(|error| format!("metadata {reference}: {error}"))?;
        operation.check()?;
        let stamp = metadata
            .as_ref()
            .map(FileStamp::from_metadata)
            .transpose()?;
        match observations.get(reference) {
            Some(observation) if observation.stamp != stamp => Err(format!(
                "philosophy input changed during operation: {reference}"
            )),
            Some(_) => Ok(stamp),
            None => {
                observations.insert(
                    reference.to_owned(),
                    Observation {
                        stamp,
                        sha256: None,
                        read_cap: None,
                    },
                );
                Ok(stamp)
            }
        }
    }
}

fn schema_validator(
    context: &mut ProductContext,
    schema_ref: &str,
) -> Result<jsonschema::Validator, String> {
    let schema_value = context.authored_object(schema_ref, JSON_SCHEMA_BYTES)?;
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .should_validate_formats(false)
        .offline()
        .build(&schema_value)
        .map_err(|error| format!("build {schema_ref} validator: {error}"))
}

fn schema_check(
    context: &ProductContext,
    validator: &jsonschema::Validator,
    value: &Value,
) -> Result<(), String> {
    let mut first = None;
    for error in validator.iter_errors(value) {
        context.check()?;
        let mut current = value;
        let mut order = Vec::new();
        let mut location = String::new();
        for segment in error.instance_path().segments() {
            let part = segment.to_string();
            if current.is_array() {
                let index = part.parse::<usize>().map_err(|error| error.to_string())?;
                order.push((0, index, String::new()));
                location.push_str(&format!("[{index}]"));
                current = current.get(index).unwrap_or(&Value::Null);
            } else {
                order.push((1, 0, part.clone()));
                if !location.is_empty() {
                    location.push('.');
                }
                location.push_str(&part);
                current = current.get(&part).unwrap_or(&Value::Null);
            }
        }
        if first
            .as_ref()
            .is_none_or(|(previous, _, _): &(Vec<(u8, usize, String)>, String, _)| {
                &order < previous
            })
        {
            first = Some((order, location, error));
        }
    }
    if let Some((_, location, error)) = first {
        return Err(format!(
            "schema violation at {}: {error}",
            if location.is_empty() {
                "<root>"
            } else {
                &location
            }
        ));
    }
    Ok(())
}

fn python_text(raw: &[u8], reference: &str) -> Result<String, String> {
    let decoded =
        std::str::from_utf8(raw).map_err(|error| format!("decode {reference}: {error}"))?;
    Ok(decoded.replace("\r\n", "\n").replace('\r', "\n"))
}

fn current_text_matches(
    context: &mut ProductContext,
    output_ref: &str,
    expected: &[u8],
    max_bytes: usize,
) -> Result<bool, String> {
    let raw = context.current_raw(output_ref, max_bytes)?;
    python_text_equal(&raw, expected, output_ref)
}

fn python_text_equal(current: &[u8], expected: &[u8], reference: &str) -> Result<bool, String> {
    std::str::from_utf8(current).map_err(|error| format!("decode {reference}: {error}"))?;
    std::str::from_utf8(expected)
        .map_err(|error| format!("decode expected {reference}: {error}"))?;
    fn next(bytes: &[u8], index: &mut usize) -> Option<u8> {
        let byte = *bytes.get(*index)?;
        *index += 1;
        if byte == b'\r' {
            if bytes.get(*index) == Some(&b'\n') {
                *index += 1;
            }
            Some(b'\n')
        } else {
            Some(byte)
        }
    }
    let mut current_index = 0;
    let mut expected_index = 0;
    loop {
        match (
            next(current, &mut current_index),
            next(expected, &mut expected_index),
        ) {
            (Some(left), Some(right)) if left == right => {}
            (None, None) => return Ok(true),
            _ => return Ok(false),
        }
    }
}

fn load_multilingual(context: &mut ProductContext) -> Result<Multilingual, String> {
    let limits = MultilingualLimits::default();
    let ledger = context.authored_object(
        source_philosophy_multilingual::LABEL_LEDGER,
        limits.max_ledger_bytes,
    )?;
    Multilingual::from_ledger(&ledger).map_err(|error| error.to_string())
}

fn build_atlas(context: &mut ProductContext, multilingual: &Multilingual) -> Result<Value, String> {
    let mut optional_refs = BTreeSet::new();
    for path in source_philosophy_atlas::NODE_SOURCES
        .iter()
        .chain(source_philosophy_atlas::RELATION_SOURCES.iter())
    {
        if context.exists(path)? {
            optional_refs.insert((*path).to_owned());
        }
    }
    let view_refs = context.view_refs()?;
    let limits = AtlasLimits::default();
    let operation = &context.operation;
    let deadline = operation.deadline();
    let cancelled = operation.cancellation_flag();
    let mut access = Access {
        operation,
        routes: &mut context.routes,
        observations: &mut context.observations,
        fences: &mut context.fences,
        work: &mut context.work,
        output: false,
    };
    source_philosophy_atlas::build_atlas_with_input_profile(
        &mut |path| access.compiler_read(path, limits.max_source_file_bytes),
        &optional_refs,
        &view_refs,
        multilingual,
        limits,
        deadline,
        cancelled,
        PhilosophySourceReadProfile::LegacyPythonJsonLoads,
    )
    .map_err(|error| error.to_string())
}

fn build_views(context: &mut ProductContext, atlas: &Value) -> Result<Value, String> {
    let nodes = atlas
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or("philosophy atlas projection must expose nodes")?;
    let edges = atlas
        .get("edges")
        .and_then(Value::as_array)
        .ok_or("philosophy atlas projection must expose edges")?;
    let limits = ViewLimits::default();
    let operation = &context.operation;
    let deadline = operation.deadline();
    let cancelled = operation.cancellation_flag();
    let mut access = Access {
        operation,
        routes: &mut context.routes,
        observations: &mut context.observations,
        fences: &mut context.fences,
        work: &mut context.work,
        output: false,
    };
    source_philosophy_views::build_views_with_input_profile_guarded(
        &mut |path| access.compiler_read(path, limits.max_source_bytes),
        nodes,
        edges,
        limits,
        deadline,
        cancelled,
        PhilosophySourceReadProfile::LegacyPythonJsonLoads,
    )
    .map_err(|error| error.to_string())
}

fn build_graph(
    context: &mut ProductContext,
    atlas: &Value,
    views: &Value,
    multilingual: &Multilingual,
) -> Result<Value, String> {
    let view_contract = context.authored_object(
        source_philosophy_views::VIEW_CONTRACT,
        ViewLimits::default().max_source_bytes,
    )?;
    let cluster_contract = context.authored_object(
        source_philosophy_graph::CLUSTER_CONTRACT,
        ViewLimits::default().max_source_bytes,
    )?;
    let review_contract = context.authored_object(
        source_philosophy_graph::REVIEW_CONTRACT,
        ViewLimits::default().max_source_bytes,
    )?;
    let limits = GraphLimits::default();
    context.reserve_native_work(limits.max_work_units)?;
    let deadline = context.operation.deadline();
    let cancelled = context.operation.cancellation_flag();
    source_philosophy_graph::build_graph(
        atlas,
        views,
        &view_contract,
        &cluster_contract,
        &review_contract,
        multilingual,
        limits,
        deadline,
        cancelled,
    )
    .map_err(|error| error.to_string())
}

fn build_views_from_current(context: &mut ProductContext) -> Result<Value, String> {
    let atlas = context.current_value(
        source_philosophy_views::ATLAS_REF,
        AtlasLimits::default().max_output_bytes,
    )?;
    build_views(context, &atlas)
}

fn build_graph_from_current(context: &mut ProductContext) -> Result<Value, String> {
    let atlas = context.current_value(
        source_philosophy_views::ATLAS_REF,
        AtlasLimits::default().max_output_bytes,
    )?;
    let views = context.current_value(
        source_philosophy_graph::VIEWS_REF,
        ViewLimits::default().max_output_bytes,
    )?;
    let multilingual = load_multilingual(context)?;
    build_graph(context, &atlas, &views, &multilingual)
}

fn build_audit(context: &mut ProductContext) -> Result<AuditPacket, String> {
    let mut limits = AuditLimits::default();
    limits.max_work_units = 512 * 1024 * 1024;
    context.reserve_native_work(limits.max_work_units)?;
    let operation = &context.operation;
    let deadline = operation.deadline();
    let cancelled = operation.cancellation_flag();
    let access = RefCell::new(Access {
        operation,
        routes: &mut context.routes,
        observations: &mut context.observations,
        fences: &mut context.fences,
        work: &mut context.work,
        output: false,
    });
    let packet = source_philosophy_post_planting::build_audit(
        |path, max| {
            access
                .borrow_mut()
                .read(path, max)
                .map_err(CompilerError::Source)
        },
        |path| {
            access
                .borrow_mut()
                .exists(path)
                .map_err(CompilerError::Source)
        },
        |root, suffix, max| {
            access
                .borrow_mut()
                .enumerate(root, suffix, max)
                .map_err(CompilerError::Source)
        },
        limits,
        deadline,
        cancelled,
    )
    .map_err(|error| error.to_string())?;
    drop(access);
    let output_bytes = packet
        .json
        .len()
        .checked_add(packet.markdown.len())
        .ok_or("philosophy audit serialized byte counter overflow")?;
    context.measured_tick((output_bytes as u64).div_ceil(65_536).saturating_add(1))?;
    Ok(packet)
}

impl ProductContext {
    fn access(&mut self) -> Access<'_> {
        Access {
            operation: &self.operation,
            routes: &mut self.routes,
            observations: &mut self.observations,
            fences: &mut self.fences,
            work: &mut self.work,
            output: false,
        }
    }

    fn output_access(&mut self) -> Access<'_> {
        Access {
            operation: &self.output,
            routes: &mut self.output_routes,
            observations: &mut self.output_observations,
            fences: &mut self.fences,
            work: &mut self.work,
            output: true,
        }
    }
}

fn build_corpus(context: &mut ProductContext) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    let multilingual = load_multilingual(context)?;
    let atlas = build_atlas(context, &multilingual)?;
    let atlas_rendered = context.render_compact(&atlas, AtlasLimits::default().max_output_bytes)?;
    let views = build_views(context, &atlas)?;
    let views_rendered = context.render_compact(&views, ViewLimits::default().max_output_bytes)?;
    let graph = build_graph(context, &atlas, &views, &multilingual)?;
    let graph_rendered = context.render_compact(&graph, PRODUCT_RENDER_BYTES)?;
    Ok(vec![
        (source_philosophy_views::ATLAS_REF, atlas_rendered),
        (source_philosophy_graph::VIEWS_REF, views_rendered),
        (source_philosophy_graph::GRAPH_REF, graph_rendered),
    ])
}

fn builder_limits(product: Product) -> Value {
    let atlas = AtlasLimits::default();
    let views = ViewLimits::default();
    let graph = GraphLimits::default();
    let multilingual = MultilingualLimits::default();
    let mut audit = AuditLimits::default();
    audit.max_work_units = 512 * 1024 * 1024;
    match product {
        Product::Atlas => json!({
            "atlas": {"source_file_bytes":atlas.max_source_file_bytes,"input_bytes":atlas.max_input_bytes,"records":atlas.max_records,"nodes":atlas.max_nodes,"edges":atlas.max_edges,"output_bytes":atlas.max_output_bytes,"row_bytes":atlas.max_row_bytes},
            "multilingual": {"ledger_bytes":multilingual.max_ledger_bytes,"label_bytes":multilingual.max_label_bytes,"local_work_bytes":multilingual.max_work_bytes}
        }),
        Product::Views => json!({
            "views": {"source_bytes":views.max_source_bytes,"views":views.max_views,"material_items":views.max_material_items,"output_bytes":views.max_output_bytes}
        }),
        Product::Graph => json!({
            "graph": {"nodes":graph.max_nodes,"edges":graph.max_edges,"views":graph.max_views,"clusters":graph.max_clusters,"material_bytes":graph.max_material_bytes,"local_work_units":graph.max_work_units,"shared_conservative_work_reservation":graph.max_work_units},
            "multilingual": {"ledger_bytes":multilingual.max_ledger_bytes,"label_bytes":multilingual.max_label_bytes,"local_work_bytes":multilingual.max_work_bytes}
        }),
        Product::Audit => json!({
            "audit": {"authored_file_bytes":audit.max_authored_file_bytes,"projection_bytes":audit.max_projection_bytes,"total_source_bytes":audit.max_total_source_bytes,"rows":audit.max_rows,"enumerated_refs":audit.max_enumerated_refs,"selected_local_work_units":audit.max_work_units,"maintained_default_local_work_units":AuditLimits::default().max_work_units,"shared_conservative_work_reservation":audit.max_work_units,"output_bytes":audit.max_output_bytes}
        }),
        Product::Corpus => json!({
            "atlas": {"source_file_bytes":atlas.max_source_file_bytes,"input_bytes":atlas.max_input_bytes,"records":atlas.max_records,"nodes":atlas.max_nodes,"edges":atlas.max_edges,"output_bytes":atlas.max_output_bytes,"row_bytes":atlas.max_row_bytes},
            "views": {"source_bytes":views.max_source_bytes,"views":views.max_views,"material_items":views.max_material_items,"output_bytes":views.max_output_bytes},
            "graph": {"nodes":graph.max_nodes,"edges":graph.max_edges,"views":graph.max_views,"clusters":graph.max_clusters,"material_bytes":graph.max_material_bytes,"local_work_units":graph.max_work_units,"shared_conservative_work_reservation":graph.max_work_units},
            "multilingual": {"ledger_bytes":multilingual.max_ledger_bytes,"label_bytes":multilingual.max_label_bytes,"local_work_bytes":multilingual.max_work_bytes}
        }),
    }
}

fn validate_atlas(
    context: &mut ProductContext,
    current_ref: &str,
    expected: &[u8],
) -> Result<(), String> {
    let current_raw = context.current_raw(current_ref, AtlasLimits::default().max_output_bytes)?;
    let current = serde_json::from_slice::<Value>(&current_raw)
        .map_err(|error| format!("parse {current_ref}: {error}"))?;
    if !current.is_object() {
        return Err(format!("{current_ref} must contain a JSON object"));
    }
    let validator = schema_validator(context, source_philosophy_atlas::ATLAS_SCHEMA)?;
    schema_check(context, &validator, &current)?;
    let actual = context.render_compact(&current, AtlasLimits::default().max_output_bytes)?;
    if actual != expected {
        return Err(format!(
            "{current_ref} does not match the canonical rebuild"
        ));
    }
    source_philosophy_atlas::validate_assertions(&current).map_err(|error| error.to_string())
}

fn validate_views(
    context: &mut ProductContext,
    current_ref: &str,
    expected: &[u8],
) -> Result<(), String> {
    let current_raw = context.current_raw(current_ref, ViewLimits::default().max_output_bytes)?;
    let current = serde_json::from_slice::<Value>(&current_raw)
        .map_err(|error| format!("parse {current_ref}: {error}"))?;
    if !current.is_object() {
        return Err(format!("{current_ref} must contain a JSON object"));
    }
    let validator = schema_validator(context, source_philosophy_views::VIEWS_SCHEMA)?;
    schema_check(context, &validator, &current)?;
    let actual = context.render_compact(&current, ViewLimits::default().max_output_bytes)?;
    if actual != expected {
        return Err(format!(
            "{current_ref} does not match the canonical rebuild"
        ));
    }
    let mut check = || context.io_check();
    philosophy_graph_views::validate_assertions(&current, &mut check)
        .map_err(|error| error.to_string())
}

fn validate_graph(
    context: &mut ProductContext,
    current_ref: &str,
    expected: &[u8],
) -> Result<(), String> {
    source_philosophy_graph::graph_phase_mark("current_read.begin", None);
    let current_raw = context.current_raw(current_ref, PRODUCT_RENDER_BYTES)?;
    source_philosophy_graph::graph_phase_mark("current_read.end", None);
    source_philosophy_graph::graph_phase_mark("current_parse.begin", None);
    let current = serde_json::from_slice::<Value>(&current_raw)
        .map_err(|error| format!("parse {current_ref}: {error}"))?;
    if !current.is_object() {
        return Err(format!("{current_ref} must contain a JSON object"));
    }
    source_philosophy_graph::graph_phase_mark("current_parse.end", None);
    source_philosophy_graph::graph_phase_mark("schema_compile.begin", None);
    let validator = schema_validator(context, source_philosophy_graph::GRAPH_SCHEMA)?;
    source_philosophy_graph::graph_phase_mark("schema_compile.end", None);
    source_philosophy_graph::graph_phase_mark("schema_validate.begin", None);
    schema_check(context, &validator, &current)?;
    source_philosophy_graph::graph_phase_mark("schema_validate.end", None);
    source_philosophy_graph::graph_phase_mark("bytes_compare.begin", None);
    // The maintained graph validator compares the UTF-8 current file exactly
    // with the rendered rebuild, after JSON Schema validation.
    if !python_text_equal(&current_raw, expected, current_ref)? {
        return Err(format!(
            "{current_ref} does not match the canonical rebuild"
        ));
    }
    source_philosophy_graph::graph_phase_mark("bytes_compare.end", None);
    source_philosophy_graph::graph_phase_mark("assertions.begin", None);
    source_philosophy_graph::validate_graph_assertions(
        &current,
        context.operation.deadline(),
        context.operation.cancellation_flag(),
    )
    .map_err(|error| error.to_string())?;
    source_philosophy_graph::graph_phase_mark("assertions.end", None);
    Ok(())
}

fn audit_expected_json(packet: &AuditPacket) -> Vec<u8> {
    packet.json.as_bytes().to_vec()
}

/// Invoke a fixed product route. This entry point never imports or executes
/// the retained Python producer modules.
/// Supervise the maintained direct CLI through the existing executor. The
/// internal worker retains the exact product parser, source profile and bytes.
pub fn run_supervised(
    arguments: &[String],
    cancel: &std::sync::atomic::AtomicI32,
    started: std::time::Instant,
) -> Result<i32, String> {
    let options = parse_options(arguments)?;
    let deadline = started
        .checked_add(std::time::Duration::from_secs(options.max_seconds))
        .ok_or("philosophy deadline overflow")?;
    if std::time::Instant::now() >= deadline {
        return Err("philosophy deadline before worker setup".into());
    }
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let mut argv = vec![
        executable
            .to_str()
            .ok_or("philosophy executable must be UTF-8")?
            .to_owned(),
        "--philosophy-product-worker".into(),
    ];
    argv.extend_from_slice(&arguments[1..]);
    crate::executor::run_philosophy_product(
        argv,
        crate::executor::Limits {
            command_wall: std::time::Duration::from_secs(options.max_seconds),
            lane_wall: std::time::Duration::from_secs(options.max_seconds),
            ..crate::executor::Limits::default()
        },
        cancel,
        deadline,
    )
    .map_err(|error| error.to_string())
}

pub fn run(arguments: &[String], cancelled: Arc<AtomicBool>) -> Result<Value, String> {
    let options = parse_options(arguments)?;
    let mut context = ProductContext::new(&options, cancelled)?;
    let limits = builder_limits(options.product);
    let refs: Vec<&'static str>;
    let mut built_outputs: Vec<(&'static str, Vec<u8>)>;
    match options.product {
        Product::Atlas => {
            refs = vec![source_philosophy_views::ATLAS_REF];
            let multilingual = load_multilingual(&mut context)?;
            let expected = build_atlas(&mut context, &multilingual)?;
            let rendered =
                context.render_compact(&expected, AtlasLimits::default().max_output_bytes)?;
            drop(expected);
            if options.mode == Mode::Validate {
                validate_atlas(&mut context, refs[0], &rendered)?;
            } else if options.mode == Mode::Check {
                if !current_text_matches(
                    &mut context,
                    refs[0],
                    &rendered,
                    AtlasLimits::default().max_output_bytes,
                )? {
                    return Err(format!("{} is out of date", refs[0]));
                }
            }
            built_outputs = vec![(refs[0], rendered)];
        }
        Product::Views => {
            refs = vec![source_philosophy_graph::VIEWS_REF];
            let expected = build_views_from_current(&mut context)?;
            let rendered =
                context.render_compact(&expected, ViewLimits::default().max_output_bytes)?;
            drop(expected);
            if options.mode == Mode::Validate {
                validate_views(&mut context, refs[0], &rendered)?;
            } else if options.mode == Mode::Check {
                if !current_text_matches(
                    &mut context,
                    refs[0],
                    &rendered,
                    ViewLimits::default().max_output_bytes,
                )? {
                    return Err(format!("{} is out of date", refs[0]));
                }
            }
            built_outputs = vec![(refs[0], rendered)];
        }
        Product::Graph => {
            refs = vec![source_philosophy_graph::GRAPH_REF];
            source_philosophy_graph::graph_phase_mark("rebuild.begin", None);
            let expected = build_graph_from_current(&mut context)?;
            source_philosophy_graph::graph_phase_mark("rebuild.end", None);
            source_philosophy_graph::graph_phase_mark("render.begin", None);
            let rendered = context.render_compact(&expected, PRODUCT_RENDER_BYTES)?;
            source_philosophy_graph::graph_phase_mark("render.end", None);
            drop(expected);
            if options.mode == Mode::Validate {
                validate_graph(&mut context, refs[0], &rendered)?;
            } else if options.mode == Mode::Check {
                if !current_text_matches(&mut context, refs[0], &rendered, PRODUCT_RENDER_BYTES)? {
                    return Err(format!("{} is out of date", refs[0]));
                }
            }
            built_outputs = vec![(refs[0], rendered)];
        }
        Product::Audit => {
            refs = vec![
                source_philosophy_post_planting::AUDIT_JSON_REF,
                source_philosophy_post_planting::AUDIT_MARKDOWN_REF,
            ];
            let expected = build_audit(&mut context)?;
            let expected_json = audit_expected_json(&expected);
            let expected_markdown = expected.markdown.clone();
            if options.mode == Mode::Validate {
                let raw = context.current_raw(refs[0], AuditLimits::default().max_output_bytes)?;
                let current = serde_json::from_slice::<Value>(&raw)
                    .map_err(|error| format!("parse {}: {error}", refs[0]))?;
                let markdown =
                    context.current_raw(refs[1], AuditLimits::default().max_output_bytes)?;
                let markdown = python_text(&markdown, refs[1])?;
                source_philosophy_post_planting::validate_audit(&current, &markdown, &expected, {
                    let mut selected = AuditLimits::default();
                    selected.max_work_units = 512 * 1024 * 1024;
                    selected
                })
                .map_err(|error| error.to_string())?;
            } else if options.mode == Mode::Check {
                let current_json =
                    context.current_raw(refs[0], AuditLimits::default().max_output_bytes)?;
                let current_markdown =
                    context.current_raw(refs[1], AuditLimits::default().max_output_bytes)?;
                if !python_text_equal(&current_json, &expected_json, refs[0])?
                    || !python_text_equal(&current_markdown, expected_markdown.as_bytes(), refs[1])?
                {
                    return Err("ToS philosophy post-planting audit is out of date".into());
                }
            }
            built_outputs = vec![
                (refs[0], expected_json),
                (refs[1], expected_markdown.into_bytes()),
            ];
        }
        Product::Corpus => {
            refs = vec![
                source_philosophy_views::ATLAS_REF,
                source_philosophy_graph::VIEWS_REF,
                source_philosophy_graph::GRAPH_REF,
            ];
            built_outputs = build_corpus(&mut context)?;
        }
    }
    context.finish_source_fences()?;
    if options.mode == Mode::Build {
        for (reference, bytes) in &built_outputs {
            context.write_output(reference, bytes)?;
        }
    }
    context.verify_roots()?;
    let output_refs = refs;
    Ok(context.report(options.product, options.mode, &output_refs, limits))
}
