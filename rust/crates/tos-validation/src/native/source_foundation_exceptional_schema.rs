//! Exceptional LegacyPythonObserved schema evaluation.
//!
//! Finite values stay on the ordinary `jsonschema` validator. This evaluator is
//! entered only when the FND legacy tree contains a Python value that has no
//! faithful finite `serde_json::Value` representation. It walks the selected
//! Draft 2020-12 schema and refuses a success if any assertion/ref semantics in
//! that schema closure are outside this closed interpreter.

use super::*;
use serde_json::{Map, Value};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use tos_foundation::{Digest256, JsonNumberKind, JsonString};

const MAX_SCHEMA_SCAN_WORK: u64 = ExceptionalSchemaUsage::whole().schema_scan_work;
const MAX_SCHEMA_SCAN_BYTES: u64 = ExceptionalSchemaUsage::whole().schema_scan_bytes;
const MAX_EXCEPTIONAL_WORK: u64 = ExceptionalSchemaUsage::whole().evaluation_work;
const MAX_EXCEPTIONAL_WORK_BYTES: u64 = ExceptionalSchemaUsage::whole().evaluation_bytes;
const MAX_EXCEPTIONAL_DEPTH: usize = 64;
const MAX_EXCEPTIONAL_REF_STEPS: u64 = ExceptionalSchemaUsage::whole().reference_steps;
const MAX_EXCEPTIONAL_PATTERN_COUNT: u64 = ExceptionalSchemaUsage::whole().pattern_compile_count;
const MAX_EXCEPTIONAL_PATTERN_BYTES: u64 = ExceptionalSchemaUsage::whole().pattern_bytes;
const MAX_EXCEPTIONAL_REGEX_CHECKS: u64 = ExceptionalSchemaUsage::whole().regex_checks;
const MAX_EXCEPTIONAL_REGEX_BYTES: u64 = ExceptionalSchemaUsage::whole().regex_bytes;
const MAX_EXCEPTIONAL_INTERMEDIATE_ISSUE_BYTES: usize = 1024 * 1024;
const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

fn charge_counter(counter: &mut u64, amount: u64, limit: u64) -> Result<(), ()> {
    let next = counter.checked_add(amount).ok_or(())?;
    if next > limit {
        return Err(());
    }
    *counter = next;
    Ok(())
}

