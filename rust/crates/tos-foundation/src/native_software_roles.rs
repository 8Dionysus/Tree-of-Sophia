//! Native software command roles shared by the build receipt and installed archive readers.
//! This describes delivery only; execution, source selection and owner grants stay separate.
pub const COMMANDS: [&str; 18] = [
    "tos-native-owner-command",
    "tos-schema-worker",
    "tos-validation-lanes",
    "tos-release-check",
    "tos-software-ci",
    "tos-ops-mechanics-plan",
    "tos-constructor-library",
    "tos-constructor-fragments",
    "tos-reader",
    "tos-route-cards",
    "tos-agents-route-harness",
    "tos-kag-provider-controls",
    "tos-kag-release",
    "tos-stats-release",
    "tos-unicode-tables",
    "tos-constructor-desktop",
    "tos-source-registry",
    "tos-open-work-queue",
];
pub const NO_DEFAULT_FEATURES: [&str; 10] = [
    "tos-validation-lanes",
    "tos-release-check",
    "tos-software-ci",
    "tos-route-cards",
    "tos-agents-route-harness",
    "tos-kag-provider-controls",
    "tos-kag-release",
    "tos-stats-release",
    "tos-unicode-tables",
    "tos-constructor-desktop",
];
pub fn package(role: &str) -> Option<&'static str> {
    Some(match role {
        "tos-access" => "tos-access",
        "tos-native-owner-command" => "tos-command",
        "tos-schema-worker" => "tos-validation",
        "tos-constructor-library" | "tos-constructor-fragments" => "tos-compiler",
        "tos-reader" => "tos-reader",
        role if COMMANDS.contains(&role) => "tos-ops-mechanics-plan",
        _ => return None,
    })
}
pub fn features(role: &str) -> Option<&'static [&'static str]> {
    if !COMMANDS.contains(&role) {
        return None;
    }
    Some(match role {
        "tos-ops-mechanics-plan" | "tos-source-registry" | "tos-open-work-queue" => {
            &["compiler-backed-validators", "default"]
        }
        "tos-schema-worker" => &["default", "native"],
        _ => &[],
    })
}
