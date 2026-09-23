//! Bounded external merge for source-owner uniqueness, reference and exclusive
//! interval predicates. Only rule modules may emit these typed facts. A clean
//! merge proves the emitted facts, never completeness of their source universe.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use tos_foundation::{Digest256, Digest256Hasher};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GlobalFact {
    /// One admitted owner for `(namespace, key)` across the complete cut.
    Owner {
        namespace: String,
        key: String,
        source: String,
    },
    /// A dependency on exactly one owner in the same namespace.
    Reference {
        namespace: String,
        key: String,
        source: String,
    },
    /// Only rules whose source contract forbids overlap may emit this type.
    ExclusiveInterval {
        namespace: String,
        start: u64,
        end: u64,
        source: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GlobalIssue {
    DuplicateOwner {
        namespace: String,
        key: String,
    },
    MissingTarget {
        namespace: String,
        key: String,
    },
    InvalidInterval {
        namespace: String,
        source: String,
    },
    OverlappingInterval {
        namespace: String,
        left: String,
        right: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlobalRefusal {
    BudgetExceeded,
    ScratchUnavailable,
    ScratchCorrupt,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GlobalBudget {
    pub max_facts: u64,
    pub max_memory_bytes: usize,
    pub max_spill_bytes: u64,
    pub max_runs: usize,
    pub max_issues: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SortFact {
    domain: u8,
    namespace: String,
    key: String,
    start: u64,
    kind: u8,
    source: String,
    end: u64,
}

impl SortFact {
    fn from_fact(fact: GlobalFact) -> Self {
        match fact {
            GlobalFact::Owner {
                namespace,
                key,
                source,
            } => Self {
                domain: 0,
                namespace,
                key,
                start: 0,
                kind: 0,
                source,
                end: 0,
            },
            GlobalFact::Reference {
                namespace,
                key,
                source,
            } => Self {
                domain: 0,
                namespace,
                key,
                start: 0,
                kind: 1,
                source,
                end: 0,
            },
            GlobalFact::ExclusiveInterval {
                namespace,
                start,
                end,
                source,
            } => Self {
                domain: 1,
                namespace,
                key: String::new(),
                start,
                kind: 0,
                source,
                end,
            },
        }
    }

    fn encoded_len(&self) -> Option<u64> {
        let strings = self
            .namespace
            .len()
            .checked_add(self.key.len())?
            .checked_add(self.source.len())?;
        u64::try_from(strings).ok()?.checked_add(2 + 3 * 4 + 2 * 8)
    }

    fn write_to(&self, writer: &mut impl Write) -> Result<(), GlobalRefusal> {
        writer
            .write_all(&[self.domain, self.kind])
            .map_err(|_| GlobalRefusal::ScratchUnavailable)?;
        for value in [&self.namespace, &self.key, &self.source] {
            let len = u32::try_from(value.len()).map_err(|_| GlobalRefusal::BudgetExceeded)?;
            writer
                .write_all(&len.to_be_bytes())
                .map_err(|_| GlobalRefusal::ScratchUnavailable)?;
            writer
                .write_all(value.as_bytes())
                .map_err(|_| GlobalRefusal::ScratchUnavailable)?;
        }
        writer
            .write_all(&self.start.to_be_bytes())
            .map_err(|_| GlobalRefusal::ScratchUnavailable)?;
        writer
            .write_all(&self.end.to_be_bytes())
            .map_err(|_| GlobalRefusal::ScratchUnavailable)
    }

    fn hash_into(&self, hasher: &mut Digest256Hasher) {
        hasher.update(&[self.domain, self.kind]);
        for value in [&self.namespace, &self.key, &self.source] {
            hasher.update(&(value.len() as u32).to_be_bytes());
            hasher.update(value.as_bytes());
        }
        hasher.update(&self.start.to_be_bytes());
        hasher.update(&self.end.to_be_bytes());
    }

    fn read_from(reader: &mut impl Read, max_string: usize) -> Result<Option<Self>, GlobalRefusal> {
        let mut first = [0u8; 1];
        if reader
            .read(&mut first)
            .map_err(|_| GlobalRefusal::ScratchCorrupt)?
            == 0
        {
            return Ok(None);
        }
        let mut kind = [0u8; 1];
        reader
            .read_exact(&mut kind)
            .map_err(|_| GlobalRefusal::ScratchCorrupt)?;
        let mut strings = Vec::with_capacity(3);
        for _ in 0..3 {
            let mut length = [0u8; 4];
            reader
                .read_exact(&mut length)
                .map_err(|_| GlobalRefusal::ScratchCorrupt)?;
            let length = u32::from_be_bytes(length) as usize;
            if length > max_string {
                return Err(GlobalRefusal::ScratchCorrupt);
            }
            let mut bytes = vec![0u8; length];
            reader
                .read_exact(&mut bytes)
                .map_err(|_| GlobalRefusal::ScratchCorrupt)?;
            strings.push(String::from_utf8(bytes).map_err(|_| GlobalRefusal::ScratchCorrupt)?);
        }
        let mut start = [0u8; 8];
        let mut end = [0u8; 8];
        reader
            .read_exact(&mut start)
            .map_err(|_| GlobalRefusal::ScratchCorrupt)?;
        reader
            .read_exact(&mut end)
            .map_err(|_| GlobalRefusal::ScratchCorrupt)?;
        if first[0] > 1 || kind[0] > 1 || (first[0] == 1 && kind[0] != 0) {
            return Err(GlobalRefusal::ScratchCorrupt);
        }
        Ok(Some(Self {
            domain: first[0],
            namespace: strings.remove(0),
            key: strings.remove(0),
            source: strings.remove(0),
            start: u64::from_be_bytes(start),
            end: u64::from_be_bytes(end),
            kind: kind[0],
        }))
    }
}

/// `scratch_dir` is an already admitted private directory supplied by OPS/STO;
/// create-new run files prevent replacement of an existing owner file. Run
/// files are removed on drop, including after an incomplete merge.
pub(crate) struct GlobalFactStore {
    budget: GlobalBudget,
    scratch_dir: Option<PathBuf>,
    attempt_id: String,
    memory: Vec<SortFact>,
    memory_bytes: usize,
    count: u64,
    spilled_bytes: u64,
    runs: Vec<Run>,
}

struct Run {
    path: PathBuf,
    digest: Digest256,
}

fn run_hasher() -> Digest256Hasher {
    let mut hasher = Digest256Hasher::new();
    hasher.update(b"tos-val-fact-run-v1\0");
    hasher
}

fn read_checked(
    reader: &mut impl Read,
    hasher: &mut Digest256Hasher,
    expected: Digest256,
    max_string: usize,
) -> Result<Option<SortFact>, GlobalRefusal> {
    let fact = SortFact::read_from(reader, max_string)?;
    if let Some(ref fact) = fact {
        fact.hash_into(hasher);
    } else if hasher.clone().finalize() != expected {
        return Err(GlobalRefusal::ScratchCorrupt);
    }
    Ok(fact)
}

impl GlobalFactStore {
    pub fn new(
        budget: GlobalBudget,
        scratch_dir: Option<&Path>,
        attempt_id: &str,
    ) -> Result<Self, GlobalRefusal> {
        if attempt_id.is_empty()
            || !attempt_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(GlobalRefusal::ScratchUnavailable);
        }
        Ok(Self {
            budget,
            scratch_dir: scratch_dir.map(Path::to_path_buf),
            attempt_id: attempt_id.to_owned(),
            memory: Vec::new(),
            memory_bytes: 0,
            count: 0,
            spilled_bytes: 0,
            runs: Vec::new(),
        })
    }

    pub fn push(&mut self, fact: GlobalFact) -> Result<(), GlobalRefusal> {
        let value = SortFact::from_fact(fact);
        let size = usize::try_from(value.encoded_len().ok_or(GlobalRefusal::BudgetExceeded)?)
            .map_err(|_| GlobalRefusal::BudgetExceeded)?;
        if size > self.budget.max_memory_bytes || self.count >= self.budget.max_facts {
            return Err(GlobalRefusal::BudgetExceeded);
        }
        if self
            .memory_bytes
            .checked_add(size)
            .is_none_or(|next| next > self.budget.max_memory_bytes)
        {
            self.flush()?;
        }
        self.memory_bytes += size;
        self.count += 1;
        self.memory.push(value);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), GlobalRefusal> {
        if self.memory.is_empty() {
            return Ok(());
        }
        let dir = self
            .scratch_dir
            .as_ref()
            .ok_or(GlobalRefusal::BudgetExceeded)?;
        if self.runs.len() >= self.budget.max_runs {
            return Err(GlobalRefusal::BudgetExceeded);
        }
        let bytes = u64::try_from(self.memory_bytes).map_err(|_| GlobalRefusal::BudgetExceeded)?;
        if self
            .spilled_bytes
            .checked_add(bytes)
            .is_none_or(|next| next > self.budget.max_spill_bytes)
        {
            return Err(GlobalRefusal::BudgetExceeded);
        }
        self.memory.sort_unstable();
        let path = dir.join(format!(
            "tos-val-facts-{}-{}.run",
            self.attempt_id,
            self.runs.len()
        ));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| GlobalRefusal::ScratchUnavailable)?;
        self.runs.push(Run {
            path,
            digest: Digest256::of_bytes(b""),
        });
        let mut digest = run_hasher();
        let mut writer = BufWriter::new(file);
        for fact in &self.memory {
            fact.write_to(&mut writer)?;
            fact.hash_into(&mut digest);
        }
        writer
            .flush()
            .map_err(|_| GlobalRefusal::ScratchUnavailable)?;
        self.runs.last_mut().expect("created run is tracked").digest = digest.finalize();
        self.spilled_bytes += bytes;
        self.memory.clear();
        self.memory_bytes = 0;
        Ok(())
    }

    pub fn finish(mut self) -> Result<Vec<GlobalIssue>, GlobalRefusal> {
        if self.runs.is_empty() {
            self.memory.sort_unstable();
            return inspect_sorted(self.memory.iter(), self.budget.max_issues);
        }
        self.flush()?;
        let mut readers = Vec::with_capacity(self.runs.len());
        let mut heap = BinaryHeap::new();
        let mut hashers = Vec::with_capacity(self.runs.len());
        for (index, run) in self.runs.iter().enumerate() {
            let file = File::open(&run.path).map_err(|_| GlobalRefusal::ScratchCorrupt)?;
            let mut reader = BufReader::new(file);
            let mut hasher = run_hasher();
            if let Some(fact) = read_checked(
                &mut reader,
                &mut hasher,
                run.digest,
                self.budget.max_memory_bytes,
            )? {
                heap.push(Reverse((fact, index)));
            }
            readers.push(reader);
            hashers.push(hasher);
        }
        // The merge itself remains bounded: inspect each fact as it appears,
        // retaining only the current key/interval and bounded issues.
        let mut inspector = Inspector::new(self.budget.max_issues);
        while let Some(Reverse((fact, index))) = heap.pop() {
            inspector.accept(&fact)?;
            if let Some(next) = read_checked(
                &mut readers[index],
                &mut hashers[index],
                self.runs[index].digest,
                self.budget.max_memory_bytes,
            )? {
                heap.push(Reverse((next, index)));
            }
        }
        inspector.finish()
    }
}

impl Drop for GlobalFactStore {
    fn drop(&mut self) {
        for run in &self.runs {
            let _ = std::fs::remove_file(&run.path);
        }
    }
}

fn inspect_sorted<'a>(
    facts: impl IntoIterator<Item = &'a SortFact>,
    limit: usize,
) -> Result<Vec<GlobalIssue>, GlobalRefusal> {
    let mut inspector = Inspector::new(limit);
    for fact in facts {
        inspector.accept(fact)?;
    }
    inspector.finish()
}

struct Inspector {
    max_issues: usize,
    issues: Vec<GlobalIssue>,
    group: Option<(String, String)>,
    owner_count: u64,
    ref_count: u64,
    last_interval: Option<(String, u64, String)>,
}

impl Inspector {
    fn new(max_issues: usize) -> Self {
        Self {
            max_issues,
            issues: Vec::new(),
            group: None,
            owner_count: 0,
            ref_count: 0,
            last_interval: None,
        }
    }
    fn issue(&mut self, issue: GlobalIssue) -> Result<(), GlobalRefusal> {
        if self.issues.len() >= self.max_issues {
            return Err(GlobalRefusal::BudgetExceeded);
        }
        self.issues.push(issue);
        Ok(())
    }
    fn finish_group(&mut self) -> Result<(), GlobalRefusal> {
        if let Some((namespace, key)) = self.group.take() {
            if self.owner_count > 1 {
                self.issue(GlobalIssue::DuplicateOwner {
                    namespace: namespace.clone(),
                    key: key.clone(),
                })?;
            }
            if self.ref_count > 0 && self.owner_count == 0 {
                self.issue(GlobalIssue::MissingTarget { namespace, key })?;
            }
        }
        self.owner_count = 0;
        self.ref_count = 0;
        Ok(())
    }
    fn accept(&mut self, fact: &SortFact) -> Result<(), GlobalRefusal> {
        if fact.domain == 0 {
            let key = (fact.namespace.clone(), fact.key.clone());
            if self.group.as_ref() != Some(&key) {
                self.finish_group()?;
                self.group = Some(key);
            }
            if fact.kind == 0 {
                self.owner_count += 1;
            } else {
                self.ref_count += 1;
            }
        } else {
            self.finish_group()?;
            if fact.start >= fact.end {
                self.issue(GlobalIssue::InvalidInterval {
                    namespace: fact.namespace.clone(),
                    source: fact.source.clone(),
                })?;
            }
            if let Some((namespace, end, source)) = &self.last_interval {
                if namespace == &fact.namespace && fact.start < *end {
                    self.issue(GlobalIssue::OverlappingInterval {
                        namespace: fact.namespace.clone(),
                        left: source.clone(),
                        right: fact.source.clone(),
                    })?;
                }
            }
            if self
                .last_interval
                .as_ref()
                .is_none_or(|(scope, end, _)| scope != &fact.namespace || fact.end > *end)
            {
                self.last_interval = Some((fact.namespace.clone(), fact.end, fact.source.clone()));
            }
        }
        Ok(())
    }
    fn finish(mut self) -> Result<Vec<GlobalIssue>, GlobalRefusal> {
        self.finish_group()?;
        Ok(self.issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget(memory: usize) -> GlobalBudget {
        GlobalBudget {
            max_facts: 100,
            max_memory_bytes: memory,
            max_spill_bytes: 100_000,
            max_runs: 100,
            max_issues: 10,
        }
    }

    #[test]
    fn detects_duplicate_missing_and_interval_overlap_in_memory() {
        let mut store = GlobalFactStore::new(budget(10_000), None, "mem").unwrap();
        for fact in [
            GlobalFact::Owner {
                namespace: "ids".into(),
                key: "a".into(),
                source: "p1".into(),
            },
            GlobalFact::Owner {
                namespace: "ids".into(),
                key: "a".into(),
                source: "p2".into(),
            },
            GlobalFact::Reference {
                namespace: "ids".into(),
                key: "absent".into(),
                source: "p3".into(),
            },
            GlobalFact::ExclusiveInterval {
                namespace: "text".into(),
                start: 0,
                end: 5,
                source: "a".into(),
            },
            GlobalFact::ExclusiveInterval {
                namespace: "text".into(),
                start: 4,
                end: 8,
                source: "b".into(),
            },
        ] {
            store.push(fact).unwrap();
        }
        let issues = store.finish().unwrap();
        assert!(
            issues.iter().any(
                |issue| matches!(issue, GlobalIssue::DuplicateOwner { key, .. } if key == "a")
            )
        );
        assert!(issues.iter().any(
            |issue| matches!(issue, GlobalIssue::MissingTarget { key, .. } if key == "absent")
        ));
        assert!(
            issues
                .iter()
                .any(|issue| matches!(issue, GlobalIssue::OverlappingInterval { .. }))
        );
    }

    #[test]
    fn spill_merge_matches_memory_and_removes_runs() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("tos-val-facts-test-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let facts = [
            GlobalFact::Reference {
                namespace: "ids".into(),
                key: "present".into(),
                source: "x".into(),
            },
            GlobalFact::Reference {
                namespace: "ids".into(),
                key: "absent".into(),
                source: "x".into(),
            },
            GlobalFact::Owner {
                namespace: "ids".into(),
                key: "present".into(),
                source: "y".into(),
            },
        ];
        let mut memory = GlobalFactStore::new(budget(10_000), None, "memory").unwrap();
        let mut spill = GlobalFactStore::new(budget(58), Some(&dir), "spill").unwrap();
        for fact in facts {
            memory.push(fact.clone()).unwrap();
            spill.push(fact).unwrap();
        }
        assert_eq!(spill.finish(), memory.finish());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        std::fs::remove_dir(&dir).unwrap();
    }

    #[test]
    fn no_spill_capacity_refuses_without_partial_validity() {
        let mut store = GlobalFactStore::new(budget(34), None, "small").unwrap();
        assert_eq!(
            store.push(GlobalFact::Owner {
                namespace: "ids".into(),
                key: "a".into(),
                source: "p".into()
            }),
            Err(GlobalRefusal::BudgetExceeded)
        );
    }

    #[test]
    fn changed_spill_bytes_refuse_and_cleanup() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "tos-val-facts-tamper-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&dir).unwrap();
        let mut store = GlobalFactStore::new(budget(80), Some(&dir), "tamper").unwrap();
        store
            .push(GlobalFact::Owner {
                namespace: "ids".into(),
                key: "a".into(),
                source: "owner-original".into(),
            })
            .unwrap();
        store.flush().unwrap();
        let path = store.runs[0].path.clone();
        let mut bytes = std::fs::read(&path).unwrap();
        let index = bytes
            .windows(14)
            .position(|slice| slice == b"owner-original")
            .unwrap();
        bytes[index] = b'X';
        std::fs::write(path, bytes).unwrap();
        assert_eq!(store.finish(), Err(GlobalRefusal::ScratchCorrupt));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        std::fs::remove_dir(dir).unwrap();
    }
}
