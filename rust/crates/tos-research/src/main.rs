//! Explicit source-selected research tools, built independently of serving.
mod antonovsky_collation_command;
mod authored_canon_bridge_command;
mod bounded_translation_input_command;
mod curation_command;
mod dta_technical_markup_command;
mod german_triangulation_command;
mod jenseits_label_command;
mod jenseits_structure_command;
mod lexical_derivatives_command;
mod lexical_index_command;
mod morphology_result_command;
mod nietzsche_transfer_routes_command;
// The numbered builder adopts this matching core when source navigation moves here.
#[allow(dead_code)]
mod numbered_matching;
mod numbered_structure;
mod provenance_event_lab_command;
mod research_builders_command;
mod source_text_foundation_command;
mod structural_paragraph_command;
mod synthetic_foundation_lab_command;
mod technical_markup_command;
mod transfer_candidates_command;
mod transfer_metadata_command;
mod transfer_source_passages_command;
mod transfer_source_visible_command;
mod transfer_target_passages_command;
mod visual_result_command;

use std::io::Write;
fn main() {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "foundation")
    {
        // Keep the protected foundation grammar and bounded argument intake
        // with its existing command owner, including the complete audit scope.
        let args = std::env::args_os().skip(2).take(65).collect::<Vec<_>>();
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let git_signal = std::sync::atomic::AtomicI32::new(0);
        let result = tos_command::source_current_cut::foundation_command::run(
            &args,
            &cancelled,
            &git_signal,
            &mut std::io::stdout().lock(),
            &mut std::io::stderr().lock(),
        );
        std::process::exit(result.unwrap_or(2));
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "--help" | "-h" | "help") {
        println!(
            "usage: tos-research build|check|validate --curation PATH [--stage STAGE] [OPTIONS]\n       tos-research COMMAND [OPTIONS]\nRun COMMAND --help for its explicit source, budget and output contract.\nResearch builders are separate from the read-only access service.\nCommands:\n  foundation\n  antonovsky-collation\n  authored-canon-bridge\n  bounded-translation-input\n  dta-technical-markup\n  german-triangulation\n  jenseits-label-correspondence\n  jenseits-numbered-structure\n  jenseits-polilov-numbered-structure\n  mysl-transfer-target-structure\n  nietzsche-transfer-source-structure\n  witness-structure-correspondence\n  lexical-index\n  nietzsche-transfer-source-routes\n  opening-sentence-alignment\n  provenance-event-v2-lab\n  semantic-source-recurrence\n  source-text-foundation\n  structural-paragraph\n  synthetic-foundation-lab\n  target-text-foundation\n  technical-markup\n  transfer-candidates\n  transfer-route-readiness\n  transfer-source-passages\n  transfer-source-visible\n  transfer-target-passages\n  zarathustra-morphology-census-result\n  zarathustra-morphology-context\n  zarathustra-morphology-context-result\n  zarathustra-morphology-input\n  zarathustra-recurrence-projection\n  zarathustra-usage-context\n  zarathustra-visual-retrieval-result\n  zarathustra-reading-workbench-v1\n  zarathustra-parallel-lexical-candidates-v1\n  zarathustra-de-ru-paragraph-alignment-v1\n  zarathustra-concept-workbench-v1\n  zarathustra-eternal-return-review-preparation-v1\n  zarathustra-morphology-theme-candidates-v1\n  zarathustra-eternal-return-concept-candidate-v1"
        );
        return;
    }
    if let Some(code) =
        curation_command::run_if_requested(&args, &mut std::io::stdout(), &mut std::io::stderr())
    {
        std::process::exit(code);
    }
    type Handler = fn(&[String], &mut dyn Write, &mut dyn Write) -> Option<i32>;
    let handlers: &[Handler] = &[
        crate::dta_technical_markup_command::run_if_requested,
        crate::transfer_source_passages_command::run_if_requested,
        crate::transfer_candidates_command::run_if_requested,
        crate::nietzsche_transfer_routes_command::run_if_requested,
        crate::jenseits_structure_command::run_if_requested,
        crate::visual_result_command::run_if_requested,
        crate::morphology_result_command::run_if_requested,
        crate::lexical_derivatives_command::run_if_requested,
        crate::jenseits_label_command::run_if_requested,
        crate::provenance_event_lab_command::run_if_requested,
        crate::synthetic_foundation_lab_command::run_if_requested,
        crate::authored_canon_bridge_command::run_if_requested,
        crate::antonovsky_collation_command::run_if_requested,
        crate::bounded_translation_input_command::run_if_requested,
        crate::german_triangulation_command::run_if_requested,
        crate::transfer_source_visible_command::run_if_requested,
        crate::transfer_target_passages_command::run_if_requested,
        crate::transfer_metadata_command::run_if_requested,
        crate::source_text_foundation_command::run_if_requested,
        crate::research_builders_command::run_if_requested,
        crate::lexical_index_command::run_if_requested,
        crate::technical_markup_command::run_if_requested,
        crate::structural_paragraph_command::run_if_requested,
    ];
    for run in handlers {
        if let Some(code) = run(&args, &mut std::io::stdout(), &mut std::io::stderr()) {
            std::process::exit(code);
        }
    }
    eprintln!("unknown research command: {}", args[0]);
    std::process::exit(2);
}
