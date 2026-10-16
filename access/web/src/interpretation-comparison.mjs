// Opaque page values and native map/slice callbacks stay with the presentation host.
let Session;
export function installInterpretationComparisonRules(runtime) {
  if (typeof runtime.InterpretationComparisonSession !== 'function') throw new Error('Interpretation comparison Rust runtime is required');
  Session = runtime.InterpretationComparisonSession;
}
export function buildInterpretationComparison(packet, selection, language, readingSummary) {
  if (!Session) throw new Error('Interpretation comparison Rust runtime is required');
  const session = new Session();
  let collection, challenges, context, posture, gaps, count, competing, contextual, selectedGaps, authority;
  try {
    while (session.phase() !== 17) {
      switch (session.phase()) {
        case 0: collection = packet.challenge_relations; session.observe(Boolean(collection)); break;
        case 1: challenges = (session.collection_fallback() ? [] : collection).map(readingSummary); session.completed(); break;
        case 2: collection = packet.context_relations; session.observe(Boolean(collection)); break;
        case 3: context = (session.collection_fallback() ? [] : collection).map(readingSummary); session.completed(); break;
        case 4: session.observe(Boolean(challenges.length)); break;
        case 5: posture = packet.posture; session.observe(Boolean(posture)); break;
        case 6: posture = packet.selection_posture?.review_posture; session.observe(Boolean(posture)); break;
        case 7: session.observe(language() === 'ru'); break;
        case 8: gaps = packet.gaps_ru; session.observe(Boolean(gaps)); break;
        case 9: gaps = packet.gaps; session.completed(); break;
        case 10: session.observe(packet.conclusion?.can_conclude === true); break;
        case 11: count = challenges.length; session.completed(); break;
        case 12: competing = challenges.slice(0, session.reading_limit()); session.completed(); break;
        case 13: contextual = context.slice(0, session.reading_limit()); session.completed(); break;
        case 14: session.observe(Boolean(gaps)); break;
        case 15: selectedGaps = (session.gaps_fallback() ? [] : gaps).slice(0, session.gap_limit()); session.completed(); break;
        case 16: authority = packet.authority_note; session.observe(Boolean(authority)); break;
      }
    }
    return {
      schema: session.schema(), selection,
      posture: session.posture_kind() ? session.fallback_posture() : posture,
      can_conclude: session.can_conclude(), competing_reading_count: count,
      competing_readings: competing, contextual_readings: contextual, gaps: selectedGaps,
      authority_note: session.authority_fallback() ? session.fallback_authority() : authority,
    };
  } finally { session.free(); }
}
