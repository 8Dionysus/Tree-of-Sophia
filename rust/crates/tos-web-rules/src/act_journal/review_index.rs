//! The review queue and the accepted proposals kept as acts arrive (K4 at
//! scale): each group's open proposals with the places, authors and models
//! among them, its deferred and decided counts, and the accepted proposals by
//! their newest decision with whether a change packet names them. A review
//! moves the proposals it decides; a withdrawal or a replacement marks the
//! index for one full count at the next read. The folds `review` and `decided`
//! read it instead of walking every proposal.

use super::{ActJournal, AuthorType, Extra, JsonValue, array, boolean, int, object, string};
use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Default)]
pub(super) struct GroupTrack {
    category: u32,
    open: BTreeSet<u32>,
    deferred: u64,
    decided: u64,
    targets: HashMap<u32, u32>,
    authors: HashMap<u32, u32>,
    models: HashMap<u32, u32>,
    humans: u32,
}

#[derive(Default)]
pub(super) struct ReviewIndex {
    groups: HashMap<u32, GroupTrack>,
    accepted: BTreeSet<(Reverse<u64>, u32, u32, u32)>,
    packed: HashSet<u32>,
    pub(super) dirty: bool,
}

fn add(counts: &mut HashMap<u32, u32>, key: u32) {
    *counts.entry(key).or_default() += 1;
}
fn remove(counts: &mut HashMap<u32, u32>, key: u32) {
    if let Some(count) = counts.get_mut(&key) {
        *count -= 1;
        if *count == 0 {
            counts.remove(&key);
        }
    }
}

impl ActJournal {
    fn accepting(&self, decision: u32) -> bool {
        matches!(self.strings.text(decision), "accept" | "accept-with-edit")
    }

    fn open_in(&self, index: &mut ReviewIndex, proposal: u32) {
        let act = &self.acts[proposal as usize];
        let Extra::Proposal { category, group } = act.extra else {
            return;
        };
        let track = index.groups.entry(group).or_default();
        track.category = category;
        track.open.insert(proposal);
        for target in act.targets.iter() {
            add(&mut track.targets, *target);
        }
        add(&mut track.authors, act.author);
        match (act.author_type, act.model) {
            (AuthorType::Agent, Some(model)) => add(&mut track.models, model),
            _ => track.humans += 1,
        }
    }

    fn close_in(&self, index: &mut ReviewIndex, proposal: u32) {
        let act = &self.acts[proposal as usize];
        let Extra::Proposal { group, .. } = act.extra else {
            return;
        };
        let Some(track) = index.groups.get_mut(&group) else {
            return;
        };
        if !track.open.remove(&proposal) {
            return;
        }
        for target in act.targets.iter() {
            remove(&mut track.targets, *target);
        }
        remove(&mut track.authors, act.author);
        match (act.author_type, act.model) {
            (AuthorType::Agent, Some(model)) => remove(&mut track.models, model),
            _ => track.humans = track.humans.saturating_sub(1),
        }
    }

    /// Place a proposal under its decision (none: open).
    fn decide_in(&self, index: &mut ReviewIndex, proposal: u32, decision: Option<(u32, u32)>) {
        let Extra::Proposal { group, .. } = self.acts[proposal as usize].extra else {
            return;
        };
        match decision {
            None => self.open_in(index, proposal),
            Some((review, decision)) => {
                let track = index.groups.entry(group).or_default();
                if self.strings.text(decision) == "defer" {
                    track.deferred += 1;
                } else {
                    track.decided += 1;
                }
                if let Extra::Proposal { category, .. } = self.acts[proposal as usize].extra {
                    track.category = category;
                }
                if self.accepting(decision) {
                    index.accepted.insert((
                        Reverse(self.acts[review as usize].seq),
                        proposal,
                        review,
                        decision,
                    ));
                }
            }
        }
    }

    /// Take a proposal out of its decision's place.
    fn undecide_in(&self, index: &mut ReviewIndex, proposal: u32, decision: Option<(u32, u32)>) {
        let Extra::Proposal { group, .. } = self.acts[proposal as usize].extra else {
            return;
        };
        match decision {
            None => self.close_in(index, proposal),
            Some((review, decision)) => {
                if let Some(track) = index.groups.get_mut(&group) {
                    if self.strings.text(decision) == "defer" {
                        track.deferred = track.deferred.saturating_sub(1);
                    } else {
                        track.decided = track.decided.saturating_sub(1);
                    }
                }
                index.accepted.remove(&(
                    Reverse(self.acts[review as usize].seq),
                    proposal,
                    review,
                    decision,
                ));
            }
        }
    }