pub(super) fn caps_sha256() -> Digest256 {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-source-foundation-exceptional-schema-caps-v3\0");
    for value in [
        MAX_SCHEMA_SCAN_WORK,
        MAX_SCHEMA_SCAN_BYTES,
        MAX_EXCEPTIONAL_WORK,
        MAX_EXCEPTIONAL_WORK_BYTES,
        MAX_EXCEPTIONAL_DEPTH as u64,
        MAX_EXCEPTIONAL_REF_STEPS,
        MAX_EXCEPTIONAL_PATTERN_COUNT,
        MAX_EXCEPTIONAL_PATTERN_BYTES,
        MAX_EXCEPTIONAL_REGEX_CHECKS,
        MAX_EXCEPTIONAL_REGEX_BYTES,
        MAX_EXCEPTIONAL_INTERMEDIATE_ISSUE_BYTES as u64,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.finalize()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PrepareFailure {
    Unsupported,
    Budget,
}

pub(super) struct PreparationBudget {
    limits: ExceptionalSchemaUsage,
    schema_scan_work: u64,
    schema_scan_bytes: u64,
    pattern_compile_count: u64,
    pattern_bytes: u64,
}

impl PreparationBudget {
    pub(super) fn new(limits: ExceptionalSchemaUsage) -> Result<Self, PrepareFailure> {
        if !limits.fits_within(ExceptionalSchemaUsage::whole()) {
            return Err(PrepareFailure::Budget);
        }
        Ok(Self {
            limits,
            schema_scan_work: 0,
            schema_scan_bytes: 0,
            pattern_compile_count: 0,
            pattern_bytes: 0,
        })
    }

    pub(super) fn usage(&self) -> ExceptionalSchemaUsage {
        ExceptionalSchemaUsage {
            schema_scan_work: self.schema_scan_work,
            schema_scan_bytes: self.schema_scan_bytes,
            pattern_compile_count: self.pattern_compile_count,
            pattern_bytes: self.pattern_bytes,
            ..ExceptionalSchemaUsage::default()
        }
    }

    fn charge_schema_scan(&mut self) -> Result<(), PrepareFailure> {
        charge_counter(&mut self.schema_scan_work, 1, self.limits.schema_scan_work)
            .map_err(|_| PrepareFailure::Budget)
    }

    fn charge_schema_bytes(&mut self, amount: usize) -> Result<(), PrepareFailure> {
        charge_counter(
            &mut self.schema_scan_bytes,
            u64::try_from(amount).map_err(|_| PrepareFailure::Budget)?,
            self.limits.schema_scan_bytes,
        )
        .map_err(|_| PrepareFailure::Budget)
    }

    fn charge_pattern(&mut self, bytes: usize) -> Result<(), PrepareFailure> {
        let bytes = u64::try_from(bytes).map_err(|_| PrepareFailure::Budget)?;
        let next_count = self
            .pattern_compile_count
            .checked_add(1)
            .ok_or(PrepareFailure::Budget)?;
        let next_bytes = self
            .pattern_bytes
            .checked_add(bytes)
            .ok_or(PrepareFailure::Budget)?;
        if next_count > self.limits.pattern_compile_count || next_bytes > self.limits.pattern_bytes
        {
            return Err(PrepareFailure::Budget);
        }
        self.pattern_compile_count = next_count;
        self.pattern_bytes = next_bytes;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SchemaLocation {
    resource_uri: String,
    path: Vec<schema_diagnostics::PathSegment>,
}

impl SchemaLocation {
    fn root(resource_uri: &str) -> Self {
        Self {
            resource_uri: resource_uri.to_owned(),
            path: Vec::new(),
        }
    }

    fn property(&self, name: &str) -> Self {
        let mut output = self.clone();
        output
            .path
            .push(schema_diagnostics::PathSegment::Property(name.to_owned()));
        output
    }

    fn index(&self, index: usize) -> Option<Self> {
        let mut output = self.clone();
        output.path.push(schema_diagnostics::PathSegment::Index(
            u64::try_from(index).ok()?,
        ));
        Some(output)
    }
}

pub(super) struct Plan<'a> {
    resources: &'a BTreeMap<String, Value>,
    root_uri: String,
    patterns: BTreeMap<String, jsonschema::Validator>,
}

impl<'a> Plan<'a> {
    pub(super) fn prepare(
        resources: &'a BTreeMap<String, Value>,
        root_uri: &str,
        budget: &mut PreparationBudget,
    ) -> Result<Self, PrepareFailure> {
        let root = resources.get(root_uri).ok_or(PrepareFailure::Unsupported)?;
        if root.get("$schema").and_then(Value::as_str) != Some(DRAFT_2020_12)
            || root.get("$id").and_then(Value::as_str) != Some(root_uri)
        {
            return Err(PrepareFailure::Unsupported);
        }
        let mut plan = Self {
            resources,
            root_uri: root_uri.to_owned(),
            patterns: BTreeMap::new(),
        };
        let mut visited = BTreeSet::new();
        plan.scan_schema(&SchemaLocation::root(root_uri), 0, &mut visited, budget)?;
        Ok(plan)
    }

    fn scan_schema(
        &mut self,
        location: &SchemaLocation,
        depth: usize,
        visited: &mut BTreeSet<SchemaLocation>,
        budget: &mut PreparationBudget,
    ) -> Result<(), PrepareFailure> {
        budget.charge_schema_scan()?;
        if depth > MAX_EXCEPTIONAL_DEPTH {
            return Err(PrepareFailure::Budget);
        }
        let location_bytes = location
            .resource_uri
            .len()
            .checked_add(path_storage_bytes(&location.path).ok_or(PrepareFailure::Budget)?)
            .ok_or(PrepareFailure::Budget)?;
        budget.charge_schema_bytes(location_bytes)?;
        if !visited.insert(location.clone()) {
            return Ok(());
        }
        let schema = schema_at(self.resources, location).ok_or(PrepareFailure::Unsupported)?;
        let Some(object) = schema.as_object() else {
            return if schema.is_boolean() {
                Ok(())
            } else {
                Err(PrepareFailure::Unsupported)
            };
        };
        if location.path.is_empty() {
            if object.get("$schema").and_then(Value::as_str) != Some(DRAFT_2020_12)
                || object.get("$id").and_then(Value::as_str) != Some(location.resource_uri.as_str())
            {
                return Err(PrepareFailure::Unsupported);
            }
        } else if object.contains_key("$schema") || object.contains_key("$id") {
            // Nested base changes need full URI-scope resolution. They are not
            // present in this selected source-foundation schema family.
            return Err(PrepareFailure::Unsupported);
        }

        for (keyword, value) in object {
            budget.charge_schema_scan()?;
            budget.charge_schema_bytes(keyword.len())?;
            if !supported_keyword(keyword) {
                return Err(PrepareFailure::Unsupported);
            }
            match keyword.as_str() {
                "pattern" => {
                    let pattern = value.as_str().ok_or(PrepareFailure::Unsupported)?;
                    budget.charge_schema_bytes(pattern.len())?;
                    if !self.patterns.contains_key(pattern) {
                        budget.charge_pattern(pattern.len())?;
                        let pattern_schema =
                            serde_json::json!({"type": "string", "pattern": pattern});
                        let validator = jsonschema::options()
                            .with_draft(jsonschema::Draft::Draft202012)
                            .should_validate_formats(true)
                            .should_ignore_unknown_formats(false)
                            .build(&pattern_schema)
                            .map_err(|_| PrepareFailure::Unsupported)?;
                        self.patterns.insert(pattern.to_owned(), validator);
                    }
                }
                "$ref" => {
                    let reference = value.as_str().ok_or(PrepareFailure::Unsupported)?;
                    budget.charge_schema_bytes(reference.len())?;
                    let target =
                        resolve_reference(self.resources, &location.resource_uri, reference)
                            .ok_or(PrepareFailure::Unsupported)?;
                    self.scan_schema(&target, depth + 1, visited, budget)?;
                }
                "$defs" | "properties" => {
                    let children = value.as_object().ok_or(PrepareFailure::Unsupported)?;
                    for (name, child) in children {
                        budget.charge_schema_bytes(name.len())?;
                        let child_location = location.property(keyword).property(name);
                        self.scan_schema(&child_location, depth + 1, visited, budget)?;
                    }
                }
                "items" | "additionalProperties" | "not" | "if" | "then" | "else" => {
                    let child_location = location.property(keyword);
                    self.scan_schema(&child_location, depth + 1, visited, budget)?;
                }
                "allOf" | "anyOf" => {
                    let children = value.as_array().ok_or(PrepareFailure::Unsupported)?;
                    for (index, child) in children.iter().enumerate() {
                        let child_location = location
                            .property(keyword)
                            .index(index)
                            .ok_or(PrepareFailure::Budget)?;
                        self.scan_schema(&child_location, depth + 1, visited, budget)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(super) fn evaluate(
        &self,
        instance: &JsonValue,
        caps: schema_diagnostics::Caps,
        context: &mut EvaluationContext,
    ) -> Evaluation {
        context.active.clear();
        if let Err(failure) = supported_instance(instance, 0, context) {
            return Evaluation::indeterminate(failure, caps);
        }
        let mut collector = IssueCollector::new(caps, true);
        let mut instance_path = Vec::new();
        let root = SchemaLocation::root(&self.root_uri);
        let state = match self.eval_schema(
            &root,
            instance,
            &mut instance_path,
            0,
            context,
            &mut collector,
        ) {
            Ok(state) => state,
            Err(failure) => {
                return Evaluation::from_collector(collector, Some(failure), caps);
            }
        };
        let failure = match state {
            EvalState::Valid | EvalState::Invalid => None,
            EvalState::Indeterminate => Some(EvaluationFailure::Unsupported),
        };
        Evaluation::from_collector(collector, failure, caps)
    }

    fn eval_schema(
        &self,
        location: &SchemaLocation,
        instance: &JsonValue,
        instance_path: &mut Vec<schema_diagnostics::PathSegment>,
        depth: usize,
        context: &mut EvaluationContext,
        output: &mut IssueCollector,
    ) -> Result<EvalState, EvaluationFailure> {
        context.charge_work(1)?;
        if depth > MAX_EXCEPTIONAL_DEPTH {
            return Err(EvaluationFailure::Budget);
        }
        let path_bytes = path_storage_bytes(instance_path)
            .and_then(|bytes| bytes.checked_add(path_storage_bytes(&location.path)?))
            .and_then(|bytes| bytes.checked_add(location.resource_uri.len()))
            .ok_or(EvaluationFailure::Budget)?;
        context.charge_bytes(path_bytes)?;
        let active_key = (location.clone(), instance_path.clone());
        if !context.active.insert(active_key.clone()) {
            return Err(EvaluationFailure::Unsupported);
        }
        let result =
            self.eval_schema_inner(location, instance, instance_path, depth, context, output);
        context.active.remove(&active_key);
        result
    }

    fn eval_schema_inner(
        &self,
        location: &SchemaLocation,
        instance: &JsonValue,
        instance_path: &mut Vec<schema_diagnostics::PathSegment>,
        depth: usize,
        context: &mut EvaluationContext,
        output: &mut IssueCollector,
    ) -> Result<EvalState, EvaluationFailure> {
        let schema = schema_at(self.resources, location).ok_or(EvaluationFailure::Unsupported)?;
        if schema.as_bool() == Some(true) {
            return Ok(EvalState::Valid);
        }
        if schema.as_bool() == Some(false) {
            output.push_generated(
                instance_path,
                location,
                schema_diagnostics::Reason::FalseSchema,
                None,
                None,
                context,
            )?;
            return Ok(EvalState::Invalid);
        }
        let object = schema.as_object().ok_or(EvaluationFailure::Unsupported)?;
        let mut state = EvalState::Valid;

        if let Some(reference) = object.get("$ref") {
            context.charge_reference_step()?;
            let reference = reference.as_str().ok_or(EvaluationFailure::Unsupported)?;
            let target = resolve_reference(self.resources, &location.resource_uri, reference)
                .ok_or(EvaluationFailure::Unsupported)?;
            let child_state =
                self.eval_schema(&target, instance, instance_path, depth + 1, context, output)?;
            state = state.combine(child_state);
        }

        if let Some(type_value) = object.get("type") {
            let (matches, compatibility) = type_matches(instance, type_value, context)?;
            if !matches {
                output.push_generated(
                    instance_path,
                    location,
                    schema_diagnostics::Reason::Type,
                    Some("type"),
                    compatibility,
                    context,
                )?;
                state = state.combine(EvalState::Invalid);
            }
        }

        if let Some(constant) = object.get("const") {
            if !python_equal(instance, constant, context, 0)? {
                output.push_generated(
                    instance_path,
                    location,
                    schema_diagnostics::Reason::Constant,
                    Some("const"),
                    None,
                    context,
                )?;
                state = state.combine(EvalState::Invalid);
            }
        }
        if let Some(values) = object.get("enum") {
            let values = values.as_array().ok_or(EvaluationFailure::Unsupported)?;
            let mut found = false;
            for value in values {
                if python_equal(instance, value, context, 0)? {
                    found = true;
                    break;
                }
            }
            if !found {
                output.push_generated(
                    instance_path,
                    location,
                    schema_diagnostics::Reason::Enum,
                    Some("enum"),
                    None,
                    context,
                )?;
                state = state.combine(EvalState::Invalid);
            }
        }

        self.eval_numeric_bounds(
            object,
            location,
            instance,
            instance_path,
            context,
            output,
            &mut state,
        )?;
        self.eval_string_keywords(
            object,
            location,
            instance,
            instance_path,
            context,
            output,
            &mut state,
        )?;
        self.eval_array_keywords(
            object,
            location,
            instance,
            instance_path,
            depth,
            context,
            output,
            &mut state,
        )?;
        self.eval_object_keywords(
            object,
            location,
            instance,
            instance_path,
            depth,
            context,
            output,
            &mut state,
        )?;

        if let Some(all_of) = object.get("allOf") {
            let branches = all_of.as_array().ok_or(EvaluationFailure::Unsupported)?;
            for (index, _) in branches.iter().enumerate() {
                let child = location
                    .property("allOf")
                    .index(index)
                    .ok_or(EvaluationFailure::Budget)?;
                let branch_state =
                    self.eval_schema(&child, instance, instance_path, depth + 1, context, output)?;
                state = state.combine(branch_state);
            }
        }
        if let Some(any_of) = object.get("anyOf") {
            let branches = any_of.as_array().ok_or(EvaluationFailure::Unsupported)?;
            let mut valid = false;
            let mut indeterminate = false;
            for (index, _) in branches.iter().enumerate() {
                let child = location
                    .property("anyOf")
                    .index(index)
                    .ok_or(EvaluationFailure::Budget)?;
                let mut scratch = IssueCollector::new(output.caps, false);
                match self.eval_schema(
                    &child,
                    instance,
                    instance_path,
                    depth + 1,
                    context,
                    &mut scratch,
                )? {
                    EvalState::Valid => {
                        valid = true;
                        break;
                    }
                    EvalState::Invalid => {}
                    EvalState::Indeterminate => indeterminate = true,
                }
            }
            if !valid {
                if indeterminate {
                    state = state.combine(EvalState::Indeterminate);
                } else {
                    output.push_generated(
                        instance_path,
                        location,
                        schema_diagnostics::Reason::AnyOf,
                        Some("anyOf"),
                        None,
                        context,
                    )?;
                    state = state.combine(EvalState::Invalid);
                }
            }
        }
        if let Some(not_schema) = object.get("not") {
            let child = location.property("not");
            let mut scratch = IssueCollector::new(output.caps, false);
            match self.eval_schema(
                &child,
                instance,
                instance_path,
                depth + 1,
                context,
                &mut scratch,
            )? {
                EvalState::Valid => {
                    output.push_generated(
                        instance_path,
                        location,
                        schema_diagnostics::Reason::Not,
                        Some("not"),
                        None,
                        context,
                    )?;
                    state = state.combine(EvalState::Invalid);
                }
                EvalState::Invalid => {}
                EvalState::Indeterminate => state = state.combine(EvalState::Indeterminate),
            }
            let _ = not_schema;
        }
        if let Some(if_schema) = object.get("if") {
            let condition_location = location.property("if");
            let mut scratch = IssueCollector::new(output.caps, false);
            let condition_state = self.eval_schema(
                &condition_location,
                instance,
                instance_path,
                depth + 1,
                context,
                &mut scratch,
            )?;
            match condition_state {
                EvalState::Indeterminate => state = state.combine(EvalState::Indeterminate),
                EvalState::Valid => {
                    if object.contains_key("then") {
                        let child = location.property("then");
                        let child_state = self.eval_schema(
                            &child,
                            instance,
                            instance_path,
                            depth + 1,
                            context,
                            output,
                        )?;
                        state = state.combine(child_state);
                    }
                }
                EvalState::Invalid => {
                    if object.contains_key("else") {
                        let child = location.property("else");
                        let child_state = self.eval_schema(
                            &child,
                            instance,
                            instance_path,
                            depth + 1,
                            context,
                            output,
                        )?;
                        state = state.combine(child_state);
                    }
                }
            }
            let _ = if_schema;
        }
        Ok(state)
    }

    fn eval_numeric_bounds(
        &self,
        object: &Map<String, Value>,
        location: &SchemaLocation,
        instance: &JsonValue,
        instance_path: &[schema_diagnostics::PathSegment],
        context: &mut EvaluationContext,
        output: &mut IssueCollector,
        state: &mut EvalState,
    ) -> Result<(), EvaluationFailure> {
        let JsonValue::Number(value) = instance else {
            return Ok(());
        };
        for (keyword, reason, operation) in [
            (
                "minimum",
                schema_diagnostics::Reason::Minimum,
                BoundOperation::Gte,
            ),
            (
                "maximum",
                schema_diagnostics::Reason::Maximum,
                BoundOperation::Lte,
            ),
            (
                "exclusiveMinimum",
                schema_diagnostics::Reason::ExclusiveMinimum,
                BoundOperation::Gt,
            ),
            (
                "exclusiveMaximum",
                schema_diagnostics::Reason::ExclusiveMaximum,
                BoundOperation::Lt,
            ),
        ] {
            let Some(bound) = object.get(keyword) else {
                continue;
            };
            let Some(bound) = bound.as_number() else {
                return Err(EvaluationFailure::Unsupported);
            };
            let Some(ordering) = python_number_cmp(value, bound, context)? else {
                // Python float NaN makes every relational expression false.
                // Schema boundary predicates are written as the positive
                // comparison, so none of them emits an error for NaN.
                continue;
            };
            let passes = match operation {
                BoundOperation::Gt => ordering == Ordering::Greater,
                BoundOperation::Gte => ordering != Ordering::Less,
                BoundOperation::Lt => ordering == Ordering::Less,
                BoundOperation::Lte => ordering != Ordering::Greater,
            };
            if !passes {
                output.push_generated(
                    instance_path,
                    location,
                    reason,
                    Some(keyword),
                    None,
                    context,
                )?;
                *state = state.combine(EvalState::Invalid);
            }
        }
        Ok(())
    }

    fn eval_string_keywords(
        &self,
        object: &Map<String, Value>,
        location: &SchemaLocation,
        instance: &JsonValue,
        instance_path: &[schema_diagnostics::PathSegment],
        context: &mut EvaluationContext,
        output: &mut IssueCollector,
        state: &mut EvalState,
    ) -> Result<(), EvaluationFailure> {
        let JsonValue::String(value) = instance else {
            return Ok(());
        };
        let text = value.as_str().ok_or(EvaluationFailure::Unsupported)?;
        context.charge_bytes(text.len())?;
        let length = u64::try_from(text.chars().count()).map_err(|_| EvaluationFailure::Budget)?;
        for (keyword, reason, is_minimum) in [
            ("minLength", schema_diagnostics::Reason::MinimumLength, true),
            (
                "maxLength",
                schema_diagnostics::Reason::MaximumLength,
                false,
            ),
        ] {
            let Some(limit) = object.get(keyword) else {
                continue;
            };
            let limit = limit.as_u64().ok_or(EvaluationFailure::Unsupported)?;
            if (is_minimum && length < limit) || (!is_minimum && length > limit) {
                output.push_generated(
                    instance_path,
                    location,
                    reason,
                    Some(keyword),
                    None,
                    context,
                )?;
                *state = state.combine(EvalState::Invalid);
            }
        }
        if let Some(pattern) = object.get("pattern") {
            let pattern = pattern.as_str().ok_or(EvaluationFailure::Unsupported)?;
            context.charge_regex_check(text.len())?;
            let validator = self
                .patterns
                .get(pattern)
                .ok_or(EvaluationFailure::Unsupported)?;
            context.charge_bytes(text.len())?;
            let finite_string = Value::String(text.to_owned());
            if let Some(error) = validator.iter_errors(&finite_string).next() {
                match error.kind() {
                    jsonschema::error::ValidationErrorKind::Pattern { .. } => {
                        output.push_generated(
                            instance_path,
                            location,
                            schema_diagnostics::Reason::Pattern,
                            Some("pattern"),
                            None,
                            context,
                        )?;
                        *state = state.combine(EvalState::Invalid);
                    }
                    jsonschema::error::ValidationErrorKind::BacktrackLimitExceeded { .. }
                    | jsonschema::error::ValidationErrorKind::RegexEngineFailure { .. } => {
                        *state = state.combine(EvalState::Indeterminate);
                    }
                    _ => *state = state.combine(EvalState::Indeterminate),
                }
            }
        }
        Ok(())
    }

    fn eval_array_keywords(
        &self,
        object: &Map<String, Value>,
        location: &SchemaLocation,
        instance: &JsonValue,
        instance_path: &mut Vec<schema_diagnostics::PathSegment>,
        depth: usize,
        context: &mut EvaluationContext,
        output: &mut IssueCollector,
        state: &mut EvalState,
    ) -> Result<(), EvaluationFailure> {
        let JsonValue::Array(items) = instance else {
            return Ok(());
        };
        let length = u64::try_from(items.len()).map_err(|_| EvaluationFailure::Budget)?;
        for (keyword, reason, is_minimum) in [
            ("minItems", schema_diagnostics::Reason::MinimumItems, true),
            ("maxItems", schema_diagnostics::Reason::MaximumItems, false),
        ] {
            let Some(limit) = object.get(keyword) else {
                continue;
            };
            let limit = limit.as_u64().ok_or(EvaluationFailure::Unsupported)?;
            if (is_minimum && length < limit) || (!is_minimum && length > limit) {
                output.push_generated(
                    instance_path,
                    location,
                    reason,
                    Some(keyword),
                    None,
                    context,
                )?;
                *state = state.combine(EvalState::Invalid);
            }
        }
        if let Some(item_schema) = object.get("items") {
            let child = location.property("items");
            for (index, item) in items.iter().enumerate() {
                context.charge_work(1)?;
                instance_path.push(schema_diagnostics::PathSegment::Index(
                    u64::try_from(index).map_err(|_| EvaluationFailure::Budget)?,
                ));
                let child_state =
                    self.eval_schema(&child, item, instance_path, depth + 1, context, output);
                instance_path.pop();
                *state = state.combine(child_state?);
            }
            let _ = item_schema;
        }
        Ok(())
    }

    fn eval_object_keywords(
        &self,
        object: &Map<String, Value>,
        location: &SchemaLocation,
        instance: &JsonValue,
        instance_path: &mut Vec<schema_diagnostics::PathSegment>,
        depth: usize,
        context: &mut EvaluationContext,
        output: &mut IssueCollector,
        state: &mut EvalState,
    ) -> Result<(), EvaluationFailure> {
        let JsonValue::Object(entries) = instance else {
            return Ok(());
        };
        let length = u64::try_from(entries.len()).map_err(|_| EvaluationFailure::Budget)?;
        for (keyword, reason, is_minimum) in [
            (
                "minProperties",
                schema_diagnostics::Reason::MinimumProperties,
                true,
            ),
            (
                "maxProperties",
                schema_diagnostics::Reason::MaximumProperties,
                false,
            ),
        ] {
            let Some(limit) = object.get(keyword) else {
                continue;
            };
            let limit = limit.as_u64().ok_or(EvaluationFailure::Unsupported)?;
            if (is_minimum && length < limit) || (!is_minimum && length > limit) {
                output.push_generated(
                    instance_path,
                    location,
                    reason,
                    Some(keyword),
                    None,
                    context,
                )?;
                *state = state.combine(EvalState::Invalid);
            }
        }

        let properties = object.get("properties").and_then(Value::as_object);
        if let Some(required) = object.get("required") {
            let required = required.as_array().ok_or(EvaluationFailure::Unsupported)?;
            for name in required {
                context.charge_work(1)?;
                let name = name.as_str().ok_or(EvaluationFailure::Unsupported)?;
                if !object_has(entries, name, context)? {
                    output.push_generated(
                        instance_path,
                        location,
                        schema_diagnostics::Reason::Required,
                        Some("required"),
                        None,
                        context,
                    )?;
                    *state = state.combine(EvalState::Invalid);
                }
            }
        }

        if let Some(property_schemas) = properties {
            for (name, child_schema) in property_schemas {
                let Some(value) = object_get(entries, name, context)? else {
                    continue;
                };
                context.charge_work(1)?;
                instance_path.push(schema_diagnostics::PathSegment::Property(name.clone()));
                let child = location.property("properties").property(name);
                let child_state =
                    self.eval_schema(&child, value, instance_path, depth + 1, context, output);
                instance_path.pop();
                *state = state.combine(child_state?);
                let _ = child_schema;
            }
        }

        if let Some(additional) = object.get("additionalProperties") {
            let additional_location = location.property("additionalProperties");
            let mut has_forbidden_additional = false;
            for (key, value) in entries {
                let key = key.as_str().ok_or(EvaluationFailure::Unsupported)?;
                if let Some(properties) = properties {
                    context.charge_bytes(key.len())?;
                    if properties.contains_key(key) {
                        continue;
                    }
                }
                context.charge_work(1)?;
                if additional.as_bool() == Some(false) {
                    has_forbidden_additional = true;
                } else if additional.is_object() || additional.as_bool() == Some(true) {
                    if additional.as_bool() != Some(true) {
                        instance_path
                            .push(schema_diagnostics::PathSegment::Property(key.to_owned()));
                        let child_state = self.eval_schema(
                            &additional_location,
                            value,
                            instance_path,
                            depth + 1,
                            context,
                            output,
                        );
                        instance_path.pop();
                        *state = state.combine(child_state?);
                    }
                } else {
                    return Err(EvaluationFailure::Unsupported);
                }
            }
            if has_forbidden_additional {
                output.push_generated(
                    instance_path,
                    location,
                    schema_diagnostics::Reason::AdditionalProperties,
                    Some("additionalProperties"),
                    None,
                    context,
                )?;
                *state = state.combine(EvalState::Invalid);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EvaluationFailure {
    Unsupported,
    Budget,
}

pub(super) struct Evaluation {
    pub issues: Vec<schema_diagnostics::Issue>,
    pub total_issue_count: u64,
    pub truncated: bool,
    pub failure: Option<EvaluationFailure>,
}

impl Evaluation {
    fn indeterminate(failure: EvaluationFailure, caps: schema_diagnostics::Caps) -> Self {
        Self::from_collector(IssueCollector::new(caps, true), Some(failure), caps)
    }

    fn from_collector(
        mut collector: IssueCollector,
        failure: Option<EvaluationFailure>,
        caps: schema_diagnostics::Caps,
    ) -> Self {
        let mut issues = collector.retained.into_vec();
        issues.sort();
        let mut kept = Vec::with_capacity(issues.len());
        let mut report_bytes = super::DIAGNOSTIC_UNIT_HEADER_BYTES;
        for issue in issues {
            let Some(payload) = schema_diagnostics::issue_payload(&issue) else {
                collector.truncated = true;
                continue;
            };
            let Some(next) = report_bytes.checked_add(payload.len()) else {
                collector.truncated = true;
                continue;
            };
            if next > caps.max_report_bytes_per_unit as usize {
                collector.truncated = true;
                continue;
            }
            report_bytes = next;
            kept.push(issue);
        }
        Self {
            issues: kept,
            total_issue_count: collector.total,
            truncated: collector.truncated,
            failure,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoundOperation {
    Lt,
    Lte,
    Gt,
    Gte,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvalState {
    Valid,
    Invalid,
    Indeterminate,
}

impl EvalState {
    fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Self::Indeterminate, _) | (_, Self::Indeterminate) => Self::Indeterminate,
            (Self::Invalid, _) | (_, Self::Invalid) => Self::Invalid,
            _ => Self::Valid,
        }
    }
}

pub(super) struct EvaluationContext {
    limits: ExceptionalSchemaUsage,
    work: u64,
    bytes: u64,
    reference_steps: u64,
    regex_checks: u64,
    regex_bytes: u64,
    active: BTreeSet<(SchemaLocation, Vec<schema_diagnostics::PathSegment>)>,
}

impl EvaluationContext {
    pub(super) fn new(limits: ExceptionalSchemaUsage) -> Result<Self, EvaluationFailure> {
        if !limits.fits_within(ExceptionalSchemaUsage::whole()) {
            return Err(EvaluationFailure::Budget);
        }
        Ok(Self {
            limits,
            work: 0,
            bytes: 0,
            reference_steps: 0,
            regex_checks: 0,
            regex_bytes: 0,
            active: BTreeSet::new(),
        })
    }

    pub(super) fn usage(&self) -> ExceptionalSchemaUsage {
        ExceptionalSchemaUsage {
            evaluation_work: self.work,
            evaluation_bytes: self.bytes,
            reference_steps: self.reference_steps,
            regex_checks: self.regex_checks,
            regex_bytes: self.regex_bytes,
            ..ExceptionalSchemaUsage::default()
        }
    }

    fn charge_work(&mut self, amount: usize) -> Result<(), EvaluationFailure> {
        charge_counter(
            &mut self.work,
            u64::try_from(amount).map_err(|_| EvaluationFailure::Budget)?,
            self.limits.evaluation_work,
        )
        .map_err(|_| EvaluationFailure::Budget)
    }

    fn charge_bytes(&mut self, amount: usize) -> Result<(), EvaluationFailure> {
        charge_counter(
            &mut self.bytes,
            u64::try_from(amount).map_err(|_| EvaluationFailure::Budget)?,
            self.limits.evaluation_bytes,
        )
        .map_err(|_| EvaluationFailure::Budget)
    }

    fn charge_reference_step(&mut self) -> Result<(), EvaluationFailure> {
        charge_counter(&mut self.reference_steps, 1, self.limits.reference_steps)
            .map_err(|_| EvaluationFailure::Budget)
    }

    fn charge_regex_check(&mut self, bytes: usize) -> Result<(), EvaluationFailure> {
        let bytes = u64::try_from(bytes).map_err(|_| EvaluationFailure::Budget)?;
        let next_checks = self
            .regex_checks
            .checked_add(1)
            .ok_or(EvaluationFailure::Budget)?;
        let next_bytes = self
            .regex_bytes
            .checked_add(bytes)
            .ok_or(EvaluationFailure::Budget)?;
        if next_checks > self.limits.regex_checks || next_bytes > self.limits.regex_bytes {
            return Err(EvaluationFailure::Budget);
        }
        self.regex_checks = next_checks;
        self.regex_bytes = next_bytes;
        Ok(())
    }
}

struct IssueCollector {
    caps: schema_diagnostics::Caps,
    retain: bool,
    retained: BinaryHeap<schema_diagnostics::Issue>,
    total: u64,
    truncated: bool,
    generated_bytes: usize,
}

impl IssueCollector {
    fn new(caps: schema_diagnostics::Caps, retain: bool) -> Self {
        Self {
            caps,
            retain,
            retained: BinaryHeap::with_capacity(if retain {
                caps.max_issues_per_unit as usize
            } else {
                0
            }),
            total: 0,
            truncated: false,
            generated_bytes: 0,
        }
    }

    fn push_generated(
        &mut self,
        instance_path: &[schema_diagnostics::PathSegment],
        schema_location: &SchemaLocation,
        reason: schema_diagnostics::Reason,
        keyword: Option<&str>,
        compatibility_text: Option<schema_diagnostics::CompatibilityText>,
        context: &mut EvaluationContext,
    ) -> Result<(), EvaluationFailure> {
        self.total = self.total.saturating_add(1);
        if !self.retain {
            return Ok(());
        }
        let schema_path_bytes = path_storage_bytes(&schema_location.path)
            .and_then(|bytes| bytes.checked_add(keyword.map_or(0, str::len)));
        if !path_within_caps(instance_path, self.caps)
            || schema_path_bytes.is_none_or(|bytes| {
                bytes > self.caps.max_path_bytes as usize
                    || schema_location.path.len() + usize::from(keyword.is_some())
                        > self.caps.max_path_segments as usize
            })
        {
            self.truncated = true;
            return Ok(());
        }
        let copy_bytes = path_storage_bytes(instance_path)
            .and_then(|bytes| bytes.checked_add(path_storage_bytes(&schema_location.path)?))
            .and_then(|bytes| bytes.checked_add(schema_location.resource_uri.len()))
            .and_then(|bytes| bytes.checked_add(keyword.map_or(0, str::len)))
            .and_then(|bytes| bytes.checked_add(reason.schema_keyword().len()))
            .ok_or(EvaluationFailure::Budget)?;
        context.charge_bytes(copy_bytes)?;
        let mut issue = make_issue(instance_path, schema_location, reason, keyword);
        issue.compatibility_text = compatibility_text;
        self.retain_issue(issue);
        Ok(())
    }

    fn retain_issue(&mut self, issue: schema_diagnostics::Issue) {
        if !self.retain {
            return;
        }
        if !path_within_caps(&issue.instance_path, self.caps)
            || !path_within_caps(&issue.schema_path, self.caps)
            || issue.schema_keyword.len() > 64
        {
            self.truncated = true;
            return;
        }
        let Some(payload) = schema_diagnostics::issue_payload(&issue) else {
            self.truncated = true;
            return;
        };
        let Some(next) = self.generated_bytes.checked_add(payload.len()) else {
            self.truncated = true;
            return;
        };
        if next > MAX_EXCEPTIONAL_INTERMEDIATE_ISSUE_BYTES {
            self.truncated = true;
            return;
        }
        self.generated_bytes = next;
        if self.retained.len() < self.caps.max_issues_per_unit as usize {
            self.retained.push(issue);
        } else {
            self.truncated = true;
            if self
                .retained
                .peek()
                .is_some_and(|largest| issue.cmp(largest) == Ordering::Less)
            {
                let _ = self.retained.pop();
                self.retained.push(issue);
            }
        }
    }
}

fn path_storage_bytes(path: &[schema_diagnostics::PathSegment]) -> Option<usize> {
    path.iter().try_fold(0usize, |total, segment| {
        let bytes = match segment {
            schema_diagnostics::PathSegment::Property(value) => value.len(),
            schema_diagnostics::PathSegment::Index(_) => std::mem::size_of::<u64>(),
        };
        total.checked_add(bytes)?.checked_add(1)
    })
}

fn path_within_caps(
    path: &[schema_diagnostics::PathSegment],
    caps: schema_diagnostics::Caps,
) -> bool {
    if path.len() > caps.max_path_segments as usize {
        return false;
    }
    path.iter()
        .try_fold(0usize, |total, segment| {
            let amount = match segment {
                schema_diagnostics::PathSegment::Property(value) => value.len(),
                schema_diagnostics::PathSegment::Index(_) => std::mem::size_of::<u64>(),
            };
            total.checked_add(amount)
        })
        .is_some_and(|total| total <= caps.max_path_bytes as usize)
}

fn make_issue(
    instance_path: &[schema_diagnostics::PathSegment],
    schema_location: &SchemaLocation,
    reason: schema_diagnostics::Reason,
    keyword: Option<&str>,
) -> schema_diagnostics::Issue {
    let mut schema_path = schema_location.path.clone();
    if let Some(keyword) = keyword {
        schema_path.push(schema_diagnostics::PathSegment::Property(
            keyword.to_owned(),
        ));
    }
    let compatibility_text = None;
    schema_diagnostics::Issue {
        instance_path: instance_path.to_vec(),
        schema_keyword: reason.schema_keyword().to_owned(),
        reason,
        schema_path,
        compatibility_text,
    }
}

fn supported_keyword(keyword: &str) -> bool {
    matches!(
        keyword,
        "$defs"
            | "$id"
            | "$ref"
            | "$schema"
            | "additionalProperties"
            | "allOf"
            | "anyOf"
            | "const"
            | "default"
            | "deprecated"
            | "description"
            | "else"
            | "enum"
            | "exclusiveMaximum"
            | "exclusiveMinimum"
            | "if"
            | "items"
            | "maxItems"
            | "maxLength"
            | "maxProperties"
            | "maximum"
            | "minItems"
            | "minLength"
            | "minProperties"
            | "minimum"
            | "not"
            | "pattern"
            | "properties"
            | "required"
            | "then"
            | "title"
            | "type"
    )
}

fn schema_at<'a>(
    resources: &'a BTreeMap<String, Value>,
    location: &SchemaLocation,
) -> Option<&'a Value> {
    let mut current = resources.get(&location.resource_uri)?;
    for segment in &location.path {
        current = match segment {
            schema_diagnostics::PathSegment::Property(name) => current.as_object()?.get(name)?,
            schema_diagnostics::PathSegment::Index(index) => {
                current.as_array()?.get(usize::try_from(*index).ok()?)?
            }
        };
    }
    Some(current)
}

fn resolve_reference(
    resources: &BTreeMap<String, Value>,
    current_uri: &str,
    reference: &str,
) -> Option<SchemaLocation> {
    let (base, fragment) = reference
        .split_once('#')
        .map_or((reference, None), |(base, fragment)| (base, Some(fragment)));
    let resource_uri = if base.is_empty() {
        current_uri
    } else if resources.contains_key(base) {
        base
    } else {
        return None;
    };
    let Some(fragment) = fragment else {
        return Some(SchemaLocation::root(resource_uri));
    };
    if fragment.is_empty() {
        return Some(SchemaLocation::root(resource_uri));
    }
    if !fragment.starts_with('/') || fragment.contains('%') {
        return None;
    }
    let mut current = resources.get(resource_uri)?;
    let mut path = Vec::new();
    for raw_token in fragment[1..].split('/') {
        let token = decode_pointer_token(raw_token)?;
        let segment = if let Some(object) = current.as_object() {
            let _ = object.get(&token)?;
            schema_diagnostics::PathSegment::Property(token)
        } else if let Some(array) = current.as_array() {
            if token.len() > 1 && token.starts_with('0') {
                return None;
            }
            let index = token.parse::<usize>().ok()?;
            let _ = array.get(index)?;
            schema_diagnostics::PathSegment::Index(u64::try_from(index).ok()?)
        } else {
            return None;
        };
        current = match &segment {
            schema_diagnostics::PathSegment::Property(name) => current.get(name)?,
            schema_diagnostics::PathSegment::Index(index) => {
                current.get(usize::try_from(*index).ok()?)?
            }
        };
        path.push(segment);
    }
    Some(SchemaLocation {
        resource_uri: resource_uri.to_owned(),
        path,
    })
}

fn decode_pointer_token(token: &str) -> Option<String> {
    if !token.contains('~') {
        return Some(token.to_owned());
    }
    let mut output = String::with_capacity(token.len());
    let mut chars = token.chars();
    while let Some(character) = chars.next() {
        if character != '~' {
            output.push(character);
            continue;
        }
        output.push(match chars.next()? {
            '0' => '~',
            '1' => '/',
            _ => return None,
        });
    }
    Some(output)
}

fn supported_instance(
    value: &JsonValue,
    depth: usize,
    context: &mut EvaluationContext,
) -> Result<(), EvaluationFailure> {
    context.charge_work(1)?;
    if depth > MAX_EXCEPTIONAL_DEPTH {
        return Err(EvaluationFailure::Budget);
    }
    match value {
        JsonValue::Null | JsonValue::Bool(_) => Ok(()),
        JsonValue::Number(number) => {
            context.charge_bytes(number.lexeme.len())?;
            match number.kind {
                JsonNumberKind::Int => Ok(()),
                JsonNumberKind::Float => number
                    .as_python_float()
                    .map(|_| ())
                    .ok_or(EvaluationFailure::Unsupported),
            }
        }
        JsonValue::String(value) => {
            let text = value.as_str().ok_or(EvaluationFailure::Unsupported)?;
            context.charge_bytes(text.len())
        }
        JsonValue::Array(items) => {
            for item in items {
                supported_instance(item, depth + 1, context)?;
            }
            Ok(())
        }
        JsonValue::Object(entries) => {
            for (key, item) in entries {
                let key = key.as_str().ok_or(EvaluationFailure::Unsupported)?;
                context.charge_bytes(key.len())?;
                supported_instance(item, depth + 1, context)?;
            }
            Ok(())
        }
    }
}

fn type_matches(
    instance: &JsonValue,
    schema_type: &Value,
    context: &mut EvaluationContext,
) -> Result<(bool, Option<schema_diagnostics::CompatibilityText>), EvaluationFailure> {
    let types: Vec<&str> = if let Some(type_name) = schema_type.as_str() {
        vec![type_name]
    } else if let Some(type_names) = schema_type.as_array() {
        type_names.iter().filter_map(Value::as_str).collect()
    } else {
        return Ok((false, None));
    };
    let mut matches = false;
    for type_name in &types {
        let type_match = match (type_name.as_ref(), instance) {
            ("null", JsonValue::Null) => true,
            ("boolean", JsonValue::Bool(_)) => true,
            ("object", JsonValue::Object(_)) => true,
            ("array", JsonValue::Array(_)) => true,
            ("string", JsonValue::String(_)) => true,
            ("number", JsonValue::Number(_)) => true,
            ("integer", JsonValue::Number(number)) => match number.kind {
                JsonNumberKind::Int => true,
                JsonNumberKind::Float => {
                    context.charge_bytes(number.lexeme.len())?;
                    number
                        .as_python_float()
                        .is_some_and(|value| value.is_finite() && value.fract() == 0.0)
                }
            },
            _ => false,
        };
        matches |= type_match;
        if matches {
            break;
        }
    }
    let compatibility = if matches || !matches!(instance, JsonValue::Null) || types.len() != 1 {
        None
    } else {
        match types[0] {
            "object" => Some(schema_diagnostics::CompatibilityText::NullIsNotObject),
            "array" => Some(schema_diagnostics::CompatibilityText::NullIsNotArray),
            _ => None,
        }
    };
    Ok((matches, compatibility))
}

fn object_has(
    entries: &[(JsonString, JsonValue)],
    name: &str,
    context: &mut EvaluationContext,
) -> Result<bool, EvaluationFailure> {
    for (key, _) in entries {
        context.charge_work(1)?;
        let key = key.as_str().ok_or(EvaluationFailure::Unsupported)?;
        context.charge_bytes(key.len().max(name.len()))?;
        if key == name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn object_get<'a>(
    entries: &'a [(JsonString, JsonValue)],
    name: &str,
    context: &mut EvaluationContext,
) -> Result<Option<&'a JsonValue>, EvaluationFailure> {
    for (key, value) in entries {
        context.charge_work(1)?;
        let key = key.as_str().ok_or(EvaluationFailure::Unsupported)?;
        context.charge_bytes(key.len().max(name.len()))?;
        if key == name {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

fn python_equal(
    instance: &JsonValue,
    schema_value: &Value,
    context: &mut EvaluationContext,
    depth: usize,
) -> Result<bool, EvaluationFailure> {
    context.charge_work(1)?;
    if depth > MAX_EXCEPTIONAL_DEPTH {
        return Err(EvaluationFailure::Budget);
    }
    match (instance, schema_value) {
        (JsonValue::Null, Value::Null) => Ok(true),
        (JsonValue::Bool(left), Value::Bool(right)) => Ok(left == right),
        (JsonValue::Number(left), Value::Number(right)) => {
            Ok(python_number_cmp_to_schema(left, right, context)? == Some(Ordering::Equal))
        }
        (JsonValue::String(left), Value::String(right)) => {
            let left = left.as_str().ok_or(EvaluationFailure::Unsupported)?;
            context.charge_bytes(left.len().max(right.len()))?;
            Ok(left == right)
        }
        (JsonValue::Array(left), Value::Array(right)) => {
            if left.len() != right.len() {
                return Ok(false);
            }
            for (left, right) in left.iter().zip(right) {
                if !python_equal(left, right, context, depth + 1)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (JsonValue::Object(left), Value::Object(right)) => {
            if left.len() != right.len() {
                return Ok(false);
            }
            for (left_key, left_value) in left {
                let left_key = left_key.as_str().ok_or(EvaluationFailure::Unsupported)?;
                context.charge_bytes(left_key.len())?;
                let Some(right_value) = right.get(left_key) else {
                    return Ok(false);
                };
                if !python_equal(left_value, right_value, context, depth + 1)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn python_number_cmp_to_schema(
    instance: &tos_foundation::JsonNumber,
    schema_value: &serde_json::Number,
    context: &mut EvaluationContext,
) -> Result<Option<Ordering>, EvaluationFailure> {
    let schema_text = schema_value.to_string();
    context.charge_bytes(
        instance
            .lexeme
            .len()
            .checked_add(schema_text.len())
            .ok_or(EvaluationFailure::Budget)?,
    )?;
    let schema_is_float = schema_text.contains(['.', 'e', 'E']);
    let ordering = (|| match instance.kind {
        JsonNumberKind::Int => {
            if schema_is_float {
                cmp_integer_to_float(&instance.lexeme, schema_text.parse().ok()?)
            } else {
                Some(compare_integer_lexemes(&instance.lexeme, &schema_text))
            }
        }
        JsonNumberKind::Float => {
            let left = instance.as_python_float()?;
            if schema_is_float {
                left.partial_cmp(&schema_text.parse::<f64>().ok()?)
            } else {
                cmp_float_to_integer(left, &schema_text)
            }
        }
    })();
    Ok(ordering)
}

fn python_number_cmp(
    instance: &tos_foundation::JsonNumber,
    schema_value: &serde_json::Number,
    context: &mut EvaluationContext,
) -> Result<Option<Ordering>, EvaluationFailure> {
    python_number_cmp_to_schema(instance, schema_value, context)
}

fn cmp_integer_to_float(integer: &str, float: f64) -> Option<Ordering> {
    if float.is_nan() {
        return None;
    }
    if float == f64::INFINITY {
        return Some(Ordering::Less);
    }
    if float == f64::NEG_INFINITY {
        return Some(Ordering::Greater);
    }
    let trunc = format!("{:.0}", float.trunc());
    let compared = compare_integer_lexemes(integer, &trunc);
    if float.fract() == 0.0 {
        Some(compared)
    } else if float.is_sign_positive() {
        Some(match compared {
            Ordering::Less | Ordering::Equal => Ordering::Less,
            Ordering::Greater => Ordering::Greater,
        })
    } else {
        Some(match compared {
            Ordering::Less => Ordering::Less,
            Ordering::Equal | Ordering::Greater => Ordering::Greater,
        })
    }
}

fn cmp_float_to_integer(float: f64, integer: &str) -> Option<Ordering> {
    cmp_integer_to_float(integer, float).map(Ordering::reverse)
}

fn compare_integer_lexemes(left: &str, right: &str) -> Ordering {
    let (left_negative, left_magnitude) = canonical_integer(left);
    let (right_negative, right_magnitude) = canonical_integer(right);
    match (left_negative, right_negative) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => left_magnitude
            .len()
            .cmp(&right_magnitude.len())
            .then_with(|| left_magnitude.cmp(right_magnitude)),
        (true, true) => right_magnitude
            .len()
            .cmp(&left_magnitude.len())
            .then_with(|| right_magnitude.cmp(left_magnitude)),
    }
}

fn canonical_integer(value: &str) -> (bool, &str) {
    let negative = value.starts_with('-');
    let magnitude = value.strip_prefix('-').unwrap_or(value);
    let magnitude = magnitude.trim_start_matches('0');
    let magnitude = if magnitude.is_empty() { "0" } else { magnitude };
    (negative && magnitude != "0", magnitude)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_numeric_edges_keep_nan_non_reflexive_and_int_float_equality() {
        assert_eq!(compare_integer_lexemes("-10", "-2"), Ordering::Less);
        assert_eq!(cmp_integer_to_float("1", 1.0), Some(Ordering::Equal));
        assert_eq!(cmp_integer_to_float("1", 1.5), Some(Ordering::Less));
        assert_eq!(cmp_integer_to_float("2", 1.5), Some(Ordering::Greater));
        assert_eq!(cmp_float_to_integer(f64::NAN, "0"), None);
    }

    #[test]
    fn inventory_nan_does_not_fail_exclusive_minimum() {
        let source =
            include_str!("../../../../../ToS/contracts/source-resource-inventory.schema.json");
        let schema: Value = serde_json::from_str(source).unwrap();
        let root_uri = schema
            .get("$id")
            .and_then(Value::as_str)
            .unwrap()
            .to_owned();
        let mut resources = BTreeMap::new();
        resources.insert(root_uri.clone(), schema);
        let limits = ExceptionalSchemaUsage::whole();
        let mut preparation = PreparationBudget::new(limits).unwrap();
        let plan = Plan::prepare(&resources, &root_uri, &mut preparation).unwrap();
        let instance =
            parse_legacy_python_observed_tree(br#"{"locator":{"width_points":NaN}}"#).unwrap();
        let mut context = EvaluationContext::new(limits).unwrap();
        let result = plan.evaluate(&instance, schema_diagnostics::Caps::CURRENT, &mut context);
        let width_path = vec![
            schema_diagnostics::PathSegment::Property("locator".to_owned()),
            schema_diagnostics::PathSegment::Property("width_points".to_owned()),
        ];
        assert_eq!(result.failure, None);
        assert!(!result.issues.iter().any(|issue| {
            issue.instance_path == width_path
                && issue.reason == schema_diagnostics::Reason::ExclusiveMinimum
        }));
    }
}
