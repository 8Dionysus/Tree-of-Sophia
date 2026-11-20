//! Agents at scale over the act journal (TOS-D-0083): what an agent may append
//! under its permission tier, and the folds the person reads an army by. The
//! browser's page tools and the bridge's MCP both admit agents through this
//! one implementation.

use super::{
    ActJournal, AuthorType, Extra, JsonValue, MAX_INTENT, MAX_RECORD_BYTES, array, boolean, emit,
    get, int, object, parse, string,
};
use std::collections::{HashMap, HashSet};

/// The permission tiers of `access/contracts/act-commands.v1.json`, weakest first.
pub const AGENT_TIERS: [&str; 3] = ["read", "draft", "propose"];
/// Decisions, assignments, squads, change packets, mirrored workspace events and research branches belong to the person.
pub const PERSON_ONLY_KINDS: &[&str] = &[
    "review",
    "assignment",
    "squad",
    "change-packet",
    "workspace-event",
    "branch",
];
/// Tickets and proposals put work before the person and need the propose tier.
pub const PROPOSING_KINDS: &[&str] = &["ticket", "proposal"];
const PERSON_ONLY: &[&str] = PERSON_ONLY_KINDS;
const PROPOSING: &[&str] = PROPOSING_KINDS;

/// A moment as JavaScript's `Date#toISOString` writes it (`YYYY-MM-DDTHH:MM:SS.mmmZ`), for years 0 to 9999.
pub fn utc_iso(ms: i64) -> Option<String> {
    let days = ms.div_euclid(86_400_000);
    let rest = ms.rem_euclid(86_400_000);
    // Civil date from days since 1970-01-01 (proleptic Gregorian).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    if !(0..=9999).contains(&year) {
        return None;
    }
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rest / 3_600_000,
        rest / 60_000 % 60,
        rest / 1_000 % 60,
        rest % 1_000
    ))
}

/// One line of intent: whitespace collapsed, at most 280 characters with an ellipsis.
fn intent_line(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() > MAX_INTENT {
        let mut short: String = line.chars().take(MAX_INTENT - 1).collect();
        short.push('…');
        short
    } else {
        line
    }
}

/// A list of addresses without repeats, in first order; any other value passes to the journal's own check.
fn unique(value: Option<&JsonValue>) -> JsonValue {
    match value {
        None | Some(JsonValue::Null) => array(Vec::new()),
        Some(JsonValue::Array(items)) if items.iter().all(|item| item.as_str().is_some()) => {
            let mut seen = HashSet::new();
            array(
                items
                    .iter()
                    .filter(|item| seen.insert(item.as_str().unwrap_or_default().to_owned()))
                    .cloned()
                    .collect(),
            )
        }
        Some(other) => other.clone(),
    }
}

impl ActJournal {
    /// Check an act an agent asks to append under `tier` and turn it into a draft the
    /// journal can take: the host gives the moment and a fresh ID for a draft
    /// without one. Refusals: `act_tier`, `act_author`, `act_person_only`,
    /// `act_not_own`. The journal still checks every field when the draft is appended.
    pub fn agent_draft(
        &self,
        raw: &[u8],
        tier: &str,
        at_ms: i64,
        fresh_id: &str,
    ) -> Result<Vec<u8>, String> {
        if !AGENT_TIERS.contains(&tier) || tier == "read" {
            return Err("act_tier".to_owned());
        }
        let input = parse(raw, MAX_RECORD_BYTES)?;
        let author = get(&input, "author")
            .filter(|author| {
                matches!(author, JsonValue::Object(_))
                    && matches!(
                        get(author, "type").and_then(JsonValue::as_str),
                        Some("agent" | "mixed")
                    )
            })
            .ok_or("act_author")?;
        let kind = get(&input, "kind")
            .and_then(JsonValue::as_str)
            .unwrap_or_default();
        if PERSON_ONLY.contains(&kind) {
            return Err("act_person_only".to_owned());
        }
        if PROPOSING.contains(&kind) && tier != "propose" {
            return Err("act_tier".to_owned());
        }
        let targets = unique(get(&input, "targets"));
        if kind == "withdrawal" {
            let own = get(author, "id").and_then(JsonValue::as_str);
            for target in targets.as_array().into_iter().flatten() {
                let index = target
                    .as_str()
                    .and_then(|address| self.act_index(address.trim_start_matches("act:")));
                let mine = index.is_some_and(|index| {
                    let act = &self.acts[index as usize];
                    own == Some(self.strings.text(act.author))
                });
                if !mine {
                    return Err("act_not_own".to_owned());
                }
            }
        }
        let id = get(&input, "id")
            .and_then(JsonValue::as_str)
            .filter(|id| !id.is_empty())
            .unwrap_or(fresh_id);
        let intent = intent_line(
            get(&input, "intent")
                .and_then(JsonValue::as_str)
                .unwrap_or_default(),
        );
        let at = utc_iso(at_ms).ok_or("act_invalid:at")?;
        let mut fields = vec![
            ("schema", string(super::ACT_SCHEMA)),
            ("id", string(id)),
            ("kind", string(kind)),
            ("author", author.clone()),
            ("targets", targets),
            ("grounds", unique(get(&input, "grounds"))),
            ("intent", string(&intent)),
            (
                "body",
                get(&input, "body")
                    .cloned()
                    .unwrap_or_else(|| JsonValue::Object(Vec::new())),
            ),
            ("parents", unique(get(&input, "parents"))),
            ("at", string(&at)),
            (
                "posture",
                object(vec![
                    ("local_only", boolean(true)),
                    ("source", boolean(false)),
                    ("reviewed_by_tree", boolean(false)),
                    ("canon", boolean(false)),
                ]),
            ),
        ];
        if let Some(replaces) = get(&input, "replaces")
            .filter(|value| !matches!(value, JsonValue::Null) && value.as_str() != Some(""))
        {
            fields.push(("replaces", replaces.clone()));
        }
        emit(&object(fields))
    }

