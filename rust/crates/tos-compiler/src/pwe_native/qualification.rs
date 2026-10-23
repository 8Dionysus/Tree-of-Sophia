//! The fixed, preserve-only v1.1.0 value grammar and Q1 extractor.
use super::basis::{Error, PweRef, Result, Session};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Name,
    Responsibility,
    Identity,
}
impl Kind {
    pub fn outer_predicate(self) -> &'static str {
        match self {
            Self::Name => "pwe_name_assignment",
            Self::Responsibility => "pwe_responsibility_account",
            Self::Identity => "pwe_identity_correspondence",
        }
    }
}
/// A shape-checked candidate. Construction neither resolves bindings nor admits
/// a Claim. The complete original borrowed value remains accessible.
pub struct PreparedAccount<'v> {
    pub(super) value: &'v Value,
    pub(super) kind: Kind,
}
impl<'v> PreparedAccount<'v> {
    pub fn value(&self) -> &'v Value {
        self.value
    }
    pub fn kind(&self) -> Kind {
        self.kind
    }
}
/// Ordered role occurrences, not an ID-set. Index paths keep repeated exact
/// refs and different versions visible to the supplied source resolver.
pub struct BasisSlot<'v> {
    pub role: &'static str,
    pub outer: usize,
    pub inner: usize,
    pub reference: PweRef<'v>,
}
/// Preserve-only extraction keeps explicit unknown slots as source data. It
/// does not construct ResolvedBasis or license a semantic consumer.
pub struct UnresolvedBasisSlot<'v> {
    pub role: &'static str,
    pub outer: usize,
    pub inner: usize,
    pub binding: &'v Value,
}
pub struct PreservedBasis<'v> {
    pub slots: Vec<BasisSlot<'v>>,
    pub unresolved: Vec<UnresolvedBasisSlot<'v>>,
}
pub struct QualifiedAccount<'v> {
    pub(super) prepared: PreparedAccount<'v>,
    pub(super) scope: &'v Value,
    pub(super) context: &'v Value,
    pub(super) signature: &'v Value,
    pub(super) predicate: &'v Value,
    pub(super) profile: &'v Value,
}
/// Resolved row supplied by the native owner from an actual exact selected
/// profile/signature. This data alone grants no authority. No row is fabricated
/// from the normative candidate's still-unregistered profile declarations.
pub struct DispatchRow<'v> {
    pub outer_predicate: &'v str,
    pub kind: Kind,
    pub signature: &'v Value,
    pub predicate: &'v Value,
    pub qualification_profile: &'v Value,
    pub arguments: &'v [&'v str],
}
impl<'v> QualifiedAccount<'v> {
    pub fn value(&self) -> &'v Value {
        self.prepared.value
    }
    pub fn scope(&self) -> &'v Value {
        self.scope
    }
    pub fn context(&self) -> &'v Value {
        self.context
    }
    pub fn signature(&self) -> &'v Value {
        self.signature
    }
    pub fn predicate(&self) -> &'v Value {
        self.predicate
    }
    pub fn qualification_profile(&self) -> &'v Value {
        self.profile
    }
}

