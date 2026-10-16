//! Supported lexical source profile remains Unicode 16 under dependency updates.
use std::collections::BTreeSet;
use tos_compiler::zarathustra_lexical::{LexicalLimits, normalize_form, word_spans};
#[test]
fn unicode16_letter_mark_and_nfc_casefold_are_exact() {
    assert_eq!(unicode_normalization::UNICODE_VERSION, (16, 0, 0));
    let j = BTreeSet::new();
    // U+11DB0 is unassigned in the owner-pinned Python Unicode 16 oracle.
    // A future regex-table update cannot silently widen this maintained profile.
    assert!(word_spans("\u{11db0}", &j, 100).unwrap().is_empty());
    assert_eq!(
        word_spans("\u{1c89}\u{0301}", &j, 100).unwrap()[0].2,
        "\u{1c89}\u{0301}"
    );
    assert_eq!(
        normalize_form("\u{1c89}İΣẞ", LexicalLimits::maintained()).unwrap(),
        "\u{1c8a}i\u{0307}σss"
    );
}