    /// Proposals no decision has reached yet: the review queue the bridge paces agents by.
    /// The count follows each append; a withdrawal or a replacement makes it count again.
    pub fn open_proposals(&self) -> usize {
        if let Some(open) = self.open_count.get() {
            return open;
        }
        let open = self
            .group_order
            .iter()
            .flat_map(|group| self.group_members[group].iter())
            .filter(|index| self.live(**index) && self.decision(**index).is_none())
            .count();
        self.open_count.set(Some(open));
        open
    }

    /// The live proposals the review at `review` decides, each with the decision
    /// it had before. Read before the review enters the indexes: afterwards the
    /// review is the decision of every one of them.
    pub(super) fn review_moves(&self, review: u32) -> Vec<(u32, Option<u32>)> {
        let act = &self.acts[review as usize];
        let mut seen = HashSet::new();
        let mut moves = Vec::new();
        let mut take = |proposal: u32, moves: &mut Vec<(u32, Option<u32>)>| {
            if self.live(proposal) && seen.insert(proposal) {
                moves.push((
                    proposal,
                    self.decision(proposal).map(|(_, decision)| decision),
                ));
            }
        };
        for target in act.targets.iter() {
            let text = self.strings.text(*target);
            if let Some(proposal) = self.act_address(text) {
                if matches!(self.acts[proposal as usize].extra, Extra::Proposal { .. }) {
                    take(proposal, &mut moves);
                }
            } else if text.starts_with("group:") {
                for member in self.group_members.get(target).into_iter().flatten() {
                    if self.acts[*member as usize].seq < act.seq {
                        take(*member, &mut moves);
                    }
                }
            }
        }
        moves
    }

    /// The place of a decision among a squad's counts: open, accepted, rejected, deferred.
    pub(super) fn decision_slot(&self, decision: Option<u32>) -> usize {
        match decision.map(|decision| self.strings.text(decision)) {
            None => 0,
            Some("reject") => 2,
            Some("defer") => 3,
            Some(_) => 1,
        }
    }