impl Session<'_> {
    pub fn check_dispatch(
        &mut self,
        a: &QualifiedAccount<'_>,
        row: &DispatchRow<'_>,
    ) -> Result<()> {
        self.admit(a.value())?;
        for x in [row.signature, row.predicate, row.qualification_profile] {
            self.admit(x)?;
            self.typed(x)?;
        }
        if row.kind != a.prepared.kind
            || !self.same_text(row.outer_predicate, a.prepared.kind.outer_predicate())?
            || !self.equal(row.signature, a.signature)?
            || !self.equal(row.predicate, a.predicate)?
            || !self.equal(row.qualification_profile, a.profile)?
        {
            return Err(Error::QualificationMismatch("selected dispatch row"));
        }
        let expected: &[&str] = match a.prepared.kind {
            Kind::Name => &[
                "subject",
                "subject_role",
                "assignment",
                "name",
                "name_function",
                "named_bearer",
            ],
            Kind::Responsibility => &[
                "subject",
                "role",
                "attribution_mode",
                "assignees",
                "responsibility_scope",
                "role_basis",
            ],
            Kind::Identity => &[
                "left",
                "right",
                "identity_level",
                "criterion",
                "asserted_outcome",
            ],
        };
        if row.arguments.len() != expected.len() {
            return Err(Error::QualificationMismatch("dispatch argument arity"));
        }
        for (actual, expected) in row.arguments.iter().zip(expected) {
            if !self.same_text(actual, expected)? {
                return Err(Error::QualificationMismatch("dispatch argument role/order"));
            }
            self.field(a.value(), expected)?;
        }
        Ok(())
    }
    pub(super) fn closed(&mut self, v: &Value, keys: &[&str]) -> Result<()> {
        let m = v.as_object().ok_or(Error::Malformed("object"))?;
        self.tick(
            m.len()
                .checked_mul(keys.len())
                .ok_or(Error::ResourceLimit)?,
        )?;
        if m.len() != keys.len() {
            return Err(Error::Malformed("closed object"));
        }
        for k in keys {
            self.field(v, k)?;
        }
        Ok(())
    }
    pub(super) fn one<'v>(&mut self, v: &'v Value, options: &[&str]) -> Result<&'v str> {
        let s = self.text(v)?;
        for option in options {
            if self.same_text(s, option)? {
                return Ok(s);
            }
        }
        Err(Error::Malformed("enum"))
    }
    pub(super) fn prose(&mut self, v: &Value) -> Result<()> {
        let s = self.text(v)?;
        // JSON-Schema's ECMA whitespace class, including BOM and excluding NEL.
        let space = |c: char| matches!(c,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}');
        if s.chars().count() > 16384 || !s.chars().any(|c| !space(c)) {
            return Err(Error::Malformed("source prose"));
        }
        Ok(())
    }
    fn wording(&mut self, v: &Value) -> Result<()> {
        self.closed(v, &["text", "language", "script"])?;
        let x = self.field(v, "text")?;
        self.prose(x)?;
        let language = self.field(v, "language")?;
        if !language.is_null() {
            let s = self.text(language)?;
            let mut pieces = s.split('-');
            let first = pieces.next().unwrap_or("");
            let private = matches!(first, "i" | "I" | "x" | "X");
            if !(private
                || (2..=8).contains(&first.len()) && first.bytes().all(|b| b.is_ascii_alphabetic()))
            {
                return Err(Error::Malformed("language tag"));
            }
            let mut count = 0;
            for p in pieces {
                count += 1;
                if !(1..=8).contains(&p.len()) || !p.bytes().all(|b| b.is_ascii_alphanumeric()) {
                    return Err(Error::Malformed("language tag"));
                }
            }
            if private && count == 0 {
                return Err(Error::Malformed("language tag"));
            }
        }
        let script = self.field(v, "script")?;
        if !script.is_null() {
            let s = self.text(script)?;
            if s.len() != 4 || !s.bytes().all(|b| b.is_ascii_alphabetic()) {
                return Err(Error::Malformed("script"));
            }
        }
        Ok(())
    }
    fn binding_shape(&mut self, v: &Value) -> Result<()> {
        match self.str_field(v, "state")? {
            "bound" => {
                self.closed(v, &["state", "value"])?;
                let x = self.field(v, "value")?;
                self.typed(x)?;
            }
            "unresolved" => {
                self.closed(v, &["state", "source_wording", "missing_binding"])?;
                for key in ["source_wording", "missing_binding"] {
                    let x = self.field(v, key)?;
                    self.prose(x)?;
                }
            }
            _ => return Err(Error::Malformed("binding state")),
        }
        Ok(())
    }
    fn bounded_unique<'v>(&mut self, v: &'v Value, min: usize) -> Result<&'v [Value]> {
        let xs = self.array(v)?;
        if xs.len() < min || xs.len() > 128 {
            return Err(Error::Malformed("array cardinality"));
        }
        for (i, x) in xs.iter().enumerate() {
            for y in &xs[..i] {
                if self.equal(x, y)? {
                    return Err(Error::Malformed("duplicate array item"));
                }
            }
        }
        Ok(xs)
    }
    fn typed_array(&mut self, v: &Value, min: usize) -> Result<()> {
        for x in self.bounded_unique(v, min)? {
            self.typed(x)?;
        }
        Ok(())
    }
    fn assignees(&mut self, v: &Value) -> Result<()> {
        match self.str_field(v, "mode")? {
            "single" => {
                self.closed(v, &["mode", "bearer"])?;
                let x = self.field(v, "bearer")?;
                self.typed(x)?;
            }
            "joint" => {
                self.closed(v, &["mode", "bearers"])?;
                let x = self.field(v, "bearers")?;
                self.typed_array(x, 2)?;
            }
            "alternatives" => {
                self.closed(
                    v,
                    &["mode", "groups", "exhaustiveness", "mutual_exclusivity"],
                )?;
                let x = self.field(v, "groups")?;
                for group in self.bounded_unique(x, 2)? {
                    self.typed_array(group, 1)?;
                }
                let x = self.field(v, "exhaustiveness")?;
                self.one(x, &["source-claimed-exhaustive", "partial", "unknown"])?;
                let x = self.field(v, "mutual_exclusivity")?;
                self.one(x, &["source-claimed-exclusive", "not-exclusive", "unknown"])?;
            }
            "unestablished" => {
                self.closed(v, &["mode", "account"])?;
                let x = self.field(v, "account")?;
                self.prose(x)?;
            }
            "identified-anonymous" => {
                self.closed(v, &["mode", "bearer", "identification_account"])?;
                let x = self.field(v, "bearer")?;
                self.typed(x)?;
                let x = self.field(v, "identification_account")?;
                self.prose(x)?;
            }
            _ => return Err(Error::Malformed("assignee mode")),
        }
        Ok(())
    }
    /// Independent item check. This accepts unresolved candidates for faithful
    /// preservation; call qualify for executable, exactly bound content.
    pub fn prepare<'v>(&mut self, v: &'v Value) -> Result<PreparedAccount<'v>> {
        self.admit(v)?;
        self.prepare_admitted(v)
    }
    pub(super) fn prepare_admitted<'v>(&mut self, v: &'v Value) -> Result<PreparedAccount<'v>> {
        let kind = match self.str_field(v, "kind")? {
            "name-assignment" => Kind::Name,
            "responsibility-account" => Kind::Responsibility,
            "identity-correspondence" => Kind::Identity,
            _ => return Err(Error::Unsupported("PWE value kind")),
        };
        let common = [
            "kind",
            "source_wording",
            "subject",
            "scope",
            "world",
            "members",
            "endpoints",
            "basis",
            "limitations",
            "supersedes_proposition",
            "qualification",
            "source_locations",
        ];
        let extra: &[&str] = match kind {
            Kind::Name => &[
                "subject_role",
                "name",
                "name_function",
                "assignment",
                "named_bearer",
                "account",
            ],
            Kind::Responsibility => &[
                "role",
                "attribution_mode",
                "assignees",
                "responsibility_scope",
                "role_basis",
            ],
            Kind::Identity => &[
                "left",
                "right",
                "identity_level",
                "criterion",
                "asserted_outcome",
                "grounds",
                "counterreading",
                "similarity_method",
            ],
        };
        let m = v.as_object().ok_or(Error::Malformed("value"))?;
        if m.len() != common.len() + extra.len() {
            return Err(Error::Malformed("value fields"));
        }
        for key in common.iter().chain(extra) {
            self.field(v, key)?;
        }
        let x = self.field(v, "source_wording")?;
        self.wording(x)?;
        let x = self.field(v, "subject")?;
        self.typed(x)?;
        for k in ["scope", "world"] {
            let x = self.field(v, k)?;
            self.binding_shape(x)?;
        }
        let x = self.field(v, "members")?;
        for member in self.bounded_unique(x, 1)? {
            self.text(member)?;
        }
        let x = self.field(v, "endpoints")?;
        self.typed_array(x, 1)?;
        let x = self.field(v, "basis")?;
        for basis in self.bounded_unique(x, 1)? {
            self.closed(basis, &["role", "input", "inspection"])?;
            let x = self.field(basis, "role")?;
            self.one(
                x,
                &[
                    "source-record",
                    "source-occurrence",
                    "criterion",
                    "scope",
                    "world",
                    "method",
                    "counterevidence",
                ],
            )?;
            let x = self.field(basis, "inspection")?;
            self.one(
                x,
                &[
                    "source-content-read",
                    "metadata-read",
                    "citation-only",
                    "not-read",
                ],
            )?;
            let x = self.field(basis, "input")?;
            self.binding_shape(x)?;
        }
        let x = self.field(v, "limitations")?;
        self.prose(x)?;
        let x = self.field(v, "supersedes_proposition")?;
        if !x.is_null() {
            self.exact(x)?;
        }
        let q = self.field(v, "qualification")?;
        self.closed(
            q,
            &[
                "signature_ref",
                "predicate_ref",
                "context_ref",
                "scope_ref",
                "modality",
                "qualification_profile_ref",
            ],
        )?;
        for key in [
            "signature_ref",
            "predicate_ref",
            "context_ref",
            "scope_ref",
            "qualification_profile_ref",
        ] {
            let x = self.field(q, key)?;
            self.binding_shape(x)?;
        }
        let x = self.field(q, "modality")?;
        self.one(
            x,
            &[
                "source-reported",
                "historical-hypothesis",
                "textual-role",
                "metadata-description",
            ],
        )?;
        let x = self.field(v, "source_locations")?;
        for origin in self.bounded_unique(x, 0)? {
            self.closed(
                origin,
                &[
                    "source_version",
                    "locator_scheme_version",
                    "locator",
                    "namespace_ref",
                ],
            )?;
            for key in ["source_version", "locator_scheme_version"] {
                let x = self.field(origin, key)?;
                self.exact(x)?;
            }
            let x = self.field(origin, "namespace_ref")?;
            if !x.is_null() {
                self.exact(x)?;
            }
            let x = self.field(origin, "locator")?;
            self.prose(x)?;
        }
        match kind {
            Kind::Name => {
                let x = self.field(v, "subject_role")?;
                self.one(x, &["named-referent", "containing-object"])?;
                let x = self.field(v, "name")?;
                self.wording(x)?;
                let x = self.field(v, "name_function")?;
                self.one(
                    x,
                    &[
                        "source-name",
                        "pseudonym",
                        "collective-pseudonym",
                        "signature",
                        "title",
                        "transliteration",
                        "editorial-label",
                        "rejected-normalization",
                    ],
                )?;
                let x = self.field(v, "assignment")?;
                self.one(x, &["name-used-by", "signature-in"])?;
                let x = self.field(v, "named_bearer")?;
                self.binding_shape(x)?;
                let x = self.field(v, "account")?;
                self.prose(x)?;
            }
            Kind::Responsibility => {
                let x = self.field(v, "role")?;
                self.one(
                    x,
                    &[
                        "author",
                        "translator",
                        "expression-editor",
                        "edition-editor",
                        "compiler",
                        "copyist",
                        "speaker",
                        "addressee",
                        "publisher",
                        "rights-holder",
                    ],
                )?;
                let x = self.field(v, "attribution_mode")?;
                self.one(
                    x,
                    &[
                        "source-attribution",
                        "historical-responsibility",
                        "literary-authorization",
                        "textual-role",
                        "modern-compilation",
                    ],
                )?;
                let x = self.field(v, "assignees")?;
                self.assignees(x)?;
                for key in ["responsibility_scope", "role_basis"] {
                    let x = self.field(v, key)?;
                    self.prose(x)?;
                }
            }
            Kind::Identity => {
                for key in ["left", "right"] {
                    let x = self.field(v, key)?;
                    self.typed(x)?;
                }
                let x = self.field(v, "identity_level")?;
                self.one(
                    x,
                    &[
                        "historical-person",
                        "textual-persona",
                        "collective",
                        "work",
                        "expression",
                        "edition",
                        "item",
                        "artifact",
                        "file",
                        "textual-fragment",
                        "quotation-passage",
                        "composite",
                    ],
                )?;
                let x = self.field(v, "asserted_outcome")?;
                self.one(
                    x,
                    &[
                        "candidate-match",
                        "same-referent",
                        "distinct-referents",
                        "undetermined",
                    ],
                )?;
                for key in ["criterion", "similarity_method"] {
                    let x = self.field(v, key)?;
                    self.binding_shape(x)?;
                }
                for key in ["grounds", "counterreading"] {
                    let x = self.field(v, key)?;
                    self.prose(x)?;
                }
            }
        }
        Ok(PreparedAccount { value: v, kind })
    }
    /// Exact first-consumer aliases. This does not pretend to validate a native
    /// signature/dispatch route: source::check_dispatch performs that separately.
    pub fn qualify<'v>(&mut self, p: PreparedAccount<'v>) -> Result<QualifiedAccount<'v>> {
        self.admit(p.value)?;
        let q = self.field(p.value, "qualification")?;
        let scope_b = self.field(p.value, "scope")?;
        let scope = self.bound(scope_b, "scope")?;
        let world_b = self.field(p.value, "world")?;
        let context = self.bound(world_b, "world")?;
        let qs_b = self.field(q, "scope_ref")?;
        let qs = self.bound(qs_b, "qualification scope")?;
        let qc_b = self.field(q, "context_ref")?;
        let qc = self.bound(qc_b, "qualification context")?;
        if !self.equal(scope, qs)? {
            return Err(Error::QualificationMismatch("scope alias"));
        }
        if !self.equal(context, qc)? {
            return Err(Error::QualificationMismatch("complete context/world alias"));
        }
        let b = self.field(q, "signature_ref")?;
        let signature = self.bound(b, "signature")?;
        let b = self.field(q, "predicate_ref")?;
        let predicate = self.bound(b, "predicate")?;
        let b = self.field(q, "qualification_profile_ref")?;
        let profile = self.bound(b, "qualification profile")?;
        if p.kind == Kind::Identity {
            let subject = self.field(p.value, "subject")?;
            let left = self.field(p.value, "left")?;
            if !self.equal(subject, left)? {
                return Err(Error::QualificationMismatch("focal identity subject"));
            }
            let b = self.field(p.value, "criterion")?;
            self.bound(b, "identity criterion")?;
        }
        Ok(QualifiedAccount {
            prepared: p,
            scope,
            context,
            signature,
            predicate,
            profile,
        })
    }
    /// One grammar-aware basis extractor shared by validation and consumers.
    /// Unresolved entries remain in the original value; they cannot be returned
    /// as resolved basis. Fails atomically, without publishing a partial list.
    pub fn exact_basis<'v>(&mut self, p: &PreparedAccount<'v>) -> Result<Vec<BasisSlot<'v>>> {
        self.collect_basis(p,None)
    }
    /// Native source creation may preserve unknown attribution/qualification.
    /// Every *bound* slot is still mandatory; unknowns remain explicit and do
    /// not produce an exact ref. The strict consumer API above is unchanged.
    pub fn preserved_basis<'v>(&mut self,p:&PreparedAccount<'v>)->Result<PreservedBasis<'v>> {
        let mut unresolved=self.vec(0)?;
        let slots=self.collect_basis(p,Some(&mut unresolved))?;
        Ok(PreservedBasis{slots,unresolved})
    }
    fn collect_basis<'v>(&mut self,p:&PreparedAccount<'v>,mut unresolved:Option<&mut Vec<UnresolvedBasisSlot<'v>>>)->Result<Vec<BasisSlot<'v>>> {
        self.admit(p.value)?;
        let mut slots = self.vec(0)?;
        let v = p.value;
        let x = self.field(v, "subject")?;
        self.basis_typed(&mut slots, "subject", 0, 0, x)?;
        for key in ["scope", "world"] {
            let x = self.field(v, key)?;
            self.basis_bound(&mut slots,&mut unresolved,key,0,0,x)?;
        }
        let x = self.field(v, "endpoints")?;
        for (i, x) in self.array(x)?.iter().enumerate() {
            self.basis_typed(&mut slots, "endpoint", i, 0, x)?;
        }
        let x = self.field(v, "basis")?;
        for (i, x) in self.array(x)?.iter().enumerate() {
            let b = self.field(x, "input")?;
            self.basis_bound(&mut slots,&mut unresolved,"basis input",i,0,b)?;
        }
        let q = self.field(v, "qualification")?;
        for key in [
            "signature_ref",
            "predicate_ref",
            "context_ref",
            "scope_ref",
            "qualification_profile_ref",
        ] {
            let b = self.field(q, key)?;
            self.basis_bound(&mut slots,&mut unresolved,key,0,0,b)?;
        }
        let x = self.field(v, "source_locations")?;
        for (i, x) in self.array(x)?.iter().enumerate() {
            for key in ["source_version", "locator_scheme_version", "namespace_ref"] {
                let r = self.field(x, key)?;
                if !r.is_null() {
                    let r = self.exact(r)?;
                    self.push(
                        &mut slots,
                        BasisSlot {
                            role: key,
                            outer: i,
                            inner: 0,
                            reference: r,
                        },
                    )?;
                }
            }
        }
        let x = self.field(v, "supersedes_proposition")?;
        if !x.is_null() {
            let r = self.exact(x)?;
            self.push(
                &mut slots,
                BasisSlot {
                    role: "supersedes proposition",
                    outer: 0,
                    inner: 0,
                    reference: r,
                },
            )?;
        }
        match p.kind {
            Kind::Name => {
                let b = self.field(v, "named_bearer")?;
                self.basis_bound(&mut slots,&mut unresolved,"named bearer",0,0,b)?;
            }
            Kind::Identity => {
                for key in ["left", "right"] {
                    let x = self.field(v, key)?;
                    self.basis_typed(&mut slots, key, 0, 0, x)?;
                }
                for key in ["criterion", "similarity_method"] {
                    let b = self.field(v, key)?;
                    self.basis_bound(&mut slots,&mut unresolved,key,0,0,b)?;
                }
            }
            Kind::Responsibility => {
                let a = self.field(v, "assignees")?;
                match self.str_field(a, "mode")? {
                    "single" | "identified-anonymous" => {
                        let b = self.field(a, "bearer")?;
                        self.basis_typed(&mut slots, "assignee", 0, 0, b)?;
                    }
                    "joint" => {
                        let xs = self.field(a, "bearers")?;
                        for (i, b) in self.array(xs)?.iter().enumerate() {
                            self.basis_typed(&mut slots, "joint assignee", i, 0, b)?;
                        }
                    }
                    "alternatives" => {
                        let xs = self.field(a, "groups")?;
                        for (i, g) in self.array(xs)?.iter().enumerate() {
                            for (j, b) in self.array(g)?.iter().enumerate() {
                                self.basis_typed(&mut slots, "alternative assignee", i, j, b)?;
                            }
                        }
                    }
                    "unestablished" => {}
                    _ => return Err(Error::Malformed("assignee mode")),
                }
            }
        }
        Ok(slots)
    }
    fn basis_bound<'v>(&mut self,out:&mut Vec<BasisSlot<'v>>,missing:&mut Option<&mut Vec<UnresolvedBasisSlot<'v>>>,
        role:&'static str,outer:usize,inner:usize,binding:&'v Value)->Result<()> {
        match self.bound(binding,role) {
            Ok(value)=>self.basis_typed(out,role,outer,inner,value),
            Err(Error::NeedsBinding(_))=>match missing.as_deref_mut() {
                Some(missing)=>self.push(missing,UnresolvedBasisSlot{role,outer,inner,binding}),
                None=>Err(Error::NeedsBinding(role)),
            },
            Err(error)=>Err(error),
        }
    }
    fn basis_typed<'v>(
        &mut self,
        out: &mut Vec<BasisSlot<'v>>,
        role: &'static str,
        outer: usize,
        inner: usize,
        v: &'v Value,
    ) -> Result<()> {
        let (kind, record) = self.typed(v)?;
        self.push(
            out,
            BasisSlot {
                role,
                outer,
                inner: inner.checked_mul(2).ok_or(Error::ResourceLimit)?,
                reference: kind,
            },
        )?;
        self.push(
            out,
            BasisSlot {
                role,
                outer,
                inner: inner
                    .checked_mul(2)
                    .and_then(|x| x.checked_add(1))
                    .ok_or(Error::ResourceLimit)?,
                reference: record,
            },
        )
    }
}
