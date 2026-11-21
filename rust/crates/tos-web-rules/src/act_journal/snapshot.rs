//! A snapshot of the journal's folds (K6, TOS-D-0083): the interned names, the
//! declared kinds and every act as the folds keep it, so a host restores a long
//! journal without parsing and checking each record again. The indexes are not
//! saved: loading enters every act through `index_act`, as appending did. A
//! snapshot names the registry it was folded under; it is derived data, and
//! the host decides when it may stand in for the records (the bridge checks it
//! against its store's bytes).

use super::{Act, ActJournal, AuthorType, Extra, Interner, parse, rule_from};
use tos_foundation::Digest256;

const MAGIC: &[u8; 8] = b"TOSAJS1\0";
const NONE: u32 = u32::MAX;

struct Writer(Vec<u8>);
impl Writer {
    fn u8(&mut self, value: u8) {
        self.0.push(value);
    }
    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn u64(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn i64(&mut self, value: i64) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn bytes(&mut self, value: &[u8]) {
        self.u32(value.len() as u32);
        self.0.extend_from_slice(value);
    }
    fn option(&mut self, value: Option<u32>) {
        self.u32(value.unwrap_or(NONE));
    }
    fn list(&mut self, items: &[u32]) {
        self.u32(items.len() as u32);
        for item in items {
            self.u32(*item);
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(n)
            .filter(|end| *end <= self.bytes.len());
        let end = end.ok_or("act_snapshot_truncated")?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| "act_snapshot_truncated")?,
        ))
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| "act_snapshot_truncated")?,
        ))
    }
    fn i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| "act_snapshot_truncated")?,
        ))
    }
    fn bytes(&mut self) -> Result<&'a [u8], String> {
        let len = self.u32()? as usize;
        self.take(len)
    }
    fn text(&mut self) -> Result<&'a str, String> {
        std::str::from_utf8(self.bytes()?).map_err(|_| "act_snapshot_text".to_owned())
    }
    fn option(&mut self) -> Result<Option<u32>, String> {
        Ok(Some(self.u32()?).filter(|value| *value != NONE))
    }
    fn list(&mut self, bound: u32) -> Result<Box<[u32]>, String> {
        let len = self.u32()? as usize;
        let mut out = Vec::with_capacity(len.min(1024));
        for _ in 0..len {
            let item = self.u32()?;
            if item >= bound {
                return Err("act_snapshot_index".to_owned());
            }
            out.push(item);
        }
        Ok(out.into_boxed_slice())
    }
}

fn write_interner(out: &mut Writer, interner: &Interner) {
    out.u32(interner.items.len() as u32);
    for item in &interner.items {
        out.bytes(item.as_bytes());
    }
}

fn read_interner(input: &mut Reader<'_>) -> Result<Interner, String> {
    let mut interner = Interner::default();
    let count = input.u32()?;
    for expected in 0..count {
        if interner.intern(input.text()?) != expected {
            return Err("act_snapshot_interner".to_owned());
        }
    }
    Ok(interner)
}

impl ActJournal {
    /// The journal's folds as bytes; `ActJournal::from_snapshot` restores them.
    pub fn snapshot(&self) -> Vec<u8> {
        let mut out = Writer(Vec::with_capacity(self.acts.len() * 80 + 1024));
        out.0.extend_from_slice(MAGIC);
        out.0.extend_from_slice(self.registry_digest.as_bytes());
        out.u64(self.next_seq);
        write_interner(&mut out, &self.strings);
        write_interner(&mut out, &self.ids);
        out.u32(self.declared_order.len() as u32);
        for name in &self.declared_order {
            out.bytes(name.as_bytes());
            out.bytes(
                self.declared_sources
                    .get(name)
                    .map_or(&[][..], Vec::as_slice),
            );
        }
        out.u32(self.acts.len() as u32);
        for act in &self.acts {
            out.u64(act.seq);
            out.u32(act.kind);
            out.u8(u8::from(act.known));
            out.u8(match act.author_type {
                AuthorType::Human => 0,
                AuthorType::Agent => 1,
                AuthorType::Mixed => 2,
            });
            out.u32(act.author);
            out.option(act.model);
            out.option(act.role);
            out.option(act.squad);
            out.i64(act.at_ms);
            out.list(&act.targets);
            out.list(&act.parents);
            out.option(act.replaces);
            match act.extra {
                Extra::None => out.u8(0),
                Extra::Ticket { category } => {
                    out.u8(1);
                    out.u32(category);
                }
                Extra::Proposal { category, group } => {
                    out.u8(2);
                    out.u32(category);
                    out.u32(group);
                }
                Extra::Review { decision } => {
                    out.u8(3);
                    out.u32(decision);
                }
                Extra::Lease { until_ms } => {
                    out.u8(4);
                    out.i64(until_ms);
                }
                Extra::Assignment { assignee } => {
                    out.u8(5);
                    out.u32(assignee);
                }
                Extra::Withdrawal { depth } => {
                    out.u8(6);
                    out.u8(depth);
                }
                Extra::Squad { name, role, budget } => {
                    out.u8(7);
                    out.u32(name);
                    out.u32(role);
                    out.u64(budget.unwrap_or(u64::MAX));
                }
            }
        }
        out.0
    }

