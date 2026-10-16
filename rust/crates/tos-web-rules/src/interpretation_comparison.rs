//! Ordered page comparison projection. Host retains opaque readings and native callbacks.
use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub struct InterpretationComparisonSession {
    phase: u8,
    posture: u8,
    collection_fallback: bool,
    gaps_fallback: bool,
    authority_fallback: bool,
    can_conclude: bool,
}
#[wasm_bindgen]
impl InterpretationComparisonSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            phase: 0,
            posture: 0,
            collection_fallback: false,
            gaps_fallback: false,
            authority_fallback: false,
            can_conclude: false,
        }
    }
    pub fn phase(&self) -> u8 {
        self.phase
    }
    pub fn completed(&mut self) {
        self.phase += 1;
    }
    pub fn observe(&mut self, value: bool) {
        self.phase = match self.phase {
            0 | 2 => {
                self.collection_fallback = !value;
                self.phase + 1
            }
            4 if value => {
                self.posture = 1;
                7
            }
            4 => 5,
            5 if value => 7,
            5 => 6,
            6 => {
                if !value {
                    self.posture = 2;
                }
                7
            }
            7 if value => 8,
            7 => 9,
            8 if value => 10,
            8 => 9,
            10 => {
                self.can_conclude = value;
                11
            }
            14 => {
                self.gaps_fallback = !value;
                15
            }
            16 => {
                self.authority_fallback = !value;
                17
            }
            _ => self.phase,
        };
    }
    pub fn collection_fallback(&self) -> bool {
        self.collection_fallback
    }

    pub fn posture_kind(&self) -> u8 {
        self.posture
    }
    pub fn gaps_fallback(&self) -> bool {
        self.gaps_fallback
    }
    pub fn authority_fallback(&self) -> bool {
        self.authority_fallback
    }
    pub fn can_conclude(&self) -> bool {
        self.can_conclude
    }
    pub fn reading_limit(&self) -> u32 {
        8
    }
    pub fn gap_limit(&self) -> u32 {
        12
    }
    pub fn schema(&self) -> String {
        "tos_interpretation_comparison_v1".into()
    }
    pub fn fallback_posture(&self) -> String {
        if self.posture == 1 {
            "contested_review_required"
        } else {
            "review_status_unresolved"
        }
        .into()
    }
    pub fn fallback_authority(&self) -> String {
        "Projected challenge relations are review leads, not adjudicated counterevidence or canon decisions.".into()
    }
}