    /// The army by squad: each squad's declaration (role, act budget), its agents and how many
    /// acted within the window, its acts against the budget, its proposals by decision, the
    /// tickets assigned to it and its stalled agents (whose last act is a lease that ran out).
    pub(super) fn squads_view(&self, now_ms: i64, window_ms: i64, limit: usize) -> JsonValue {
        #[derive(Default)]
        struct Row {
            declared: Option<(u32, Option<u64>)>,
            acts: u64,
            recent: u64,
            agents: usize,
            active: HashSet<u32>,
            models: Vec<u32>,
            open: u64,
            accepted: u64,
            rejected: u64,
            deferred: u64,
            assigned: u64,
            stalled: HashSet<u32>,
            last_ms: Option<i64>,
        }
        let since = now_ms - window_ms.max(0);
        let mut rows: HashMap<u32, Row> = HashMap::new();
        let mut order: Vec<u32> = Vec::new();
        // What each squad did, kept as acts were appended: its acts, agents, models, proposals and assignments.
        // A withdrawal or a replacement may have changed decisions anywhere: the counts are read again once.
        if self.squad_counts_dirty.get() {
            for track in self.squad_tracks.values() {
                let mut counts = [0u64; 4];
                for proposal in track.proposals.iter().filter(|p| self.live(**p)) {
                    counts[self.decision_slot(self.decision(*proposal).map(|(_, d)| d))] += 1;
                }
                track.decisions.set(counts);
            }
            self.squad_counts_dirty.set(false);
        }
        for (name, track) in &self.squad_tracks {
            order.push(*name);
            let [open, accepted, rejected, deferred] = track.decisions.get();
            let row = Row {
                acts: track.acts,
                agents: track.agents.len(),
                models: track.models.iter().copied().collect(),
                last_ms: track.last_ms,
                assigned: track.assignments.iter().filter(|i| self.live(**i)).count() as u64,
                open,
                accepted,
                rejected,
                deferred,
                ..Row::default()
            };
            rows.insert(*name, row);
        }
        for index in &self.squads {
            if !self.live(*index) {
                continue;
            }
            if let Extra::Squad { name, role, budget } = self.acts[*index as usize].extra {
                rows.entry(name)
                    .or_insert_with(|| {
                        order.push(name);
                        Row::default()
                    })
                    .declared = Some((role, budget));
            }
        }
        // The window is read from the newest acts back, as presence reads it.
        let mut active = HashSet::new();
        let mut scanned = 0usize;
        for act in self.acts.iter().rev() {
            scanned += 1;
            if scanned > super::MAX_PRESENCE_SCAN || act.at_ms < since - window_ms.max(60_000) {
                break;
            }
            if act.at_ms < since || act.at_ms > now_ms || act.author_type == AuthorType::Human {
                continue;
            }
            active.insert(act.author);
            if let Some(entry) = act.squad.and_then(|squad| rows.get_mut(&squad)) {
                entry.recent += 1;
                entry.active.insert(act.author);
            }
        }
        let mut stalled = 0u64;
        for (author, index) in &self.agent_last {
            let act = &self.acts[*index as usize];
            if let Extra::Lease { until_ms } = act.extra
                && until_ms < now_ms
            {
                stalled += 1;
                if let Some(entry) = act.squad.and_then(|squad| rows.get_mut(&squad)) {
                    entry.stalled.insert(*author);
                }
            }
        }
        let agents = self.agent_last.len();
        order.sort_by(|a, b| {
            let (left, right) = (&rows[a], &rows[b]);
            right
                .acts
                .cmp(&left.acts)
                .then_with(|| self.strings.text(*a).cmp(self.strings.text(*b)))
        });
        let text_list = |items: &HashSet<u32>| {
            let mut names: Vec<&str> = items.iter().map(|i| self.strings.text(*i)).collect();
            names.sort_unstable();
            array(names.into_iter().map(string).collect())
        };
        object(vec![
            ("squads_count", int(order.len() as i128)),
            ("agents", int(agents as i128)),
            ("active", int(active.len() as i128)),
            ("stalled", int(stalled as i128)),
            ("open_proposals", int(self.open_proposals() as i128)),
            (
                "squads",
                array(
                    order
                        .iter()
                        .take(limit)
                        .map(|name| {
                            let entry = &rows[name];
                            let (role, budget) = entry.declared.unzip();
                            let budget = budget.flatten();
                            object(vec![
                                ("squad", string(self.strings.text(*name))),
                                ("declared", boolean(entry.declared.is_some())),
                                (
                                    "role",
                                    role.map_or(JsonValue::Null, |r| string(self.strings.text(r))),
                                ),
                                (
                                    "budget_acts",
                                    budget.map_or(JsonValue::Null, |b| int(b as i128)),
                                ),
                                ("acts", int(entry.acts as i128)),
                                ("recent_acts", int(entry.recent as i128)),
                                (
                                    "over_budget",
                                    boolean(budget.is_some_and(|b| entry.acts >= b)),
                                ),
                                ("agents", int(entry.agents as i128)),
                                ("active", int(entry.active.len() as i128)),
                                ("models", text_list(&entry.models.iter().copied().collect())),
                                (
                                    "proposals",
                                    object(vec![
                                        ("open", int(entry.open as i128)),
                                        ("accepted", int(entry.accepted as i128)),
                                        ("rejected", int(entry.rejected as i128)),
                                        ("deferred", int(entry.deferred as i128)),
                                    ]),
                                ),
                                ("assigned", int(entry.assigned as i128)),
                                ("stalled", int(entry.stalled.len() as i128)),
                                (
                                    "last_ms",
                                    entry.last_ms.map_or(JsonValue::Null, |at| int(at as i128)),
                                ),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }
}
