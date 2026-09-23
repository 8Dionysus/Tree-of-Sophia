use std::collections::BTreeMap;

use crate::digest::Digest256;
use crate::error::{FoundationError, FoundationErrorCode as Code, Result};

pub const DESCRIPTOR_FORMAT_VERSION: &str = "tos_foundation_descriptors_v1";

/// Authored contract identity. Foundation knows no fixed corpus families.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ContractKey {
    pub kind: String,
    pub version: String,
}

impl ContractKey {
    pub fn new(kind: &str, version: &str) -> Result<Self> {
        validate_name(kind)?;
        validate_name(version)?;
        Ok(Self { kind: kind.to_owned(), version: version.to_owned() })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractDescriptor {
    pub key: ContractKey,
    pub source_ref: String,
    pub schema_digest: Digest256,
    pub codec_profile: String,
}

impl ContractDescriptor {
    pub fn new(key: ContractKey, source_ref: &str, schema_digest: Digest256, codec_profile: &str) -> Result<Self> {
        validate_name(source_ref)?;
        validate_name(codec_profile)?;
        Ok(Self { key, source_ref: source_ref.to_owned(), schema_digest, codec_profile: codec_profile.to_owned() })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationEffect { Read, WriteIntent }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationDescriptor {
    pub operation_id: String,
    pub version: String,
    pub input_contract: ContractKey,
    pub output_contract: ContractKey,
    pub effect: OperationEffect,
}

impl OperationDescriptor {
    pub fn new(operation_id: &str, version: &str, input_contract: ContractKey, output_contract: ContractKey, effect: OperationEffect) -> Result<Self> {
        validate_name(operation_id)?;
        validate_name(version)?;
        Ok(Self { operation_id: operation_id.to_owned(), version: version.to_owned(), input_contract, output_contract, effect })
    }
}

/// In-memory descriptor catalogue; registration does not validate a JSON Schema or grant authority.
#[derive(Clone, Debug, Default)]
pub struct DescriptorRegistry {
    contracts: BTreeMap<ContractKey, ContractDescriptor>,
    operations: BTreeMap<(String, String), OperationDescriptor>,
}

impl DescriptorRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register_contract(&mut self, descriptor: ContractDescriptor) -> Result<()> {
        if self.contracts.contains_key(&descriptor.key) {
            return Err(FoundationError::new(Code::DuplicateDescriptor, "contract key already registered"));
        }
        self.contracts.insert(descriptor.key.clone(), descriptor);
        Ok(())
    }

    pub fn register_operation(&mut self, descriptor: OperationDescriptor) -> Result<()> {
        if !self.contracts.contains_key(&descriptor.input_contract) || !self.contracts.contains_key(&descriptor.output_contract) {
            return Err(FoundationError::new(Code::InvalidDescriptor, "operation contract is not registered"));
        }
        let key = (descriptor.operation_id.clone(), descriptor.version.clone());
        if self.operations.contains_key(&key) {
            return Err(FoundationError::new(Code::DuplicateDescriptor, "operation key already registered"));
        }
        self.operations.insert(key, descriptor);
        Ok(())
    }

    pub fn contract(&self, key: &ContractKey) -> Option<&ContractDescriptor> { self.contracts.get(key) }
    pub fn operation(&self, id: &str, version: &str) -> Option<&OperationDescriptor> {
        self.operations.get(&(id.to_owned(), version.to_owned()))
    }
    pub fn contracts(&self) -> impl Iterator<Item = &ContractDescriptor> { self.contracts.values() }
    pub fn operations(&self) -> impl Iterator<Item = &OperationDescriptor> { self.operations.values() }
}

fn validate_name(value: &str) -> Result<()> {
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(FoundationError::new(Code::InvalidDescriptor, "descriptor field is empty or contains a control character"));
    }
    Ok(())
}