    /// Keep the index in step with one appended act. `moves` are the proposals a
    /// review decides with their decisions before it (`ActJournal::review_moves`).
    pub(super) fn keep_review_index(
        &self,
        index: u32,
        moves: Option<&(u32, Vec<(u32, Option<(u32, u32)>)>)>,
    ) {
        let mut kept = self.review_index.borrow_mut();
        if kept.dirty {
            return;
        }
        let act = &self.acts[index as usize];
        if act.replaces.is_some() || matches!(act.extra, Extra::Withdrawal { .. }) {
            kept.dirty = true;
            return;
        }
        match act.extra {
            Extra::Proposal { .. } => self.open_in(&mut kept, index),
            Extra::Review { decision } => {
                for (proposal, before) in
                    moves.map(|(_, moves)| moves.as_slice()).unwrap_or_default()
                {
                    self.undecide_in(&mut kept, *proposal, *before);
                    self.decide_in(&mut kept, *proposal, Some((index, decision)));
                }
            }
            _ if self.strings.text(act.kind) == "change-packet" => {
                for target in act.targets.iter() {
                    if let Some(proposal) = self.act_address(self.strings.text(*target)) {
                        kept.packed.insert(proposal);
                    }
                }
            }
            _ => {}
        }
    }

    /// Count the index whole from the journal (after a withdrawal or a replacement).
    fn rebuild_review_index(&self) {
        let mut kept = ReviewIndex::default();
        for group in &self.group_order {
            for proposal in &self.group_members[group] {
                if self.live(*proposal) {
                    self.decide_in(&mut kept, *proposal, self.decision(*proposal));
                }
            }
        }
        for (index, act) in self.acts.iter().enumerate() {
            if self.strings.text(act.kind) == "change-packet" && self.live(index as u32) {
                for target in act.targets.iter() {
                    if let Some(proposal) = self.act_address(self.strings.text(*target)) {
                        kept.packed.insert(proposal);
                    }
                }
            }
        }
        *self.review_index.borrow_mut() = kept;
    }

    fn clean_review_index(&self) {
        if self.review_index.borrow().dirty {
            self.rebuild_review_index();
        }
    }

    /// The review queue by group: most open proposals first, then the oldest.
    pub(super) fn review(&self, limit: usize) -> JsonValue {
        self.clean_review_index();
        let kept = self.review_index.borrow();
        let mut groups: Vec<(u32, &GroupTrack, u64)> = self
            .group_order
            .iter()
            .filter_map(|group| kept.groups.get(group).map(|track| (*group, track)))
            .filter(|(_, track)| !track.open.is_empty() || track.deferred > 0)
            .map(|(group, track)| {
                let first = track
                    .open
                    .first()
                    .map_or(u64::MAX, |p| self.acts[*p as usize].seq);
                (group, track, first)
            })
            .collect();
        let (open_total, deferred_total) =
            groups
                .iter()
                .fold((0u64, 0u64), |(open, deferred), (_, track, _)| {
                    (open + track.open.len() as u64, deferred + track.deferred)
                });
        groups.sort_by(|a, b| b.1.open.len().cmp(&a.1.open.len()).then(a.2.cmp(&b.2)));
        let rows = groups
            .iter()
            .take(limit)
            .map(|(group, track, _)| {
                let mut models: Vec<&str> =
                    track.models.keys().map(|m| self.strings.text(*m)).collect();
                models.sort_unstable();
                let independent = track.models.len() + usize::from(track.humans > 0);
                object(vec![
                    ("group", string(self.strings.text(*group))),
                    ("category", string(self.strings.text(track.category))),
                    ("open_count", int(track.open.len() as i128)),
                    ("deferred_count", int(track.deferred as i128)),
                    ("decided_count", int(track.decided as i128)),
                    ("targets_count", int(track.targets.len() as i128)),
                    ("authors_count", int(track.authors.len() as i128)),
                    ("independent_count", int(independent as i128)),
                    ("models", array(models.into_iter().map(string).collect())),
                    (
                        "open",
                        array(
                            track
                                .open
                                .iter()
                                .take(32)
                                .map(|p| string(self.ids.text(*p)))
                                .collect(),
                        ),
                    ),
                ])
            })
            .collect();
        object(vec![
            ("open_total", int(open_total as i128)),
            ("deferred_total", int(deferred_total as i128)),
            ("groups_total", int(groups.len() as i128)),
            ("groups", array(rows)),
        ])
    }

    /// Proposals the person accepted, newest decision first, each with its review and
    /// whether a live change packet already names it.
    pub(super) fn decided(&self, limit: usize) -> JsonValue {
        self.clean_review_index();
        let kept = self.review_index.borrow();
        let unpacked = kept
            .accepted
            .iter()
            .filter(|row| !kept.packed.contains(&row.1))
            .count();
        object(vec![
            ("total", int(kept.accepted.len() as i128)),
            ("unpacked", int(unpacked as i128)),
            (
                "proposals",
                array(
                    kept.accepted
                        .iter()
                        .take(limit)
                        .map(|(_, proposal, review, decision)| {
                            object(vec![
                                ("proposal", string(self.ids.text(*proposal))),
                                ("review", string(self.ids.text(*review))),
                                ("decision", string(self.strings.text(*decision))),
                                ("packed", boolean(kept.packed.contains(proposal))),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }

    /// Mark the review index for a full count (tests compare it with the kept one).
    #[cfg(test)]
    pub(super) fn recount_reviews(&self) {
        self.review_index.borrow_mut().dirty = true;
    }
}