    /// A journal from a snapshot folded under the same registry. Every act is
    /// indexed again; names and references are checked against their tables.
    pub fn from_snapshot(registry_json: &[u8], bytes: &[u8]) -> Result<Self, String> {
        let mut journal = Self::new(registry_json)?;
        let mut input = Reader { bytes, at: 0 };
        if input.take(8)? != MAGIC {
            return Err("act_snapshot_format".to_owned());
        }
        if input.take(32)? != journal.registry_digest.as_bytes() {
            return Err("act_snapshot_registry".to_owned());
        }
        let next_seq = input.u64()?;
        journal.strings = read_interner(&mut input)?;
        journal.ids = read_interner(&mut input)?;
        let strings = journal.strings.items.len() as u32;
        let ids = journal.ids.items.len() as u32;
        for _ in 0..input.u32()? {
            let name = input.text()?.to_owned();
            let source = input.bytes()?;
            let rule = rule_from(&parse(source, super::MAX_RECORD_BYTES)?)?;
            journal
                .declared_sources
                .insert(name.clone(), source.to_vec());
            journal.declared_order.push(name.clone());
            journal.declared.insert(name, rule);
        }
        let count = input.u32()?;
        if count != ids {
            return Err("act_snapshot_ids".to_owned());
        }
        let name = |value: u32| {
            (value < strings)
                .then_some(value)
                .ok_or("act_snapshot_index")
        };
        let optional = |value: Option<u32>| value.map(name).transpose();
        journal.acts.reserve(count as usize);
        for index in 0..count {
            let seq = input.u64()?;
            if seq != u64::from(index) + 1 {
                return Err("act_snapshot_sequence".to_owned());
            }
            let kind = name(input.u32()?)?;
            let known = input.u8()? == 1;
            let author_type = match input.u8()? {
                0 => AuthorType::Human,
                1 => AuthorType::Agent,
                2 => AuthorType::Mixed,
                _ => return Err("act_snapshot_author".to_owned()),
            };
            let author = name(input.u32()?)?;
            let (model, role, squad) = (
                optional(input.option()?)?,
                optional(input.option()?)?,
                optional(input.option()?)?,
            );
            let at_ms = input.i64()?;
            let targets = input.list(strings)?;
            let parents = input.list(index)?;
            let replaces = input.option()?;
            if replaces.is_some_and(|replaced| replaced >= index) {
                return Err("act_snapshot_index".to_owned());
            }
            let extra = match input.u8()? {
                0 => Extra::None,
                1 => Extra::Ticket {
                    category: name(input.u32()?)?,
                },
                2 => Extra::Proposal {
                    category: name(input.u32()?)?,
                    group: name(input.u32()?)?,
                },
                3 => Extra::Review {
                    decision: name(input.u32()?)?,
                },
                4 => Extra::Lease {
                    until_ms: input.i64()?,
                },
                5 => Extra::Assignment {
                    assignee: name(input.u32()?)?,
                },
                6 => Extra::Withdrawal { depth: input.u8()? },
                7 => Extra::Squad {
                    name: name(input.u32()?)?,
                    role: name(input.u32()?)?,
                    budget: Some(input.u64()?).filter(|budget| *budget != u64::MAX),
                },
                _ => return Err("act_snapshot_extra".to_owned()),
            };
            journal.acts.push(Act {
                seq,
                kind,
                known,
                author_type,
                author,
                model,
                role,
                squad,
                at_ms,
                targets,
                parents,
                replaces,
                extra,
            });
            journal.index_act(index);
        }
        if input.at != bytes.len() || next_seq != u64::from(count) + 1 {
            return Err("act_snapshot_format".to_owned());
        }
        journal.next_seq = next_seq;
        Ok(journal)
    }

    /// The IDs of the acts in sequence order (the act at position `n` has sequence `n + 1`).
    pub fn act_ids(&self) -> impl Iterator<Item = &str> {
        self.ids.items.iter().map(|id| &**id)
    }
}

/// The digest a snapshot carries for its registry.
pub(super) fn registry_digest(registry_json: &[u8]) -> Digest256 {
    Digest256::of_bytes(registry_json)
}
