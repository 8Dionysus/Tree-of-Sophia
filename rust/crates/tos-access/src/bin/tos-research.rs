//! Explicit source-selected research tools, built independently of serving.
use std::io::Write;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "--help" | "-h" | "help") {
        println!(
            "usage: tos-research COMMAND [OPTIONS]\nRun COMMAND --help for its explicit source, budget and output contract.\nResearch builders are separate from the read-only access service.\nCommands:\n  antonovsky-collation\n  authored-canon-bridge\n  bounded-translation-input\n  dta-technical-markup\n  german-triangulation\n  jenseits-label-correspondence\n  jenseits-numbered-structure\n  jenseits-polilov-numbered-structure\n  mysl-transfer-target-structure\n  nietzsche-transfer-source-structure\n  witness-structure-correspondence\n  lexical-index\n  nietzsche-transfer-source-routes\n  opening-sentence-alignment\n  provenance-event-v2-lab\n  semantic-source-recurrence\n  source-text-foundation\n  structural-paragraph\n  synthetic-foundation-lab\n  target-text-foundation\n  technical-markup\n  transfer-candidates\n  transfer-route-readiness\n  transfer-source-passages\n  transfer-source-visible\n  transfer-target-passages\n  zarathustra-morphology-census-result\n  zarathustra-morphology-context\n  zarathustra-morphology-context-result\n  zarathustra-morphology-input\n  zarathustra-recurrence-projection\n  zarathustra-usage-context\n  zarathustra-visual-retrieval-result\n  zarathustra-reading-workbench-v1\n  zarathustra-parallel-lexical-candidates-v1\n  zarathustra-de-ru-paragraph-alignment-v1\n  zarathustra-concept-workbench-v1\n  zarathustra-eternal-return-review-preparation-v1\n  zarathustra-morphology-theme-candidates-v1\n  zarathustra-eternal-return-concept-candidate-v1"
        );
        return;
    }
    type Handler = fn(&[String], &mut dyn Write, &mut dyn Write) -> Option<i32>;
    let handlers: &[Handler] = &[
        tos_access::dta_technical_markup_command::run_if_requested,
        tos_access::transfer_source_passages_command::run_if_requested,
        tos_access::transfer_candidates_command::run_if_requested,
        tos_access::nietzsche_transfer_routes_command::run_if_requested,
        tos_access::jenseits_structure_command::run_if_requested,
        tos_access::visual_result_command::run_if_requested,
        tos_access::morphology_result_command::run_if_requested,
        tos_access::lexical_derivatives_command::run_if_requested,
        tos_access::jenseits_label_command::run_if_requested,
        tos_access::provenance_event_lab_command::run_if_requested,
        tos_access::synthetic_foundation_lab_command::run_if_requested,
        tos_access::authored_canon_bridge_command::run_if_requested,
        tos_access::antonovsky_collation_command::run_if_requested,
        tos_access::bounded_translation_input_command::run_if_requested,
        tos_access::german_triangulation_command::run_if_requested,
        tos_access::transfer_source_visible_command::run_if_requested,
        tos_access::transfer_target_passages_command::run_if_requested,
        tos_access::transfer_metadata_command::run_if_requested,
        tos_access::source_text_foundation_command::run_if_requested,
        tos_access::research_builders_command::run_if_requested,
        tos_access::lexical_index_command::run_if_requested,
        tos_access::technical_markup_command::run_if_requested,
        tos_access::structural_paragraph_command::run_if_requested,
    ];
    for run in handlers {
        if let Some(code) = run(&args, &mut std::io::stdout(), &mut std::io::stderr()) {
            std::process::exit(code);
        }
    }
    eprintln!("unknown research command: {}", args[0]);
    std::process::exit(2);
}
