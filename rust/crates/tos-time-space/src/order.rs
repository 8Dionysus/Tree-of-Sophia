use crate::{
    ALLEN_RELATIONS, Allen, AxisBinding, ProperInterval, Rational, Span,
    budget::{Budget, DomainError, DomainLimits, Result, push},
};
use tos_foundation::ExactRecordRef;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderOp {
    Less,
    LessEqual,
    Equal,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderAtom {
    pub left: usize,
    pub op: OrderOp,
    pub right: usize,
}
#[derive(Clone, Debug)]
pub struct SourcedConstraint {
    pub atom: OrderAtom,
    pub source: ExactRecordRef,
}
#[derive(Clone, Debug)]
pub struct Anchor {
    pub variable: usize,
    pub value: Rational,
    pub source: ExactRecordRef,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderDomain {
    DenseUnboundedRational,
    Discrete,
    Unknown,
}
/// Endpoint i*2 is start, i*2+1 is end. Input vectors remain caller-owned.
/// The exact reference is a supplied pin; the caller/resolver owns its binding
/// to this actual body. A matching digest string alone does not establish that.
#[derive(Debug)]
pub struct OrderProblem {
    pub exact: ExactRecordRef,
    pub axis: AxisBinding,
    pub domain: OrderDomain,
    pub interval_count: usize,
    pub constraints: Vec<SourcedConstraint>,
    pub anchors: Vec<Anchor>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Premise {
    ProperInterval(usize),
    Constraint(usize),
    AnchorOrder {
        left_anchor: usize,
        right_anchor: usize,
    },
    Hypothesis(usize),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CycleStep {
    pub from: usize,
    pub to: usize,
    pub strict: bool,
    pub premise: Premise,
}
#[derive(Debug)]
pub enum OrderOutcome {
    Witness(Vec<Rational>),
    StrictCycle(Vec<CycleStep>),
}
#[derive(Debug)]
pub enum RelationImage {
    NoAdmissibleModel(Vec<CycleStep>),
    Exact {
        possible: Vec<(Allen, Vec<Rational>)>,
    },
}
fn array<T: Clone>(n: usize, value: T, b: &mut Budget) -> Result<Vec<T>> {
    b.reserve::<T>(n)?;
    b.work(n)?;
    let mut out = Vec::new();
    out.try_reserve_exact(n)
        .map_err(|_| DomainError::ResourceLimit)?;
    out.resize(n, value);
    Ok(out)
}
fn admit(p: &OrderProblem, extra: &[OrderAtom], b: &mut Budget) -> Result<usize> {
    b.exact(&p.exact)?;
    p.axis.admit(b)?;
    match p.domain {
        OrderDomain::DenseUnboundedRational => {}
        OrderDomain::Discrete => {
            return Err(DomainError::Unsupported("discrete order is not dense Q"));
        }
        OrderDomain::Unknown => return Err(DomainError::NeedsBinding("dense order capability")),
    }
    if p.interval_count == 0 {
        return Err(DomainError::Invalid("empty interval carrier"));
    }
    let n = p
        .interval_count
        .checked_mul(2)
        .ok_or(DomainError::ResourceLimit)?;
    b.work(n)?;
    for c in &p.constraints {
        b.item()?;
        b.exact(&c.source)?;
        if c.atom.left >= n || c.atom.right >= n {
            return Err(DomainError::Invalid("constraint variable"));
        }
    }
    for a in &p.anchors {
        b.item()?;
        b.exact(&a.source)?;
        if a.variable >= n {
            return Err(DomainError::Invalid("anchor variable"));
        }
    }
    for a in extra {
        b.work(1)?;
        if a.left >= n || a.right >= n {
            return Err(DomainError::Invalid("hypothesis variable"));
        }
    }
    Ok(n)
}
fn arcs(out: &mut Vec<CycleStep>, atom: OrderAtom, premise: Premise, b: &mut Budget) -> Result<()> {
    push(
        b,
        out,
        CycleStep {
            from: atom.left,
            to: atom.right,
            strict: atom.op == OrderOp::Less,
            premise,
        },
    )?;
    if atom.op == OrderOp::Equal {
        push(
            b,
            out,
            CycleStep {
                from: atom.right,
                to: atom.left,
                strict: false,
                premise,
            },
        )?;
    }
    Ok(())
}
fn graph(p: &OrderProblem, extra: &[OrderAtom], b: &mut Budget) -> Result<Vec<CycleStep>> {
    let mut out = Vec::new();
    for i in 0..p.interval_count {
        arcs(
            &mut out,
            OrderAtom {
                left: 2 * i,
                op: OrderOp::Less,
                right: 2 * i + 1,
            },
            Premise::ProperInterval(i),
            b,
        )?;
    }
    for (i, c) in p.constraints.iter().enumerate() {
        arcs(&mut out, c.atom, Premise::Constraint(i), b)?;
    }
    for (i, c) in extra.iter().enumerate() {
        arcs(&mut out, *c, Premise::Hypothesis(i), b)?;
    }
    // Budgeted insertion sort. Every anchor is retained in p and independently
    // validated in the witness; equal-valued anchors produce equality arcs.
    let mut sorted = Vec::new();
    for i in 0..p.anchors.len() {
        push(b, &mut sorted, i)?;
        let mut j = sorted.len() - 1;
        while j > 0 {
            b.work(1)?;
            if p.anchors[sorted[j - 1]].value <= p.anchors[sorted[j]].value {
                break;
            }
            sorted.swap(j - 1, j);
            j -= 1;
        }
    }
    for pair in sorted.windows(2) {
        b.work(1)?;
        let (a, c) = (&p.anchors[pair[0]], &p.anchors[pair[1]]);
        arcs(
            &mut out,
            OrderAtom {
                left: a.variable,
                op: if a.value == c.value {
                    OrderOp::Equal
                } else {
                    OrderOp::Less
                },
                right: c.variable,
            },
            Premise::AnchorOrder {
                left_anchor: pair[0],
                right_anchor: pair[1],
            },
            b,
        )?;
    }
    Ok(out)
}
fn holds(a: OrderAtom, values: &[Rational]) -> bool {
    match a.op {
        OrderOp::Less => values[a.left] < values[a.right],
        OrderOp::LessEqual => values[a.left] <= values[a.right],
        OrderOp::Equal => values[a.left] == values[a.right],
    }
}
fn verify_values(
    p: &OrderProblem,
    values: &[Rational],
    extra: &[OrderAtom],
    b: &mut Budget,
) -> Result<bool> {
    let n = admit(p, extra, b)?;
    if values.len() != n {
        return Ok(false);
    }
    for i in 0..p.interval_count {
        b.work(1)?;
        if values[2 * i] >= values[2 * i + 1] {
            return Ok(false);
        }
    }
    for c in &p.constraints {
        b.work(1)?;
        if !holds(c.atom, values) {
            return Ok(false);
        }
    }
    for c in extra {
        b.work(1)?;
        if !holds(*c, values) {
            return Ok(false);
        }
    }
    for a in &p.anchors {
        b.work(1)?;
        if values[a.variable] != a.value {
            return Ok(false);
        }
    }
    Ok(true)
}
pub fn verify_order_witness(
    p: &OrderProblem,
    values: &[Rational],
    limits: DomainLimits,
) -> Result<bool> {
    verify_values(p, values, &[], &mut Budget::new(limits)?)
}
fn premise_atom(p: &OrderProblem, premise: Premise) -> Result<OrderAtom> {
    match premise {
        Premise::ProperInterval(i) if i < p.interval_count => Ok(OrderAtom {
            left: 2 * i,
            op: OrderOp::Less,
            right: 2 * i + 1,
        }),
        Premise::Constraint(i) => p
            .constraints
            .get(i)
            .map(|c| c.atom)
            .ok_or(DomainError::Invalid("proof constraint")),
        Premise::AnchorOrder {
            left_anchor,
            right_anchor,
        } => {
            let a = p
                .anchors
                .get(left_anchor)
                .ok_or(DomainError::Invalid("proof anchor"))?;
            let c = p
                .anchors
                .get(right_anchor)
                .ok_or(DomainError::Invalid("proof anchor"))?;
            if a.value > c.value {
                return Err(DomainError::Invalid("anchor direction"));
            }
            Ok(OrderAtom {
                left: a.variable,
                right: c.variable,
                op: if a.value == c.value {
                    OrderOp::Equal
                } else {
                    OrderOp::Less
                },
            })
        }
        _ => Err(DomainError::Invalid("non-base proof premise")),
    }
}
/// Check the actual cycle and each anchor-derived step against the full input.
/// No claim of a trusted proof-kernel or source-lookup execution is implied.
pub fn verify_order_cycle(
    p: &OrderProblem,
    cycle: &[CycleStep],
    limits: DomainLimits,
) -> Result<bool> {
    let mut b = Budget::new(limits)?;
    let n = admit(p, &[], &mut b)?;
    if cycle.is_empty() {
        return Ok(false);
    }
    let mut strict = false;
    for (i, step) in cycle.iter().enumerate() {
        b.work(1)?;
        if step.from >= n || step.to >= n || step.to != cycle[(i + 1) % cycle.len()].from {
            return Ok(false);
        }
        let a = premise_atom(p, step.premise)?;
        let direction = step.from == a.left && step.to == a.right;
        let reverse_equal = a.op == OrderOp::Equal && step.from == a.right && step.to == a.left;
        if !(direction || reverse_equal) || step.strict != (a.op == OrderOp::Less) {
            return Ok(false);
        }
        strict |= step.strict;
    }
    Ok(strict)
}
fn solve(p: &OrderProblem, extra: &[OrderAtom], b: &mut Budget) -> Result<OrderOutcome> {
    let n = admit(p, extra, b)?;
    let size = n.checked_mul(n).ok_or(DomainError::ResourceLimit)?;
    let mut reach = array(size, 0u8, b)?;
    let edges = graph(p, extra, b)?;
    for i in 0..n {
        b.work(1)?;
        reach[i * n + i] = 1;
    }
    for edge in &edges {
        b.work(1)?;
        let at = edge.from * n + edge.to;
        reach[at] = reach[at].max(if edge.strict { 2 } else { 1 });
    }
    for k in 0..n {
        for i in 0..n {
            for j in 0..n {
                b.work(1)?;
                if reach[i * n + k] != 0 && reach[k * n + j] != 0 {
                    reach[i * n + j] = reach[i * n + j].max(reach[i * n + k].max(reach[k * n + j]));
                }
            }
        }
    }
    let mut bad = None;
    for i in 0..n {
        b.work(1)?;
        if reach[i * n + i] == 2 {
            bad = Some(i);
            break;
        }
    }
    if let Some(start) = bad {
        let states = n.checked_mul(2).ok_or(DomainError::ResourceLimit)?;
        let mut seen = array(states, false, b)?;
        let mut parent = array(states, None::<(usize, usize)>, b)?;
        let mut queue = Vec::new();
        push(b, &mut queue, 2 * start)?;
        seen[2 * start] = true;
        let target = 2 * start + 1;
        let mut head = 0;
        while head < queue.len() && !seen[target] {
            b.work(1)?;
            let at = queue[head];
            head += 1;
            for (index, edge) in edges.iter().enumerate() {
                b.work(1)?;
                if edge.from != at / 2 {
                    continue;
                }
                let next = 2 * edge.to + if at % 2 == 1 || edge.strict { 1 } else { 0 };
                if !seen[next] {
                    seen[next] = true;
                    parent[next] = Some((at, index));
                    push(b, &mut queue, next)?;
                }
            }
        }
        if !seen[target] {
            return Err(DomainError::Invalid("internal cycle reconstruction"));
        }
        let mut cycle = Vec::new();
        let mut at = target;
        while at != 2 * start {
            b.work(1)?;
            let (prior, index) = parent[at].ok_or(DomainError::Invalid("internal cycle parent"))?;
            push(b, &mut cycle, edges[index])?;
            at = prior;
        }
        b.work(cycle.len())?;
        cycle.reverse();
        return Ok(OrderOutcome::StrictCycle(cycle));
    }
    let mut class = array(n, usize::MAX, b)?;
    let mut reps = Vec::new();
    for i in 0..n {
        b.work(1)?;
        if class[i] != usize::MAX {
            continue;
        }
        let label = reps.len();
        push(b, &mut reps, i)?;
        for j in 0..n {
            b.work(1)?;
            if reach[i * n + j] != 0 && reach[j * n + i] != 0 {
                class[j] = label;
            }
        }
    }
    let g = reps.len();
    let mut used = array(g, false, b)?;
    let mut order = Vec::new();
    while order.len() < g {
        let mut chosen = None;
        for c in 0..g {
            b.work(1)?;
            if used[c] {
                continue;
            }
            let mut incoming = false;
            for h in 0..g {
                b.work(1)?;
                if h != c && !used[h] && reach[reps[h] * n + reps[c]] != 0 {
                    incoming = true;
                    break;
                }
            }
            if !incoming {
                chosen = Some(c);
                break;
            }
        }
        let c = chosen.ok_or(DomainError::Invalid("internal quotient cycle"))?;
        used[c] = true;
        push(b, &mut order, c)?;
    }
    let mut rank = array(g, 0usize, b)?;
    for (i, c) in order.iter().enumerate() {
        b.work(1)?;
        rank[*c] = i;
    }
    let mut pins = array(g, None::<Rational>, b)?;
    for a in &p.anchors {
        b.work(1)?;
        let r = rank[class[a.variable]];
        if let Some(old) = pins[r] {
            if old != a.value {
                return Err(DomainError::Invalid("internal anchor quotient"));
            }
        }
        pins[r] = Some(a.value);
    }
    let zero = Rational::integer(0)?;
    let mut values_by_rank = array(g, zero, b)?;
    for r in 0..g {
        b.work(1)?;
        if let Some(value) = pins[r] {
            values_by_rank[r] = value;
            continue;
        }
        let mut before = None;
        let mut after = None;
        for (k, pin) in pins.iter().enumerate() {
            b.work(1)?;
            if pin.is_some() {
                if k < r {
                    before = Some(k);
                } else if k > r && after.is_none() {
                    after = Some(k);
                }
            }
        }
        let q =
            |x: usize| Rational::integer(i128::try_from(x).map_err(|_| DomainError::NumericRange)?);
        values_by_rank[r] = match (before, after) {
            (Some(lo), Some(hi)) => {
                let a = pins[lo].ok_or(DomainError::Invalid("internal lower pin"))?;
                let c = pins[hi].ok_or(DomainError::Invalid("internal upper pin"))?;
                a.checked_add(
                    c.checked_sub(a)?
                        .checked_mul(q(r - lo)?.checked_div(q(hi - lo)?)?)?,
                )?
            }
            (Some(lo), None) => pins[lo]
                .ok_or(DomainError::Invalid("internal lower pin"))?
                .checked_add(q(r - lo)?)?,
            (None, Some(hi)) => pins[hi]
                .ok_or(DomainError::Invalid("internal upper pin"))?
                .checked_sub(q(hi - r)?)?,
            (None, None) => q(r)?,
        };
    }
    let mut values = Vec::new();
    for c in class {
        b.work(1)?;
        push(b, &mut values, values_by_rank[rank[c]])?;
    }
    if !verify_values(p, &values, extra, b)? {
        return Err(DomainError::Invalid("internal witness substitution"));
    }
    Ok(OrderOutcome::Witness(values))
}
pub fn solve_order(p: &OrderProblem, limits: DomainLimits) -> Result<OrderOutcome> {
    solve(p, &[], &mut Budget::new(limits)?)
}
fn relation_atoms(a: usize, c: usize, relation: Allen, b: &mut Budget) -> Result<Vec<OrderAtom>> {
    use Allen::*;
    if matches!(
        relation,
        After | MetBy | OverlappedBy | StartedBy | Contains | FinishedBy
    ) {
        return relation_atoms(c, a, relation.converse(), b);
    }
    let (s, e, u, v) = (2 * a, 2 * a + 1, 2 * c, 2 * c + 1);
    let mut out = Vec::new();
    let mut add = |left, op, right| push(b, &mut out, OrderAtom { left, op, right });
    match relation {
        Before => add(e, OrderOp::Less, u)?,
        Meets => add(e, OrderOp::Equal, u)?,
        Overlaps => {
            add(s, OrderOp::Less, u)?;
            add(u, OrderOp::Less, e)?;
            add(e, OrderOp::Less, v)?;
        }
        Starts => {
            add(s, OrderOp::Equal, u)?;
            add(e, OrderOp::Less, v)?;
        }
        During => {
            add(u, OrderOp::Less, s)?;
            add(e, OrderOp::Less, v)?;
        }
        Finishes => {
            add(u, OrderOp::Less, s)?;
            add(e, OrderOp::Equal, v)?;
        }
        Equals => {
            add(s, OrderOp::Equal, u)?;
            add(e, OrderOp::Equal, v)?;
        }
        _ => return Err(DomainError::Invalid("relation direction")),
    }
    Ok(out)
}
/// One call budget covers base plus all thirteen hypotheses, their anchors,
/// generated arcs, closure, cycle reconstruction and witness substitution (N8).
/// A limit returns Err(ResourceLimit), never a complete possible-relation set.
pub fn possible_allen(
    p: &OrderProblem,
    a: usize,
    c: usize,
    limits: DomainLimits,
) -> Result<RelationImage> {
    let mut b = Budget::new(limits)?;
    admit(p, &[], &mut b)?;
    if a >= p.interval_count || c >= p.interval_count {
        return Err(DomainError::Invalid("interval index"));
    }
    if let OrderOutcome::StrictCycle(cycle) = solve(p, &[], &mut b)? {
        return Ok(RelationImage::NoAdmissibleModel(cycle));
    }
    let mut possible = Vec::new();
    for relation in ALLEN_RELATIONS {
        let atoms = relation_atoms(a, c, relation, &mut b)?;
        if let OrderOutcome::Witness(values) = solve(p, &atoms, &mut b)? {
            let left =
                ProperInterval::new(Span::new(values[2 * a], values[2 * a + 1], true, false)?)?;
            let right =
                ProperInterval::new(Span::new(values[2 * c], values[2 * c + 1], true, false)?)?;
            if crate::allen(left, right) != relation {
                return Err(DomainError::Invalid("internal relation witness"));
            }
            push(&mut b, &mut possible, (relation, values))?;
        }
    }
    if possible.is_empty() {
        return Err(DomainError::Invalid(
            "internal empty interval relation image",
        ));
    }
    Ok(RelationImage::Exact { possible })
}
