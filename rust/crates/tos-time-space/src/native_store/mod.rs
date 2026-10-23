//! Native TS input binding. Exact content is checked here; source membership,
//! current fences, schema dispatch and admission remain with their native owner.
mod record;
mod session;
mod codec;
mod store;
pub use record::{NativeIdentityFields, NativeTsRecord, check_native_ts_record};
pub use session::{NativeTsLimits, NativeTsSession, NativeTsUsage};
pub use codec::{codomain_tag, decode_codomain, decode_domain_value, decode_profile_body,
    decode_result_body, encode_domain_value, encode_profile_body, encode_result_body,
    typed_projection, value_codomain};
pub use store::{NativeTsDefinition, NativeTsDescriptor, NativeTsRequest, NativeTsRole,
    NativeTsSelection, NativeTsSourceOwner, NativeTsStore, SelectedNativeTsRecord};
