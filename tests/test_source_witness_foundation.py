from __future__ import annotations

import copy
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

from jsonschema import FormatChecker
from jsonschema.validators import validator_for


REPO_ROOT = Path(__file__).resolve().parents[1]
DISCOVERY_PROVENANCE_PATH = (
    REPO_ROOT / "ToS/source-witnesses/discovery/provenance.jsonl"
)
DISCOVERY_RUNS_README_PATH = (
    REPO_ROOT / "ToS/source-witnesses/discovery/runs/README.md"
)
GOLD_ROOT = REPO_ROOT / "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1"
PRIVATE_HANDOFF_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "private-evidence-handoff.v1.json"
)
MANUAL_ERROR_LEDGER_PATH = GOLD_ROOT / "manual-error-ledger.jsonl"
MANUAL_ERROR_LEDGER_PROVENANCE_PATH = (
    GOLD_ROOT
    / "provenance.manual-error-ledger."
    "ocr-candidate-review-foundation-v1.jsonl"
)
TRANSFER_CANDIDATE_CROSSWALK_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "jenseits-von-gut-und-boese/alignments/structure/"
    "naumann-1886-polilov-mysl-1996/"
    "transfer-candidate-page-crosswalk.v1.json"
)
HIERARCHICAL_TARGET_STRUCTURE_ROOTS = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "zur-genealogie-der-moral/expressions/ru-svasyan-mysl-1996/"
    "structure/mysl-1996-volume-2-operator-pdf",
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "der-antichrist/expressions/ru-flerova-mysl-1996/"
    "structure/mysl-1996-volume-2-operator-pdf",
)
GERMAN_ASSISTED_REVIEW_PATH = (
    GOLD_ROOT / "german-assisted-source-review.v1.json"
)
CRITICAL_EDITION_WITNESS_PATH = (
    GOLD_ROOT / "critical-edition-witness.ekgwb.za-i-vorrede-1.v1.json"
)
CRITICAL_EDITION_CITATION_DECISION_PATH = (
    GOLD_ROOT
    / "critical-edition-citation-witness-decision."
    "ekgwb.za-i-vorrede-1.v1.json"
)
EDITION_READING_ADMISSION_PATH = (
    GOLD_ROOT
    / "edition-reading-admission."
    "dta-ekgwb.za-i-vorrede-1.v1.json"
)
GERMAN_SOURCE_TRIANGULATION_PATH = (
    GOLD_ROOT
    / "german-source-triangulation."
    "ekgwb-dta-naumann.za-i-vorrede-1.v1.json"
)
BOUNDED_TRANSLATION_RESEARCH_INPUT_PATH = (
    GOLD_ROOT
    / "bounded-translation-research-input."
    "za-i-vorrede-1-opening-sentence.v1.json"
)
EXPERIMENTAL_TRANSLATION_CANDIDATE_PATH = (
    GOLD_ROOT
    / "experimental-translation-candidate."
    "admitted-ekgwb.za-i-vorrede-1-opening.variant-a.v1.json"
)
EXPERIMENTAL_TRANSLATION_FAILURE_EPISODE_PATH = (
    GOLD_ROOT
    / "experimental-translation-episode.e4b-direct.infrastructure-failure.v1.json"
)
EXPERIMENTAL_TRANSLATION_REJECTION_EPISODE_PATH = (
    GOLD_ROOT
    / "experimental-translation-episode.e4b-direct.russian-surface-rejection.v1.json"
)
EXPERIMENTAL_TRANSLATION_UNCERTAIN_EPISODE_PATH = (
    GOLD_ROOT
    / "experimental-translation-episode."
    "madlad400-3b-candle-cpu.uncertain-retention.v1.json"
)
EKGWB_RIGHTS_PATH = (
    GOLD_ROOT / "rights.ekgwb.za-i-vorrede-1.v1.json"
)
EKGWB_INSTITUTIONAL_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "ekgwb-za-i-vorrede-1-institutional-corroboration.2026-07-30.v3.json"
)
MYSL_WORK_BOUNDARY_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/collections/friedrich-nietzsche/"
    "works-in-two-volumes-volume-2-mysl-1996/structure/work-boundaries"
)
JENSEITS_1886_EXPRESSION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "jenseits-von-gut-und-boese/expressions/de-naumann-1886"
)
JENSEITS_1886_EDITION_ROOT = (
    JENSEITS_1886_EXPRESSION_ROOT / "editions/leipzig-c-g-naumann-1886"
)
JENSEITS_1886_ITEM_ROOT = (
    JENSEITS_1886_EDITION_ROOT
    / "items/internet-archive-google-harvard-scan-pdf"
)
JENSEITS_1886_PROVISION_CLAIMS_PATH = (
    JENSEITS_1886_EDITION_ROOT / "provision-activity-claims.jsonl"
)
JENSEITS_1886_PROVISION_ANCHORS_PATH = (
    JENSEITS_1886_EXPRESSION_ROOT / "structure/provision-statements/anchors.jsonl"
)
JENSEITS_1886_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "jenseits-naumann-1886-open-scan-witness.2026-07-28.v1.json"
)
JENSEITS_1886_PROVISION_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "jenseits-1886-provision-identity.2026-08-01.v1.json"
)
JENSEITS_1886_PROVISION_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "JENSEITS_1886_PROVISION_IDENTITY_RESEARCH.md"
)
JENSEITS_1886_LAYERED_RIGHTS_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "JENSEITS_1886_IA_GOOGLE_HARVARD_LAYERED_RIGHTS_ASSESSMENT.md"
)
JENSEITS_1886_SERVER_PLAN_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/server-import/plans/"
    "jenseits-naumann-1886-internet-archive-google-harvard-"
    "scan-pdf.server-import.json"
)
JENSEITS_AUTHORIAL_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "jenseits-authorial-witness-route.2026-07-30.v1.json"
)
GENEALOGIE_1892_ITEM_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "zur-genealogie-der-moral/expressions/de-naumann-1892-second/"
    "editions/leipzig-c-g-naumann-1892-second-edition/items/"
    "wikimedia-commons-unc-scan-pdf"
)
GENEALOGIE_1892_EXPRESSION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "zur-genealogie-der-moral/expressions/de-naumann-1892-second"
)
GENEALOGIE_1892_EDITION_ROOT = (
    GENEALOGIE_1892_EXPRESSION_ROOT
    / "editions/leipzig-c-g-naumann-1892-second-edition"
)
GENEALOGIE_1892_PROVISION_CLAIMS_PATH = (
    GENEALOGIE_1892_EDITION_ROOT / "provision-activity-claims.jsonl"
)
GENEALOGIE_1892_PROVISION_ANCHORS_PATH = (
    GENEALOGIE_1892_EXPRESSION_ROOT
    / "structure/provision-statements/anchors.jsonl"
)
GENEALOGIE_1892_PROVISION_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "genealogie-1892-provision-identity.2026-08-01.v1.json"
)
GENEALOGIE_1892_PROVISION_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "GENEALOGIE_1892_PROVISION_IDENTITY_RESEARCH.md"
)
GENEALOGIE_1892_LAYERED_RIGHTS_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "GENEALOGIE_1892_COMMONS_UNC_LAYERED_RIGHTS_ASSESSMENT.md"
)
GENEALOGIE_1892_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "genealogie-naumann-1892-open-scan-witness.2026-07-30.v1.json"
)
GENEALOGIE_AUTHORIAL_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "genealogie-authorial-witness-route.2026-07-30.v1.json"
)
ANTICHRIST_1906_COLLECTION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/collections/friedrich-nietzsche/"
    "nietzsches-werke-erste-abtheilung-band-viii-naumann-1906"
)
ANTICHRIST_1906_ITEM_ROOT = (
    ANTICHRIST_1906_COLLECTION_ROOT
    / "editions/leipzig-c-g-naumann-1906/items/"
    "wikimedia-commons-stanford-scan-djvu"
)
ANTICHRIST_1906_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "der-antichrist-naumann-1906-open-scan-witness.2026-07-30.v1.json"
)
ANTICHRIST_1906_LAYERED_RIGHTS_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "ANTICHRIST_1906_COMMONS_STANFORD_LAYERED_RIGHTS_ASSESSMENT.md"
)
ANTICHRIST_AUTHORIAL_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "antichrist-authorial-witness-route.2026-07-30.v1.json"
)
FALL_WAGNER_1888_EDITION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "der-fall-wagner/expressions/de-naumann-1888/"
    "editions/leipzig-c-g-naumann-1888"
)
FALL_WAGNER_1888_ITEM_ROOT = (
    FALL_WAGNER_1888_EDITION_ROOT / "items/mdz-bamberg-scan-pdf"
)
FALL_WAGNER_1888_PUBLICATION_CLAIMS_PATH = (
    FALL_WAGNER_1888_EDITION_ROOT / "publication-claims.jsonl"
)
FALL_WAGNER_1888_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "der-fall-wagner-naumann-1888-open-scan-witness.2026-07-30.v1.json"
)
FALL_WAGNER_AUTHORIAL_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "fall-wagner-authorial-witness-route.2026-07-30.v1.json"
)
GOETZEN_1889_EDITION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "goetzen-daemmerung/expressions/de-naumann-1889/"
    "editions/leipzig-c-g-naumann-1889"
)
GOETZEN_1889_ITEM_ROOT = GOETZEN_1889_EDITION_ROOT / "items/mdz-bsb-scan-pdf"
GOETZEN_1889_PUBLICATION_CLAIMS_PATH = (
    GOETZEN_1889_EDITION_ROOT / "publication-claims.jsonl"
)
GOETZEN_1889_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "goetzen-daemmerung-naumann-1889-open-scan-witness.2026-07-30.v1.json"
)
GOETZEN_AUTHORIAL_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "goetzen-daemmerung-authorial-witness-route.2026-07-30.v1.json"
)
MDZ_PAIR_LAYERED_RIGHTS_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "MDZ_FALL_WAGNER_1888_GOETZEN_1889_LAYERED_RIGHTS_ASSESSMENT.md"
)
ECCE_HOMO_WORK_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "ecce-homo"
)
ECCE_HOMO_1908_EDITION_ROOT = (
    ECCE_HOMO_WORK_ROOT
    / "expressions/de-richter-insel-1908/"
    "editions/leipzig-insel-verlag-1908"
)
ECCE_HOMO_1908_ITEM_ROOT = (
    ECCE_HOMO_1908_EDITION_ROOT
    / "items/wikimedia-commons-getty-scan-pdf"
)
ECCE_HOMO_RESPONSIBILITY_CLAIMS_PATH = (
    ECCE_HOMO_WORK_ROOT / "responsibility-claims.jsonl"
)
ECCE_HOMO_1908_LAYERED_RIGHTS_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "ECCE_HOMO_1908_COMMONS_GETTY_LAYERED_RIGHTS_ASSESSMENT.md"
)
WORK_CHRONOLOGY_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/chronology/friedrich-nietzsche/first-publication"
)
WORK_CHRONOLOGY_CLAIMS_PATH = WORK_CHRONOLOGY_ROOT / "work-chronology-claims.jsonl"
WORK_CHRONOLOGY_PROVENANCE_PATH = WORK_CHRONOLOGY_ROOT / "provenance.jsonl"
WORK_CHRONOLOGY_SCHEMA_PATH = (
    REPO_ROOT / "ToS/contracts/first-publication-chronology.schema.json"
)
PROVISION_ACTIVITY_SCHEMA_PATH = (
    REPO_ROOT / "ToS/contracts/provision-activity.schema.json"
)
GOETZEN_1889_PROVISION_CLAIMS_PATH = (
    GOETZEN_1889_EDITION_ROOT / "provision-activity-claims.jsonl"
)
ECCE_HOMO_1908_PROVISION_CLAIMS_PATH = (
    ECCE_HOMO_1908_EDITION_ROOT / "provision-activity-claims.jsonl"
)
ZARATHUSTRA_PART_1_EDITION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-1/"
    "editions/chemnitz-schmeitzner-1883-part-1"
)
ZARATHUSTRA_PART_1_PROVISION_CLAIMS_PATH = (
    ZARATHUSTRA_PART_1_EDITION_ROOT / "provision-activity-claims.jsonl"
)
ZARATHUSTRA_PART_2_EDITION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-2/"
    "editions/chemnitz-schmeitzner-1883-part-2"
)
ZARATHUSTRA_PART_2_PROVISION_CLAIMS_PATH = (
    ZARATHUSTRA_PART_2_EDITION_ROOT / "provision-activity-claims.jsonl"
)
ZARATHUSTRA_PART_3_EDITION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/expressions/de-schmeitzner-1884-part-3/"
    "editions/chemnitz-schmeitzner-1884-part-3"
)
ZARATHUSTRA_PART_3_PROVISION_CLAIMS_PATH = (
    ZARATHUSTRA_PART_3_EDITION_ROOT / "provision-activity-claims.jsonl"
)
ZARATHUSTRA_PART_4_EDITION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/expressions/de-naumann-1891-part-4/"
    "editions/leipzig-naumann-1891-part-4"
)
ZARATHUSTRA_PART_4_PROVISION_CLAIMS_PATH = (
    ZARATHUSTRA_PART_4_EDITION_ROOT / "provision-activity-claims.jsonl"
)
ZARATHUSTRA_PART_1_PROVISION_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "zarathustra-part-1-provision-identity.2026-08-01.v1.json"
)
ZARATHUSTRA_PARTS_2_3_PROVISION_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "zarathustra-parts-2-3-provision-identity.2026-08-01.v1.json"
)
ZARATHUSTRA_PARTS_2_3_PROVISION_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "ZARATHUSTRA_PARTS_2_3_PROVISION_IDENTITY_RESEARCH.md"
)
ZARATHUSTRA_PART_4_PROVISION_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "zarathustra-part-4-provision-identity.2026-08-01.v1.json"
)
ZARATHUSTRA_PART_4_PROVISION_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "ZARATHUSTRA_PART4_PROVISION_IDENTITY_RESEARCH.md"
)
NAUMANN_1893_EXPRESSION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/expressions/de-naumann-1893"
)
NAUMANN_1893_EDITION_ROOT = (
    NAUMANN_1893_EXPRESSION_ROOT / "editions/leipzig-c-g-naumann-1893"
)
NAUMANN_1893_PROVISION_CLAIMS_PATH = (
    NAUMANN_1893_EDITION_ROOT / "provision-activity-claims.jsonl"
)
NAUMANN_1893_PROVISION_ANCHORS_PATH = (
    NAUMANN_1893_EXPRESSION_ROOT / "structure/provision-statements/anchors.jsonl"
)
NAUMANN_1893_PROVISION_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "naumann-1893-provision-identity.2026-08-01.v1.json"
)
NAUMANN_1893_PROVISION_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "NAUMANN_1893_PROVISION_IDENTITY_RESEARCH.md"
)
NAUMANN_1893_RIGHTS_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "NAUMANN_1893_IA_PDF_EPUB_LAYERED_RIGHTS_ASSESSMENT.md"
)
NAUMANN_1893_PUBLISHER_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/organizations/"
    "c-g-naumann-verlag-leipzig/organization.json"
)
NAUMANN_1893_PRINTER_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/organizations/"
    "druckerei-c-g-naumann-leipzig/organization.json"
)
MYSL_TRANSLATOR_IDENTITY_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "mysl-translator-identity-reconciliation.2026-08-01.v1.json"
)
MYSL_TRANSLATOR_IDENTITY_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "MYSL_TRANSLATOR_IDENTITY_RECONCILIATION_RESEARCH.md"
)
MYSL_RESPONSIBILITY_CLAIMS_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/collections/friedrich-nietzsche/"
    "works-in-two-volumes-volume-2-mysl-1996/responsibility-claims.jsonl"
)
AGENT_ROOT = REPO_ROOT / "ToS/source-witnesses/agents"
ANTONOVSKY_1900_EXPRESSION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/expressions/ru-antonovsky-1900"
)
ANTONOVSKY_1900_EDITION_ROOT = (
    ANTONOVSKY_1900_EXPRESSION_ROOT
    / "editions/saint-petersburg-unknown-publisher-1900"
)
ANTONOVSKY_1900_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "antonovsky-1900-rsl-rnl-lnb-current-holdings.2026-08-10.v2.json"
)
ANTONOVSKY_1900_REQUEST_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/access-requests/public-ledger/"
    "antonovsky-1900-lnb-research-copy.access-request.json"
)
ANTONOVSKY_1903_EXPRESSION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/expressions/ru-antonovsky-1903"
)
ANTONOVSKY_1903_EDITION_ROOT = (
    ANTONOVSKY_1903_EXPRESSION_ROOT
    / "editions/saint-petersburg-altshuler-typography-1903-second-corrected"
)
ANTONOVSKY_1903_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "antonovsky-1903-rsl-rnl-current-holdings.2026-08-10.v2.json"
)
ANTONOVSKY_1903_REQUEST_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/access-requests/public-ledger/"
    "antonovsky-1903-rsl-research-copy.access-request.json"
)
ANTONOVSKY_1903_RNL_REQUEST_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/access-requests/public-ledger/"
    "antonovsky-1903-rnl-research-copy.access-request.json"
)
ANTONOVSKY_1907_EXPRESSION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/expressions/ru-antonovsky-1907"
)
ANTONOVSKY_1907_EDITION_ROOT = (
    ANTONOVSKY_1907_EXPRESSION_ROOT
    / "editions/saint-petersburg-vaisberg-gershunin-typography-1907-third"
)
ANTONOVSKY_1907_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "antonovsky-1907-runeb-edition.2026-08-01.v1.json"
)
ANTONOVSKY_1907_REQUEST_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/access-requests/public-ledger/"
    "antonovsky-1907-rsl-neb-replacement.access-request.json"
)
ANTONOVSKY_1907_RNL_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "antonovsky-1907-rnl-primo-current-holding.2026-08-10.v3.json"
)
ANTONOVSKY_1907_RNL_REPORTED_RSL_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "antonovsky-1907-rnl-reported-rsl-holding.2026-08-10.v4.json"
)
ANTONOVSKY_1907_RNL_REQUEST_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/access-requests/public-ledger/"
    "antonovsky-1907-rnl-gak-holding.access-request.json"
)
ANTONOVSKY_1911_EXPRESSION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/expressions/ru-antonovsky-1911"
)
ANTONOVSKY_1911_EDITION_ROOT = (
    ANTONOVSKY_1911_EXPRESSION_ROOT
    / "editions/saint-petersburg-prometey-1911-fourth"
)
ANTONOVSKY_1911_ITEM_ROOT = (
    ANTONOVSKY_1911_EDITION_ROOT / "items/rsl-neb-scan-pdf"
)
ANTONOVSKY_1911_RESPONSIBILITY_CLAIMS_PATH = (
    ANTONOVSKY_1911_EXPRESSION_ROOT / "responsibility-claims.jsonl"
)
ANTONOVSKY_1911_PROVISION_CLAIMS_PATH = (
    ANTONOVSKY_1911_EDITION_ROOT / "provision-activity-claims.jsonl"
)
ANTONOVSKY_1911_SOURCE_ANCHORS_PATH = (
    ANTONOVSKY_1911_EXPRESSION_ROOT
    / "structure/source-statements/anchors.jsonl"
)
ANTONOVSKY_1911_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "antonovsky-prometey-1911-source-witness.2026-08-01.v1.json"
)
ANTONOVSKY_1911_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "ANTONOVSKY_PROMETEY_1911_SOURCE_WITNESS_RESEARCH.md"
)
ANTONOVSKY_1911_RIGHTS_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "ANTONOVSKY_1911_RSL_RUNEB_LAYERED_RIGHTS_ASSESSMENT.md"
)
ANTONOVSKY_1911_SERVER_PLAN_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/server-import/plans/"
    "antonovsky-prometey-1911-rsl-neb-scan-pdf.server-import.json"
)
READER_1899_EXPRESSION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/expressions/ru-reader-1899-uncredited"
)
READER_1899_EDITION_ROOT = (
    READER_1899_EXPRESSION_ROOT
    / "editions/moscow-reader-editorial-office-1899"
)
READER_1899_ITEM_ROOT = (
    READER_1899_EDITION_ROOT / "items/rnl-rusneb-fragment-pdf-parts"
)
READER_1899_PROVISION_CLAIMS_PATH = (
    READER_1899_EDITION_ROOT / "provision-activity-claims.jsonl"
)
READER_1899_SOURCE_ANCHORS_PATH = (
    READER_1899_EXPRESSION_ROOT / "structure/source-statements/anchors.jsonl"
)
READER_1899_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "reader-1899-rnl-rusneb-fragment-source-witness.2026-08-01.v1.json"
)
READER_1899_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "READER_1899_FRAGMENT_SOURCE_WITNESS_RESEARCH.md"
)
READER_1899_RIGHTS_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "READER_1899_LAYERED_RIGHTS_ASSESSMENT.md"
)
READER_1899_REQUEST_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/access-requests/public-ledger/"
    "reader-1899-rnl-runeb-complete-copy.access-request.json"
)
READER_1899_SERVER_PLAN_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/server-import/plans/"
    "reader-1899-rnl-rusneb-fragment-pdf-parts.server-import.json"
)
NANI_1899_EXPRESSION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/expressions/ru-nani-1899-nine-fragments"
)
NANI_1899_EDITION_ROOT = (
    NANI_1899_EXPRESSION_ROOT
    / "editions/saint-petersburg-stasyulevich-1899-nine-fragments"
)
NANI_1899_ITEM_ROOT = (
    NANI_1899_EDITION_ROOT / "items/rsl-rusneb-parallel-scan-pdf"
)
NANI_1899_RESPONSIBILITY_CLAIMS_PATH = (
    NANI_1899_EXPRESSION_ROOT / "responsibility-claims.jsonl"
)
NANI_1899_PROVISION_CLAIMS_PATH = (
    NANI_1899_EDITION_ROOT / "provision-activity-claims.jsonl"
)
NANI_1899_SOURCE_ANCHORS_PATH = (
    NANI_1899_EXPRESSION_ROOT / "structure/source-statements/anchors.jsonl"
)
NANI_1899_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "nani-1899-parallel-fragment-source-witness.2026-08-01.v1.json"
)
NANI_1899_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "NANI_1899_PARALLEL_FRAGMENT_SOURCE_WITNESS_RESEARCH.md"
)
NANI_1899_RIGHTS_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "NANI_1899_LAYERED_RIGHTS_ASSESSMENT.md"
)
NANI_1899_SERVER_PLAN_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/server-import/plans/"
    "nani-1899-rsl-rusneb-parallel-scan-pdf.server-import.json"
)
NANI_AGENT_PATH = (
    REPO_ROOT / "ToS/source-witnesses/agents/s-p-nani/agent.json"
)
STASYULEVICH_PRINTING_ORGANIZATION_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/organizations/"
    "m-m-stasyulevich-printing-saint-petersburg/organization.json"
)
ANTONOVSKY_IDENTITY_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "antonovsky-agent-identity-reconciliation.2026-08-01.v1.json"
)
ANTONOVSKY_IDENTITY_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "ANTONOVSKY_AGENT_IDENTITY_RECONCILIATION_RESEARCH.md"
)
ANTONOVSKY_1913_RESPONSIBILITY_CLAIMS_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/expressions/ru-antonovsky-1913/"
    "responsibility-claims.jsonl"
)
CHEMNITZ_PLACE_PATH = (
    REPO_ROOT / "ToS/source-witnesses/places/chemnitz/place.json"
)
SCHMEITZNER_ORGANIZATION_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/organizations/"
    "ernst-schmeitzner-verlagsbuchhandlung-chemnitz/organization.json"
)
ANTONOVSKY_1913_EXPRESSION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/expressions/ru-antonovsky-1913"
)
ANTONOVSKY_1913_RESPONSIBILITY_CLAIMS_PATH = (
    ANTONOVSKY_1913_EXPRESSION_ROOT / "responsibility-claims.jsonl"
)
ANTONOVSKY_1913_TITLE_ANCHORS_PATH = (
    ANTONOVSKY_1913_EXPRESSION_ROOT / "structure/title-page/anchors.jsonl"
)
ANTONOVSKY_1913_TRANSLATION_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "ANTONOVSKY_1913_TRANSLATION_RESPONSIBILITY_RESEARCH.md"
)
ANTONOVSKY_2007_EXPRESSION_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/expressions/"
    "ru-antonovsky-cultural-revolution"
)
ANTONOVSKY_2007_RESPONSIBILITY_CLAIMS_PATH = (
    ANTONOVSKY_2007_EXPRESSION_ROOT / "responsibility-claims.jsonl"
)
ANTONOVSKY_2007_RESPONSIBILITY_ANCHORS_PATH = (
    ANTONOVSKY_2007_EXPRESSION_ROOT
    / "structure/edition-responsibility/anchors.jsonl"
)
ANTONOVSKY_2007_TRANSLATION_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "antonovsky-cultural-revolution-2007-translation-responsibility."
    "2026-08-01.v1.json"
)
ANTONOVSKY_2007_TRANSLATION_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "ANTONOVSKY_CULTURAL_REVOLUTION_2007_TRANSLATION_RESPONSIBILITY_RESEARCH.md"
)
ANTONOVSKY_1913_EDITION_ROOT = (
    ANTONOVSKY_1913_EXPRESSION_ROOT
    / "editions/saint-petersburg-zhizn-dlya-vsekh-1913"
)
ANTONOVSKY_1913_ITEM_ROOT = (
    ANTONOVSKY_1913_EDITION_ROOT
    / "items/wikimedia-commons-penza-scan-pdf"
)
ANTONOVSKY_1913_RIGHTS_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "ANTONOVSKY_1913_LAYERED_RIGHTS_ASSESSMENT.md"
)
ANTONOVSKY_1913_SERVER_PLAN_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/server-import/plans/"
    "antonovsky-1913-wikimedia-commons-penza-scan-pdf.server-import.json"
)
ANTONOVSKY_1913_PROVISION_CLAIMS_PATH = (
    ANTONOVSKY_1913_EDITION_ROOT / "provision-activity-claims.jsonl"
)
ANTONOVSKY_1913_PROVISION_ANCHORS_PATH = (
    ANTONOVSKY_1913_EXPRESSION_ROOT
    / "structure/provision-statements/anchors.jsonl"
)
ANTONOVSKY_1913_PROVISION_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "antonovsky-1913-provision-identity.2026-08-01.v1.json"
)
ANTONOVSKY_1913_PROVISION_RESEARCH_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "ANTONOVSKY_1913_PROVISION_IDENTITY_RESEARCH.md"
)
SAINT_PETERSBURG_PLACE_PATH = (
    REPO_ROOT / "ToS/source-witnesses/places/saint-petersburg/place.json"
)
ZHIZN_DLYA_VSEKH_ORGANIZATION_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/organizations/"
    "zhizn-dlya-vsekh-saint-petersburg/organization.json"
)
LINNIK_PRINTING_ORGANIZATION_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/organizations/"
    "bratya-v-i-i-linnik-printing-saint-petersburg/organization.json"
)
ECCE_HOMO_1908_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "ecce-homo-insel-1908-open-scan-witness.2026-07-30.v1.json"
)
SCRIPTS_DIR = REPO_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import validate_source_witness_foundation as foundation
import build_source_witness_catalog as catalog_builder
import build_zarathustra_german_source_triangulation as triangulation_builder
import build_zarathustra_bounded_translation_input as bounded_input_builder


PRE_DRAFT_STAGE_NAMES = [
    "morphology",
    "syntax",
    "polysemy_unusual_forms_and_repetition",
    "historical_german_senses",
    "sourced_etymology",
    "intra_zarathustra_parallels",
    "nietzsche_corpus_parallels",
    "evidenced_allusions_idioms_and_cultural_links",
    "literal_interlinear",
]

TRANSLATION_EVALUATION_AXES = [
    "primary_meaning",
    "secondary_meanings",
    "etymological_connection",
    "polysemy",
    "terminological_consistency",
    "recurring_signs",
    "metaphor",
    "image",
    "syntax",
    "rhythm",
    "spoken_pronounceability",
    "intentional_strangeness",
    "cultural_allusion",
    "omissions",
    "additions",
    "stylistic_smoothing",
    "unjustified_embellishment",
    "recognized_translation_influence",
    "decision_explainability",
]

SEMANTIC_STAGE_ORDER = [
    "exact_form",
    "frequency_and_concordance",
    "context",
    "morphology",
    "lemma",
    "recurrence_within_section",
    "recurrence_within_work",
    "recurrence_within_author_corpus",
    "translation_correspondences",
    "stable_sign_candidate",
    "manual_confirmation_or_rejection",
    "relations_between_signs",
    "conceptual_interpretations",
    "competing_readings",
    "graph_projection",
]


def _synthetic_pre_draft_packet(*, lane: str = "human_only") -> dict:
    maker_type = "human" if lane == "human_only" else "model"
    findings = []
    for index, stage_name in enumerate(PRE_DRAFT_STAGE_NAMES, start=1):
        reference_entry_ids = []
        citations = []
        if stage_name == "sourced_etymology":
            reference_entry_ids = ["tos-ref.synthetic-etymology"]
            citations = [
                {
                    "citation_id": "tos-citation.synthetic-etymology",
                    "reference_entry_id": "tos-ref.synthetic-etymology",
                    "locator": "synthetic dictionary entry",
                    "evidence_url": "https://example.invalid/dictionary/entry",
                    "accessed_at": "2026-07-23T00:00:00Z",
                    "rights_posture": "citation-only synthetic test fixture",
                    "content_stored": "citation-only",
                }
            ]
        findings.append(
            {
                "stage": stage_name,
                "status": "frozen",
                "findings": [
                    {
                        "finding_id": f"tos-finding.synthetic-{index}",
                        "statement": f"Synthetic finding for stage {index}.",
                        "source_anchor_refs": ["tos.anchor.synthetic-source"],
                        "reference_entry_ids": reference_entry_ids,
                        "citations": citations,
                        "epistemic_status": (
                            "sourced" if stage_name == "sourced_etymology" else "observed"
                        ),
                        "confidence": "not-applicable",
                        "maker": {
                            "maker_type": maker_type,
                            "agent_ref": f"synthetic-{maker_type}",
                        },
                        "provenance_event_ref": "tos.event.synthetic-analysis",
                        "review_status": (
                            "human-reviewed" if lane == "human_only" else "unreviewed"
                        ),
                    }
                ],
                "source_return_verified": True,
                "frozen_at": "2026-07-23T00:00:00Z",
                "review_status": (
                    "human-reviewed" if lane == "human_only" else "unreviewed"
                ),
            }
        )

    is_human = lane == "human_only"
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/translation-pre-draft-analysis.schema.json",
        "schema_version": "tos_translation_pre_draft_analysis_v1",
        "packet_id": f"tos.translation-pre-draft-analysis.synthetic-{lane.replace('_', '-')}",
        "experiment_id": "tos-translation-foundation-v1",
        "work_ref": "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
        "source_expression_ref": "tos.expression.synthetic-german",
        "source_anchor_refs": ["tos.anchor.synthetic-source"],
        "source_review_evidence": {
            "review_unit_id": "tos-translation-source-review-v2-001",
            "review_receipt_refs": ["synthetic-pass-1", "synthetic-pass-2"],
            "accepted_diplomatic_sha256": "a" * 64,
            "pass_1_performed_by_real_human": True,
            "pass_2_performed_by_real_human": True,
            "pass_timestamps_distinct": True,
            "source_acceptance": "accept",
        },
        "source_forms": {
            "diplomatic": "Synthetic source form.",
            "diplomatic_sha256": "a" * 64,
            "normalized": "Synthetic source form.",
            "normalization_posture": "none",
            "normalization_event_ref": "tos.event.synthetic-normalization",
        },
        "target_language": "ru",
        "lane": lane,
        "blindness": {
            "recognized_comparator_visible": False,
            "recognized_comparator_consulted": False,
            "other_lane_analysis_visible": False,
            "other_lane_drafts_visible": False,
            "translation_draft_created": False,
        },
        "maker": {
            "maker_type": maker_type,
            "agent_ref": f"synthetic-{maker_type}",
            "performed_by_real_human": is_human,
            "ai_assistance_used": not is_human,
            "human_editing_used": is_human,
            "model_refs": [] if is_human else ["synthetic-model"],
            "runtime_receipt_refs": [] if is_human else ["synthetic-runtime-receipt"],
        },
        "reference_register": {
            "ref": "ToS/source-witnesses/synthetic-reference-register.json",
            "sha256": "b" * 64,
            "status": "researched-not-content-admitted",
            "content_admission_is_per_citation": True,
        },
        "analysis_order": PRE_DRAFT_STAGE_NAMES,
        "stages": findings,
        "all_claims_return_to_source": True,
        "frequency_is_not_semantic_sufficiency": True,
        "model_memory_is_not_etymological_evidence": True,
        "rights_and_visibility": {
            "visibility": "local-only",
            "restricted_source_text_redistribution": False,
            "reference_rights_inherited_per_citation": True,
            "public_metadata_separate": True,
        },
        "packet_status": "frozen-ready-for-lane-draft",
        "provenance_event_refs": ["tos.event.synthetic-analysis"],
        "authority_boundary": "blind pre-draft linguistic and philological evidence only; this packet contains no translation draft, recognized comparator content, accepted etymology without citation, semantic sign decision, or graph claim",
        "packet_version": 1,
    }


def _synthetic_pre_draft_link(lane: str, suffix: str) -> dict:
    posture = {
        "human_only": "real-human-only",
        "ai_only": "model-only",
        "ai_alternative": "model-alternative-only",
    }[lane]
    return {
        "packet_id": f"tos.translation-pre-draft-analysis.synthetic-{suffix}",
        "ref": f"ToS/local-content/translation/synthetic-{suffix}.json",
        "sha256": "b" * 64,
        "lane": lane,
        "maker_posture": posture,
        "accepted_source_sha256": "a" * 64,
        "status": "frozen-ready-for-lane-draft",
        "frozen_at": "2026-07-23T01:00:00Z",
    }


def _synthetic_draft(
    draft_type: str,
    suffix: str,
    pre_draft_links: list[dict],
    *,
    input_drafts: list[dict] | None = None,
) -> dict:
    if draft_type == "human_only":
        maker = {
            "maker_type": "human",
            "agent_refs": ["human:synthetic-translator"],
            "performed_by_real_human": True,
            "ai_assistance_used": False,
            "human_editing_used": True,
            "model_refs": [],
            "prompt_receipt_refs": [],
            "runtime_receipt_refs": [],
            "human_intervention_refs": [],
        }
    elif draft_type == "ai_human":
        maker = {
            "maker_type": "human-ai-collaboration",
            "agent_refs": ["human:synthetic-translator", "model:synthetic-model"],
            "performed_by_real_human": True,
            "ai_assistance_used": True,
            "human_editing_used": True,
            "model_refs": ["synthetic-model"],
            "prompt_receipt_refs": ["synthetic-prompt-receipt"],
            "runtime_receipt_refs": ["synthetic-runtime-receipt"],
            "human_intervention_refs": ["synthetic-human-intervention"],
        }
    else:
        maker = {
            "maker_type": "model",
            "agent_refs": ["model:synthetic-model"],
            "performed_by_real_human": False,
            "ai_assistance_used": True,
            "human_editing_used": False,
            "model_refs": ["synthetic-model"],
            "prompt_receipt_refs": ["synthetic-prompt-receipt"],
            "runtime_receipt_refs": ["synthetic-runtime-receipt"],
            "human_intervention_refs": [],
        }
    return {
        "draft_id": f"tos-translation-draft.synthetic-{suffix}",
        "draft_type": draft_type,
        "text": f"Synthetic translation draft {suffix}.",
        "text_sha256": "c" * 64,
        "pre_draft_inputs": [
            {
                "packet_id": link["packet_id"],
                "lane": link["lane"],
                "sha256": link["sha256"],
                "status": link["status"],
            }
            for link in pre_draft_links
        ],
        "maker": maker,
        "recognized_comparator_visible": False,
        "other_lane_drafts_visible": draft_type == "ai_human",
        "input_drafts": input_drafts or [],
        "alternatives": [],
        "frozen_at": "2026-07-23T02:00:00Z",
        "status": "frozen",
        "provenance_event_ref": "tos.event.synthetic-translation-draft",
    }


def _synthetic_translation_packet() -> dict:
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/translation-packet.schema.json",
        "schema_version": "tos_translation_packet_v2",
        "packet_id": "tos.translation-packet.synthetic-foundation",
        "experiment_id": "tos-translation-foundation-v1",
        "laboratory_plan": {
            "ref": "ToS/local-content/translation/translation-laboratory-plan.json",
            "sha256": "d" * 64,
        },
        "work_ref": "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
        "source_expression_ref": "tos.expression.synthetic-german",
        "source_anchor_refs": ["tos.anchor.synthetic-source"],
        "source_review_evidence": {
            "review_unit_id": "tos-translation-source-review-v2-001",
            "review_receipt_refs": ["synthetic-pass-1", "synthetic-pass-2"],
            "accepted_diplomatic_sha256": "a" * 64,
            "pass_1_performed_by_real_human": True,
            "pass_2_performed_by_real_human": True,
            "pass_timestamps_distinct": True,
            "source_acceptance": "accept",
        },
        "source_language": "de",
        "target_language": "ru",
        "source_forms": {
            "diplomatic": "Synthetic accepted German source.",
            "diplomatic_sha256": "a" * 64,
            "normalized": "Synthetic accepted German source.",
            "normalized_sha256": "a" * 64,
            "normalization_posture": "none",
            "normalization_event_ref": "tos.event.synthetic-normalization",
        },
        "pre_draft_analysis": {
            "human_only": None,
            "ai_only": None,
            "ai_alternatives": [],
            "cross_lane_analysis_visible": False,
            "recognized_comparator_visible": False,
        },
        "drafts": [],
        "recognized_comparator": {
            "state": "sealed",
            "witnesses": [],
            "content_visible": False,
            "content_consulted": False,
            "revealed_at": None,
            "revealed_by_real_human_ref": None,
            "pre_reveal_freeze": {
                "human_only_draft_ref": None,
                "ai_only_draft_ref": None,
                "ai_alternative_draft_refs": [],
                "ai_human_draft_ref": None,
                "all_required_drafts_frozen": False,
                "freeze_set_sha256": None,
            },
            "post_reveal_change_ledger_required": True,
            "recognized_translation_is_ground_truth": False,
        },
        "alignments": [],
        "evaluation_axes": TRANSLATION_EVALUATION_AXES,
        "evaluations": [],
        "evaluation_completion": {
            "required_axis_count": 19,
            "reviewed_axis_count": 0,
            "all_required_axes_reviewed": False,
            "drafts_with_complete_axis_coverage": [],
        },
        "post_reveal_changes": [],
        "preserved_alternatives": [],
        "adjudications": [],
        "accepted_output": None,
        "personal_read_aloud_layer": {
            "state": "not-provided",
            "review_refs": [],
            "performed_by_real_human": False,
            "separate_from_philological_adjudication": True,
            "used_as_philological_ground_truth": False,
        },
        "rights_and_visibility": {
            "visibility": "local-only",
            "rights_record_refs": ["tos.rights.synthetic-source"],
            "restricted_source_text_redistribution": False,
            "recognized_translation_rights_inherited": True,
            "public_metadata_separate": True,
        },
        "packet_status": "preparing",
        "provenance_event_refs": ["tos.event.synthetic-translation-packet"],
        "authority_boundary": "a translation packet preserves source, independent blind analyses and drafts, comparator influence, alternatives, and human decisions; it does not make a recognized translation, model output, metric, or validator ground truth",
        "packet_version": 1,
    }


def _freeze_synthetic_translation_packet(packet: dict) -> dict:
    human_link = _synthetic_pre_draft_link("human_only", "human")
    ai_link = _synthetic_pre_draft_link("ai_only", "ai")
    alternative_links = [
        _synthetic_pre_draft_link("ai_alternative", "alternative-1"),
        _synthetic_pre_draft_link("ai_alternative", "alternative-2"),
    ]
    human_draft = _synthetic_draft("human_only", "human", [human_link])
    ai_draft = _synthetic_draft("ai_only", "ai", [ai_link])
    alternative_drafts = [
        _synthetic_draft("ai_alternative", "alternative-1", [alternative_links[0]]),
        _synthetic_draft("ai_alternative", "alternative-2", [alternative_links[1]]),
    ]
    ai_human_draft = _synthetic_draft(
        "ai_human",
        "ai-human",
        [human_link, ai_link],
        input_drafts=[
            {
                "role": "human_only",
                "draft_ref": human_draft["draft_id"],
                "draft_sha256": human_draft["text_sha256"],
                "status": "frozen",
            },
            {
                "role": "ai_only",
                "draft_ref": ai_draft["draft_id"],
                "draft_sha256": ai_draft["text_sha256"],
                "status": "frozen",
            },
        ],
    )
    packet["pre_draft_analysis"] = {
        "human_only": human_link,
        "ai_only": ai_link,
        "ai_alternatives": alternative_links,
        "cross_lane_analysis_visible": False,
        "recognized_comparator_visible": False,
    }
    packet["drafts"] = [
        human_draft,
        ai_draft,
        *alternative_drafts,
        ai_human_draft,
    ]
    packet["packet_status"] = "blind-drafts-frozen"
    return packet


def _reveal_synthetic_comparator(packet: dict) -> dict:
    drafts_by_type = {
        draft["draft_type"]: draft
        for draft in packet["drafts"]
        if draft["draft_type"] != "ai_alternative"
    }
    alternatives = [
        draft["draft_id"]
        for draft in packet["drafts"]
        if draft["draft_type"] == "ai_alternative"
    ]
    packet["recognized_comparator"] = {
        "state": "revealed-after-independent-freeze",
        "witnesses": [
            {
                "reference_entry_id": "tos-ref.synthetic-recognized-translation",
                "expression_ref": "tos.expression.synthetic-russian-comparator",
                "item_ref": "tos.item.synthetic-russian-comparator",
                "anchor_refs": ["tos.anchor.synthetic-russian-comparator"],
                "rights_ref": "tos.rights.synthetic-russian-comparator",
                "authority_posture": "authoritative-witness-not-ground-truth",
            }
        ],
        "content_visible": True,
        "content_consulted": False,
        "revealed_at": "2026-07-23T03:00:00Z",
        "revealed_by_real_human_ref": "human:synthetic-reviewer",
        "pre_reveal_freeze": {
            "human_only_draft_ref": drafts_by_type["human_only"]["draft_id"],
            "ai_only_draft_ref": drafts_by_type["ai_only"]["draft_id"],
            "ai_alternative_draft_refs": alternatives,
            "ai_human_draft_ref": drafts_by_type["ai_human"]["draft_id"],
            "all_required_drafts_frozen": True,
            "freeze_set_sha256": "e" * 64,
        },
        "post_reveal_change_ledger_required": True,
        "recognized_translation_is_ground_truth": False,
    }
    packet["packet_status"] = "comparators-revealed"
    return packet


def _freeze_synthetic_comparison(packet: dict) -> dict:
    packet["alignments"] = [
        {
            "alignment_id": "synthetic-alignment",
            "source_anchor_refs": ["tos.anchor.synthetic-source"],
            "target_ref": packet["drafts"][0]["draft_id"],
            "mapping": "1:1",
            "status": "proposed",
            "maker_ref": "human:synthetic-reviewer",
            "provenance_event_ref": "tos.event.synthetic-alignment",
        }
    ]
    packet["post_reveal_changes"] = [
        {
            "change_id": f"synthetic-change-{index}",
            "pre_reveal_draft_ref": draft["draft_id"],
            "change_status": "unchanged",
            "post_reveal_text": draft["text"],
            "post_reveal_text_sha256": draft["text_sha256"],
            "consulted_comparator_reference_ids": [
                "tos-ref.synthetic-recognized-translation"
            ],
            "recognized_translation_influence": "none",
            "accepted_changes": [],
            "rejected_changes": ["Synthetic comparator wording not adopted."],
            "rationale": "Synthetic lifecycle fixture preserves the decision.",
            "maker_ref": "human:synthetic-reviewer",
            "provenance_event_ref": "tos.event.synthetic-post-reveal-comparison",
        }
        for index, draft in enumerate(packet["drafts"], start=1)
    ]
    packet["packet_status"] = "comparison-frozen"
    return packet


def _synthetic_semantic_maker(maker_type: str = "model") -> dict:
    is_human = maker_type == "human"
    return {
        "maker_type": maker_type,
        "agent_ref": f"{maker_type}:synthetic-semantic-worker",
        "performed_by_real_human": is_human,
        "ai_assistance_used": not is_human,
        "model_refs": [] if is_human else ["synthetic-semantic-model"],
    }


def _synthetic_semantic_stage(
    stage_name: str,
    *,
    status: str,
    body: dict | None = None,
    maker: dict | None = None,
    blocker_refs: list[str] | None = None,
) -> dict:
    active = status not in {"not-started", "blocked"}
    return {
        "stage": stage_name,
        "status": status,
        "source_anchor_refs": ["tos.anchor.synthetic-source"],
        "source_return_verified": active,
        "body": body or {},
        "maker": maker if active else None,
        "provenance_event_ref": "tos.event.synthetic-semantic-stage" if active else None,
        "review_status": "unreviewed" if active else "not-started",
        "blocker_refs": blocker_refs or [],
    }


def _synthetic_semantic_packet() -> dict:
    maker = _synthetic_semantic_maker()
    bodies = {
        "exact_form": {
            "exact_form_local_ref": "ToS/local-content/semantic/synthetic-form.txt",
            "exact_form_sha256": "a" * 64,
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "page_return_verified": True,
            "source_value_tracked": False,
        },
        "frequency_and_concordance": {
            "count": 3,
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "concordance_refs": ["synthetic-concordance"],
            "observation_refs": ["tos.event.synthetic-frequency-observation"],
            "counting_method_ref": "synthetic-counting-method",
            "frequency_only_basis": False,
        },
        "context": {
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "observation_refs": ["tos.event.synthetic-context-observation"],
            "context_local_ref": "ToS/local-content/semantic/synthetic-context.txt",
            "context_sha256": "c" * 64,
            "source_values_tracked": False,
        },
        "morphology": {
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "claim_refs": ["tos.claim.synthetic-morphology"],
            "analysis": {"part_of_speech": "synthetic"},
        },
        "lemma": {
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "lexeme_ref": "tos.lexeme.synthetic-form",
            "lemma_label": "synthetic-form",
            "claim_refs": ["tos.claim.synthetic-lemma"],
        },
        "recurrence_within_section": {
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "declared_set_ref": "synthetic-section-set",
            "observation_refs": ["tos.event.synthetic-section-recurrence"],
            "membership_method_ref": "synthetic-membership-method",
        },
        "recurrence_within_work": {
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "declared_set_ref": "synthetic-work-set",
            "observation_refs": ["tos.event.synthetic-work-recurrence"],
            "membership_method_ref": "synthetic-membership-method",
        },
        "recurrence_within_author_corpus": {
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "declared_set_ref": "synthetic-author-set",
            "observation_refs": ["tos.event.synthetic-author-recurrence"],
            "membership_method_ref": "synthetic-membership-method",
        },
        "translation_correspondences": {
            "translation_evidence_ids": ["tos-translation-evidence.synthetic"],
            "correspondence_claim_refs": [
                "tos.claim.synthetic-translation-correspondence"
            ],
            "translation_differences_preserved": True,
            "recognized_translation_is_ground_truth": False,
        },
        "stable_sign_candidate": {
            "candidate_statement": "Synthetic stable-sign candidate, not a decision.",
            "candidate_ref": "tos.annotation.synthetic-sign-candidate",
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "lexeme_refs": ["tos.lexeme.synthetic-form"],
            "proposal_claim_refs": ["tos.claim.synthetic-sign-candidate"],
            "recurrence_evidence_refs": ["synthetic-section", "synthetic-work"],
            "context_variation_considered": True,
            "metaphorical_development_considered": True,
            "compositional_role_considered": True,
            "translation_divergence_considered": True,
            "negations_and_inversions_considered": True,
            "related_sign_leads_considered": True,
            "frequency_only_basis": False,
            "human_opinion_required": True,
            "proposed_status": "stable-sign-candidate-not-confirmed",
        },
    }
    stages = []
    for index, stage_name in enumerate(SEMANTIC_STAGE_ORDER):
        if index < 10:
            stages.append(
                _synthetic_semantic_stage(
                    stage_name,
                    status="proposed",
                    body=bodies.get(stage_name, {"statement": f"Synthetic {stage_name}."}),
                    maker=maker,
                )
            )
        elif index == 10:
            stages.append(
                _synthetic_semantic_stage(stage_name, status="not-started")
            )
        else:
            stages.append(
                _synthetic_semantic_stage(
                    stage_name,
                    status="blocked",
                    blocker_refs=["manual-sign-decision-missing"],
                )
            )
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/semantic-ladder-packet.schema.json",
        "schema_version": "tos_semantic_ladder_packet_v4",
        "packet_id": "tos.semantic-ladder-packet.synthetic-foundation",
        "work_ref": "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
        "source_expression_ref": "tos.expression.synthetic-german",
        "task_specific_source_gate": {
            "gate_kind": "task-specific-edition-reading-source-bundle",
            "source_anchor_refs": ["tos.anchor.synthetic-source"],
            "edition_reading_admission_ref": "ToS/evidence/synthetic-edition-reading.json",
            "edition_reading_admission_sha256": "a" * 64,
            "source_reading_status": "edition-attested",
            "source_review_event_ref": "tos.event.synthetic-source-review",
            "local_source_ref": "ToS/local-content/semantic/synthetic-source.txt",
            "local_source_sha256": "a" * 64,
            "local_source_tracked": False,
            "source_observation_allowed": True,
            "universal_packet_completion_required": False,
            "language_competence_status": "evidence-attested",
            "language_competence_evidence_refs": [
                "tos.review.synthetic-language-competence"
            ],
            "linguistic_claim_review_allowed": True,
            "missing_competence_effect": "leave-unresolved-never-infer-acceptance",
            "gate_status": "satisfied",
        },
        "source_forms": {
            "diplomatic_local_ref": "ToS/local-content/semantic/synthetic-diplomatic.txt",
            "diplomatic_sha256": "a" * 64,
            "normalized_local_ref": "ToS/local-content/semantic/synthetic-normalized.txt",
            "normalized_sha256": "a" * 64,
            "source_values_tracked": False,
        },
        "candidate_ref": "tos.annotation.synthetic-sign-candidate",
        "accepted_sign_ref": None,
        "translation_evidence": [
            {
                "evidence_id": "tos-translation-evidence.synthetic",
                "evidence_kind": "translation-packet",
                "ref": "ToS/local-content/translation/synthetic-packet.json",
                "sha256": "b" * 64,
                "review_status": "human-adjudicated",
                "rights_ref": "tos.rights.synthetic-translation",
                "recognized_translation_is_ground_truth": False,
            }
        ],
        "assurance_policy": {
            "operating_model": "solo_human_plus_ai",
            "routine_human_work_for_prepared_packet": False,
            "promotion_checkpoint_posture": (
                "opens-only-for-concrete-source-grounded-sign-candidate"
            ),
            "unassisted_operator_baseline_required": True,
            "baseline_frozen_before_model_suggestions": True,
            "second_human_review_posture": "triggered-exception-not-routine",
            "second_human_review_triggers": [
                "declared-language-competence-gap",
                "high-impact-canon-promotion",
                "persistent-source-grounded-ambiguity",
                "operator-baseline-instability",
            ],
            "model_disagreement_is_human_perspective": False,
            "rationale_preserved_separately_from_label": True,
            "alternatives_uncertainty_and_refusal_preserved": True,
            "human_work_scheduled": True,
        },
        "stage_order": SEMANTIC_STAGE_ORDER,
        "stages": stages,
        "frequency_is_not_sufficient": True,
        "every_stage_returns_to_source_page": True,
        "model_may_propose_but_not_confirm": True,
        "manual_sign_decision_required_before_interpretation": True,
        "packet_status": "awaiting-manual-sign-decision",
        "result": {
            "human_decision_refs": [],
            "relation_refs": [],
            "concept_refs": [],
            "claim_refs": [],
            "graph_projection_refs": [],
            "promotion_authorized": False,
            "conclusion": "Synthetic candidate awaits a real-human decision.",
        },
        "provenance_event_refs": ["tos.event.synthetic-semantic-packet"],
        "authority_boundary": "this packet keeps edition-reading evidence independent from language competence: exact-form, frequency, context, and explicitly typed model proposals may return to an attested Edition reading, but no linguistic claim becomes reviewed and no sign, interpretation, relation, concept, claim, graph, canon, transfer, or publication authority follows without its own competence-appropriate and real-human evidence",
        "packet_version": 1,
    }


def _accept_synthetic_sign(packet: dict) -> dict:
    packet["stages"][10] = _synthetic_semantic_stage(
        "manual_confirmation_or_rejection",
        status="human-accepted",
        body={
            "decision": "accept-with-limits",
            "sign_status_assigned": "stable-sign",
            "accepted_sign_ref": "tos.sign.synthetic-foundation",
            "rationale": "Synthetic real-human decision fixture.",
            "review_receipt_ref": "tos.review.synthetic-manual-sign",
            "unassisted_baseline_ref": "tos.review.synthetic-unassisted-baseline",
            "model_suggestions_hidden_until_baseline_frozen": True,
            "considered_evidence_refs": ["synthetic-context", "synthetic-recurrence"],
            "decision_owned_by_real_human": True,
            "frequency_was_not_sole_basis": True,
            "ai_proposal_is_authority": False,
        },
        maker=_synthetic_semantic_maker("human"),
    )
    packet["stages"][10]["review_status"] = "accepted-with-limits"
    packet["accepted_sign_ref"] = "tos.sign.synthetic-foundation"
    packet["assurance_policy"]["human_work_scheduled"] = False
    packet["result"]["human_decision_refs"] = [
        "tos.review.synthetic-manual-sign"
    ]
    packet["result"]["promotion_authorized"] = True
    packet["result"]["conclusion"] = "Synthetic stable sign accepted with limits."
    packet["packet_status"] = "manual-sign-accepted"
    return packet


def _project_synthetic_semantic_graph(packet: dict) -> dict:
    packet["stages"][11] = _synthetic_semantic_stage(
        "relations_between_signs",
        status="proposed",
        body={
            "sign_refs": [
                "tos.sign.synthetic-foundation",
                "tos.sign.synthetic-related",
            ],
            "relation_records": [
                {
                    "relation_ref": "tos.relation.synthetic-sign-relation",
                    "subject_sign_ref": "tos.sign.synthetic-foundation",
                    "predicate": "synthetic_relation",
                    "object_sign_ref": "tos.sign.synthetic-related",
                    "claim_ref": "tos.claim.synthetic-sign-relation",
                    "evidence_anchor_refs": ["tos.anchor.synthetic-source"],
                    "review_status": "unreviewed",
                }
            ],
            "each_relation_resolves_to_claim_and_evidence": True,
        },
        maker=_synthetic_semantic_maker(),
    )
    packet["stages"][12] = _synthetic_semantic_stage(
        "conceptual_interpretations",
        status="proposed",
        body={
            "accepted_sign_ref": "tos.sign.synthetic-foundation",
            "concept_refs": ["tos.concept.synthetic-interpretation"],
            "claim_refs": ["tos.claim.synthetic-concept"],
            "interpretation_statement": "Synthetic conceptual interpretation.",
        },
        maker=_synthetic_semantic_maker(),
    )
    packet["stages"][13] = _synthetic_semantic_stage(
        "competing_readings",
        status="proposed",
        body={
            "primary_claim_refs": ["tos.claim.synthetic-concept"],
            "competing_claim_refs": ["tos.claim.synthetic-counterreading"],
            "bounded_rationale": "Synthetic competing reading.",
            "unresolved_is_allowed": True,
        },
        maker=_synthetic_semantic_maker(),
    )
    packet["stages"][14] = _synthetic_semantic_stage(
        "graph_projection",
        status="projected",
        body={
            "projection_ref": "ToS/derived-exports/synthetic-semantic-graph.json",
            "projection_sha256": "c" * 64,
            "sign_refs": ["tos.sign.synthetic-foundation"],
            "relation_refs": ["tos.relation.synthetic-sign-relation"],
            "claim_refs": ["tos.claim.synthetic-graph"],
            "rebuildable_from_stronger_records": True,
            "projection_is_authority": False,
        },
        maker={
            "maker_type": "software",
            "agent_ref": "software:synthetic-projector",
            "performed_by_real_human": False,
            "ai_assistance_used": False,
            "model_refs": [],
        },
    )
    packet["result"]["relation_refs"] = ["tos.relation.synthetic-sign-relation"]
    packet["result"]["concept_refs"] = ["tos.concept.synthetic-interpretation"]
    packet["result"]["claim_refs"] = [
        "tos.claim.synthetic-sign-relation",
        "tos.claim.synthetic-concept",
        "tos.claim.synthetic-counterreading",
        "tos.claim.synthetic-graph",
    ]
    packet["result"]["graph_projection_refs"] = [
        "ToS/derived-exports/synthetic-semantic-graph.json"
    ]
    packet["result"]["conclusion"] = "Synthetic graph projection completed."
    packet["packet_status"] = "graph-projected"
    return packet


def _synthetic_discovery_record() -> dict:
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/material-discovery-record.schema.json",
        "schema_version": "tos_material_discovery_record_v1",
        "discovery_id": "tos.discovery.synthetic-source",
        "protocol_ref": "ToS/source-witnesses/discovery/DISCOVERY_PROTOCOL.md",
        "target": {
            "target_kind": "edition",
            "known_tos_refs": ["tos.work.friedrich-nietzsche.also-sprach-zarathustra"],
            "description": "Synthetic exact-edition discovery target.",
            "required_properties": ["edition identity", "originating record"],
            "acceptable_substitutions": [],
            "languages": ["de"],
            "formats": ["metadata"],
            "purpose_ref": "synthetic-test",
        },
        "channels": [
            {
                "channel_id": "channel-national-catalog",
                "sequence": 1,
                "channel_type": "national-catalog",
                "role": "originating-record",
                "source_name": "Synthetic national catalog",
                "endpoint_url": "https://example.invalid/catalog",
                "interface_type": "api",
                "interface_version": "synthetic-v1",
                "exact_query": "synthetic exact query",
                "queried_at": "2026-07-23T00:00:00Z",
                "elapsed_seconds": 1.0,
                "result_order_preserved": True,
                "results": [
                    {
                        "result_id": "tos-discovery-result.synthetic-1",
                        "rank": 1,
                        "title_as_displayed": "Synthetic catalog result",
                        "result_url": "https://example.invalid/catalog/1",
                        "originating_record_url": "https://example.invalid/catalog/1",
                        "identifiers": [{"scheme": "synthetic", "value": "1"}],
                        "available_formats": ["metadata"],
                        "declared_rights": {
                            "statement": None,
                            "scope": "metadata",
                            "evidence_url": None,
                            "tos_conclusion": "evidence-only-not-a-rights-conclusion",
                        },
                        "availability": "metadata-only",
                        "machine_interface": "api",
                        "decision": "select",
                        "rationale": "Synthetic originating record fixture.",
                        "acquisition": {
                            "downloaded": False,
                            "acquired_at": None,
                            "byte_size": None,
                            "sha256": None,
                            "event_ref": None,
                        },
                        "snapshot": {
                            "state": "not-captured",
                            "format": None,
                            "sha256": None,
                            "reason": "Synthetic fixture does not capture external content.",
                        },
                    }
                ],
            }
        ],
        "channel_comparison": [
            {
                "channel_id": "channel-national-catalog",
                "completeness": "adequate",
                "metadata_precision": "strong",
                "rights_clarity": "limited",
                "machine_interface_quality": "strong",
                "human_minutes": 1,
                "machine_seconds": 1,
                "notes": "Synthetic comparison fixture.",
            }
        ],
        "selected_result_ids": ["tos-discovery-result.synthetic-1"],
        "rejected_result_ids": [],
        "rights_inference_from_availability_prohibited": True,
        "general_web_search_is_last_resort": True,
        "technical_access_bypass_used": False,
        "maker": {"maker_type": "model", "agent_ref": "model:synthetic-discovery"},
        "started_at": "2026-07-23T00:00:00Z",
        "ended_at": "2026-07-23T00:00:01Z",
        "status": "executed",
        "provenance_event_refs": ["tos.event.synthetic-discovery"],
        "record_version": 1,
    }


class SourceWitnessFoundationTests(unittest.TestCase):
    def test_source_anchor_v2_synthetic_abc_resolves_real_segments(self) -> None:
        issues, report = foundation.validate_source_anchor_v2_lab(REPO_ROOT)

        self.assertEqual([], issues)
        self.assertEqual(
            [
                ("A", "alternatives", "The source remains the authority."),
                ("B", "single", "café"),
                ("C", "refinement_chain", "relations"),
            ],
            [
                (row["variant_id"], row["mode"], row["selection"])
                for row in report["variants"]
            ],
        )
        self.assertTrue(
            all(
                row["resolution_status"] == "mechanically_resolved"
                and row["review_status"] == "unreviewed"
                for row in report["variants"]
            )
        )
        self.assertEqual(
            {
                "alternatives_resolve_to_different_passages": "rejected",
                "normalized_page_region_overflow": "rejected",
                "refinement_steps_reversed": "rejected",
                "representation_digest_drift": "rejected",
                "tracked_nonpublic_text_quote": "rejected",
                "utf16_offset_mislabeled_as_unicode_code_point": "rejected",
            },
            report["negative_controls"],
        )

    def test_source_anchor_v2_requires_explicit_position_and_state_semantics(self) -> None:
        schema = json.loads(
            (REPO_ROOT / "ToS/contracts/source-anchor-v2.schema.json").read_text(
                encoding="utf-8"
            )
        )
        validator_class = validator_for(schema)
        validator_class.check_schema(schema)
        validator = validator_class(schema, format_checker=FormatChecker())
        anchor = json.loads(
            (
                REPO_ROOT
                / "ToS/research-packets/foundation-laboratory-2026-07/"
                "source-anchor-v2-abc/variant-b.anchor.json"
            ).read_text(encoding="utf-8")
        )

        missing_unit = copy.deepcopy(anchor)
        del missing_unit["selector_payload"]["expression"]["selector"]["selector"][
            "position_unit"
        ]
        self.assertTrue(list(validator.iter_errors(missing_unit)))

        wrong_identity_kind = copy.deepcopy(anchor)
        wrong_identity_kind["target"]["item_id"] = "tos.agent.synthetic"
        self.assertTrue(list(validator.iter_errors(wrong_identity_kind)))

        unsupported_review_claim = copy.deepcopy(anchor)
        unsupported_review_claim["review_status"] = "accepted"
        self.assertTrue(list(validator.iter_errors(unsupported_review_claim)))

        local_configuration_path = copy.deepcopy(anchor)
        local_configuration_path["selector_method"]["configuration_ref"] = (
            "C:\\forbidden\\config.json"
        )
        self.assertTrue(list(validator.iter_errors(local_configuration_path)))

        unbound_second_version = copy.deepcopy(anchor)
        unbound_second_version["anchor_version"] = 2
        self.assertTrue(list(validator.iter_errors(unbound_second_version)))

        self_supersession = copy.deepcopy(anchor)
        self_supersession["supersedes_anchor_ref"] = self_supersession["anchor_id"]
        self.assertIn(
            "source anchor cannot supersede itself",
            foundation._anchor_v2_semantic_issues(self_supersession),
        )

        reversed_interval = copy.deepcopy(anchor)
        position = reversed_interval["selector_payload"]["expression"]["selector"][
            "selector"
        ]
        position["start"] = position["end"]
        self.assertIn(
            "text_position interval is empty or reversed",
            foundation._anchor_v2_semantic_issues(reversed_interval),
        )

        v1_schema = json.loads(
            (REPO_ROOT / "ToS/contracts/source-anchor.schema.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual(
            "tos_source_anchor_v1",
            v1_schema["properties"]["schema_version"]["const"],
        )

        container_anchor = json.loads(
            (
                REPO_ROOT
                / "ToS/research-packets/foundation-laboratory-2026-07/"
                "source-anchor-v2-abc/variant-c.anchor.json"
            ).read_text(encoding="utf-8")
        )
        container_envelope = container_anchor["selector_payload"]["expression"][
            "steps"
        ][0]
        with self.assertRaisesRegex(
            ValueError,
            "container member is not declared",
        ):
            foundation._anchor_v2_apply_selector(
                container_envelope,
                scope={"value": b'{"members":[]}'},
                resources_by_path={},
            )

    def test_source_text_layer_synthetic_abc_replays_real_unicode_edits(self) -> None:
        issues, report = foundation.validate_source_text_layer_lab(REPO_ROOT)

        self.assertEqual([], issues)
        self.assertEqual("café", report["source_anchor_selection"])
        self.assertEqual(
            [
                ("A", "raw_ocr", "cafe"),
                ("B", "diplomatic_transcription", "café"),
                ("C", "normalized_text", "café"),
            ],
            [
                (row["variant_id"], row["layer_role"], row["text"])
                for row in report["variants"]
            ],
        )
        self.assertTrue(
            all(
                row["review_status"] == "unreviewed"
                and row["accepted_uses"] == []
                and row["publication_authorized"] is False
                and row["rights_record_refs"] == []
                for row in report["variants"]
            )
        )
        self.assertEqual(
            {
                "edit_span_or_digest_mismatch": "rejected",
                "input_record_digest_drift": "rejected",
                "language_sensitive_use_without_competence": "rejected",
                "normalized_layer_claims_diplomatic_use": "rejected",
                "publication_use_without_publication_authority": "rejected",
                "self_supersession": "rejected",
                "silent_correction_without_operations": "rejected",
                "tracked_nonpublic_explicit_text": "rejected",
                "unreviewed_layer_claims_accepted_use": "rejected",
            },
            report["negative_controls"],
        )

    def test_source_text_layer_keeps_review_competence_and_visibility_fail_closed(
        self,
    ) -> None:
        schema = json.loads(
            (REPO_ROOT / "ToS/contracts/source-text-layer.schema.json").read_text(
                encoding="utf-8"
            )
        )
        validator_class = validator_for(schema)
        validator_class.check_schema(schema)
        validator = validator_class(schema, format_checker=FormatChecker())
        layer = json.loads(
            (
                REPO_ROOT
                / "ToS/research-packets/foundation-laboratory-2026-07/"
                "source-text-layer-abc/variant-b.layer.json"
            ).read_text(encoding="utf-8")
        )

        silent_correction = copy.deepcopy(layer)
        silent_correction["derivation"]["change_payload"] = {"kind": "none"}
        self.assertTrue(list(validator.iter_errors(silent_correction)))

        unreviewed_acceptance = copy.deepcopy(layer)
        unreviewed_acceptance["admission"]["accepted_uses"] = ["citation"]
        self.assertTrue(list(validator.iter_errors(unreviewed_acceptance)))

        competence_bypass = copy.deepcopy(layer)
        competence_bypass["admission"].update(
            {
                "review_status": "accepted_with_limits",
                "review_ref": "tos.review.synthetic.source-text-layer",
                "human_review_performed": True,
                "human_language_competence": "blocked",
                "accepted_uses": ["semantic_analysis"],
            }
        )
        self.assertTrue(list(validator.iter_errors(competence_bypass)))

        rejected_but_used = copy.deepcopy(layer)
        rejected_but_used["admission"].update(
            {
                "review_status": "rejected",
                "review_ref": "tos.review.synthetic.source-text-layer",
                "human_review_performed": True,
                "accepted_uses": ["citation"],
                "promotion_authorized": True,
            }
        )
        self.assertTrue(list(validator.iter_errors(rejected_but_used)))

        unbound_identity_copy = copy.deepcopy(layer)
        unbound_identity_copy["derivation"].update(
            {
                "method": "identity_copy",
                "input_layers": [],
                "change_payload": {"kind": "none"},
            }
        )
        self.assertTrue(list(validator.iter_errors(unbound_identity_copy)))

        ungrounded_publication = copy.deepcopy(layer)
        ungrounded_publication["representation"]["publication_authorized"] = True
        self.assertTrue(list(validator.iter_errors(ungrounded_publication)))

        rejected_active_edit = copy.deepcopy(layer)
        rejected_active_edit["derivation"]["change_payload"]["operations"][0][
            "status"
        ] = "rejected"
        self.assertTrue(list(validator.iter_errors(rejected_active_edit)))

        unbound_second_version = copy.deepcopy(layer)
        unbound_second_version["layer_version"] = 2
        unbound_second_version["supersedes_layer_ref"] = None
        self.assertTrue(list(validator.iter_errors(unbound_second_version)))

        tracked_nonpublic = copy.deepcopy(layer)
        tracked_nonpublic["representation"].update(
            {
                "content_visibility": "local_only",
                "publication_authorized": False,
            }
        )
        self.assertIn(
            "tracked source text layer must be public content",
            foundation._source_text_layer_semantic_issues(tracked_nonpublic),
        )

        self_supersession = copy.deepcopy(layer)
        self_supersession["supersedes_layer_ref"] = self_supersession["layer_id"]
        self.assertIn(
            "source text layer cannot supersede itself",
            foundation._source_text_layer_semantic_issues(self_supersession),
        )

        reversed_scope = copy.deepcopy(layer)
        reversed_scope["representation"]["text_scope"].update(
            {"start": 5, "end": 4}
        )
        self.assertIn(
            "representation text scope is reversed",
            foundation._source_text_layer_semantic_issues(reversed_scope),
        )

        legacy_gold = json.loads(
            (REPO_ROOT / "ToS/contracts/manual-gold-status.schema.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual(
            "tos_manual_gold_status_v1",
            legacy_gold["properties"]["schema_version"]["const"],
        )

    def test_provenance_v2_synthetic_abc_preserves_success_transform_and_failure(
        self,
    ) -> None:
        issues, report = foundation.validate_provenance_v2_lab(REPO_ROOT)

        self.assertEqual([], issues)
        self.assertEqual(
            [
                ("A", "completed", 0, "identity_copy_of"),
                ("B", "completed", 0, "revision_of"),
                ("C", "failed", 7, None),
            ],
            [
                (
                    row["variant_id"],
                    row["status"],
                    row["exit_code"],
                    row["relation"],
                )
                for row in report["variants"]
            ],
        )
        self.assertEqual(
            report["variants"][0]["input_sha256"],
            report["variants"][0]["output_sha256"],
        )
        self.assertNotEqual(
            report["variants"][1]["input_sha256"],
            report["variants"][1]["output_sha256"],
        )
        self.assertIsNone(report["variants"][2]["output_sha256"])
        self.assertIsNotNone(report["variants"][2]["byproduct_sha256"])
        self.assertTrue(
            all(
                row["signature_status"] == "unsigned"
                and row["human_review_status"] == "not_performed"
                for row in report["variants"]
            )
        )
        self.assertEqual(
            {
                "command_digest_drift",
                "completed_without_output",
                "derivation_endpoint_escape",
                "event_record_digest_drift",
                "failed_with_authoritative_output",
                "identity_copy_content_drift",
                "input_fixity_drift",
                "manual_change_without_receipt",
                "model_event_without_invocation",
                "publication_without_authority",
                "replay_ready_with_withheld_command",
                "self_supersession",
                "unattested_human_review",
                "unsigned_claimed_signature_verification",
            },
            set(report["negative_controls"]),
        )
        self.assertEqual(
            {"rejected"},
            set(report["negative_controls"].values()),
        )

    def test_provenance_v2_separates_replay_fixity_authentication_and_authority(
        self,
    ) -> None:
        schema = json.loads(
            (REPO_ROOT / "ToS/contracts/provenance-event-v2.schema.json").read_text(
                encoding="utf-8"
            )
        )
        validator_class = validator_for(schema)
        validator_class.check_schema(schema)
        validator = validator_class(schema, format_checker=FormatChecker())
        event = json.loads(
            (
                REPO_ROOT
                / "ToS/research-packets/foundation-laboratory-2026-07/"
                "provenance-event-v2-abc/variant-a.event.v2.json"
            ).read_text(encoding="utf-8")
        )

        completed_without_output = copy.deepcopy(event)
        completed_without_output["entities"]["outputs"] = []
        completed_without_output["derivations"] = []
        self.assertTrue(list(validator.iter_errors(completed_without_output)))

        model_without_invocation = copy.deepcopy(event)
        model_without_invocation["activity"]["event_type"] = "model_inference"
        self.assertTrue(list(validator.iter_errors(model_without_invocation)))

        publication_without_authority = copy.deepcopy(event)
        publication_without_authority["rights_and_visibility"][
            "publication_authorized"
        ] = True
        self.assertTrue(list(validator.iter_errors(publication_without_authority)))

        fabricated_human_review = copy.deepcopy(event)
        fabricated_human_review["review_and_authority"].update(
            {
                "human_review_status": "performed",
                "review_bindings": [
                    {
                        "ref": (
                            "ToS/research-packets/foundation-laboratory-2026-07/"
                            "provenance-event-v2-abc/plan.json"
                        ),
                        "sha256": "0" * 64,
                    }
                ],
            }
        )
        self.assertTrue(list(validator.iter_errors(fabricated_human_review)))

        command_drift = copy.deepcopy(event)
        command_drift["method"]["command_capture"]["argv_sha256"] = "0" * 64
        self.assertIn(
            "inline command argv digest drifted",
            foundation._provenance_v2_semantic_issues(command_drift),
        )

        unsigned_claim = copy.deepcopy(event)
        unsigned_claim["evidence_authentication"][
            "verification_status"
        ] = "signature_verified"
        self.assertIn(
            "unsigned provenance event claims signature verification",
            foundation._provenance_v2_semantic_issues(unsigned_claim),
        )

        legacy_schema = json.loads(
            (REPO_ROOT / "ToS/contracts/provenance-event.schema.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual(
            "tos_provenance_event_v1",
            legacy_schema["properties"]["schema_version"]["const"],
        )

    def test_source_witness_claim_catalog_is_exact_source_returnable_projection(
        self,
    ) -> None:
        catalog_root = REPO_ROOT / "ToS/source-witnesses/catalog"
        manifest = json.loads(
            (catalog_root / "catalog.manifest.json").read_text(encoding="utf-8")
        )
        claim_entries = [
            json.loads(line)
            for line in (catalog_root / "claims.jsonl")
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
        ]
        work_entries = {
            entry["record_id"]: entry
            for line in (catalog_root / "works.jsonl")
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
            for entry in (json.loads(line),)
        }
        object_ids = {
            entry["record_id"]
            for filename in catalog_builder.RECORD_FILES.values()
            for line in (catalog_root / filename)
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
            for entry in (json.loads(line),)
        }
        source_claims: dict[str, tuple[str, int, dict, str]] = {}
        for basename in catalog_builder.CLAIM_SOURCE_BASENAMES:
            for path in sorted(
                (REPO_ROOT / catalog_builder.SOURCE_ROOT).rglob(basename)
            ):
                relative = path.relative_to(REPO_ROOT).as_posix()
                for line_number, raw_line in enumerate(
                    path.read_text(encoding="utf-8").splitlines(),
                    start=1,
                ):
                    if not raw_line.strip():
                        continue
                    claim = json.loads(raw_line)
                    canonical = catalog_builder.canonical_json(claim).encode(
                        "utf-8"
                    )
                    source_claims[claim["claim_id"]] = (
                        relative,
                        line_number,
                        claim,
                        hashlib.sha256(canonical).hexdigest(),
                    )

        self.assertEqual("tos_source_witness_catalog_v3", manifest["schema_version"])
        self.assertEqual(
            "ToS/source-witnesses/catalog/claims.jsonl",
            manifest["claim_file"],
        )
        self.assertEqual(96, manifest["counts"]["object_total"])
        self.assertEqual(140, manifest["counts"]["claim"])
        self.assertEqual(236, manifest["counts"]["total"])
        self.assertEqual(140, len(claim_entries))
        self.assertEqual(set(source_claims), {entry["claim_id"] for entry in claim_entries})

        for entry in claim_entries:
            relative, line_number, claim, claim_digest = source_claims[
                entry["claim_id"]
            ]
            self.assertEqual(relative, entry["source_claim_file_ref"])
            self.assertEqual(line_number, entry["source_claim_line"])
            self.assertEqual(claim_digest, entry["claim_sha256"])
            for field in (
                "claim_type",
                "assertion_layer",
                "subject_ref",
                "predicate",
                "object",
                "evidence_refs",
                "maker",
                "provenance_event_ref",
                "epistemic_status",
                "review_status",
                "claim_version",
            ):
                self.assertEqual(claim[field], entry[field])
            self.assertEqual(
                [review["review_id"] for review in claim["reviews"]],
                entry["review_refs"],
            )
            self.assertEqual(claim.get("qualifiers"), entry.get("qualifiers"))
            self.assertIn(entry["subject_ref"], object_ids)
            if (
                isinstance(entry["object"], str)
                and entry["object"].startswith("tos.")
            ):
                self.assertIn(entry["object"], object_ids)

        self.assertEqual(
            {
                "bibliographic_assertion": 123,
                "scholarly_report": 17,
            },
            {
                layer: sum(
                    entry["assertion_layer"] == layer for entry in claim_entries
                )
                for layer in {
                    entry["assertion_layer"] for entry in claim_entries
                }
            },
        )
        self.assertTrue(
            all(
                entry["claim_type"] in {"bibliographic", "relation"}
                and entry["review_status"] == "unreviewed"
                and entry["visibility"] == "public_metadata_only"
                for entry in claim_entries
            )
        )
        self.assertEqual(
            2,
            sum(
                entry["claim_type"] == "relation"
                and entry["predicate"] == "is_derivative_of"
                for entry in claim_entries
            ),
        )
        self.assertEqual(
            {
                "has_expression": 26,
                "embodied_by": 26,
                "exemplified_by": 19,
                "is_derivative_of": 2,
            },
            {
                predicate: sum(
                    entry["predicate"] == predicate for entry in claim_entries
                )
                for predicate in {
                    "has_expression",
                    "embodied_by",
                    "exemplified_by",
                    "is_derivative_of",
                }
            },
        )
        ecce_roles = {
            entry["predicate"]: entry["object"]
            for entry in claim_entries
            if entry["subject_ref"]
            in {
                "tos.work.friedrich-nietzsche.ecce-homo",
                "tos.edition.friedrich-nietzsche.ecce-homo."
                "leipzig-insel-verlag-1908",
            }
            and entry["predicate"]
            in {"authored_by", "edited_by", "afterword_by", "designed_by"}
        }
        self.assertEqual(
            {
                "authored_by": "tos.agent.friedrich-nietzsche",
                "edited_by": "tos.agent.raoul-richter",
                "afterword_by": "tos.agent.raoul-richter",
                "designed_by": "tos.agent.henry-van-de-velde",
            },
            ecce_roles,
        )
        authorship_claims = [
            entry for entry in claim_entries if entry["predicate"] == "authored_by"
        ]
        self.assertEqual(7, len(authorship_claims))
        self.assertEqual(set(work_entries), {entry["subject_ref"] for entry in authorship_claims})
        self.assertTrue(
            all(
                entry["object"] == "tos.agent.friedrich-nietzsche"
                and entry["maker"]
                == {"maker_type": "model", "agent_ref": "model:codex"}
                and entry["review_status"] == "unreviewed"
                and entry["visibility"] == "public_metadata_only"
                for entry in authorship_claims
            )
        )
        for claim in authorship_claims:
            self.assertEqual(
                [claim["claim_id"]],
                work_entries[claim["subject_ref"]]["links"][
                    "responsibility_claim_refs"
                ],
            )
        chronology_claims = [
            entry
            for entry in claim_entries
            if entry["predicate"] == "first_publication_chronology"
        ]
        self.assertEqual(7, len(chronology_claims))
        self.assertEqual(
            set(work_entries),
            {entry["subject_ref"] for entry in chronology_claims},
        )
        for claim in chronology_claims:
            self.assertEqual("first_publication", claim["object"]["chronology_kind"])
            self.assertTrue(claim["object"]["ordering_warning"])
            self.assertEqual(
                [claim["claim_id"]],
                work_entries[claim["subject_ref"]]["links"][
                    "chronology_claim_refs"
                ],
            )

        provision_claims = [
            entry
            for entry in claim_entries
            if entry["predicate"] == "provision_activity"
        ]
        self.assertEqual(19, len(provision_claims))
        self.assertEqual(
            {
                "tos.place.leipzig",
                "tos.place.chemnitz",
                "tos.place.saint-petersburg",
                "tos.place.moscow",
            },
            {
                place["normalized_place_ref"]
                for claim in provision_claims
                for place in claim["object"]["places"]
            },
        )
        self.assertEqual(
            {
                "tos.organization.c-g-naumann-verlag-leipzig",
                "tos.organization.insel-verlag-anton-kippenberg-leipzig",
                "tos.organization.ernst-schmeitzner-verlagsbuchhandlung-chemnitz",
                "tos.organization.zhizn-dlya-vsekh-saint-petersburg",
                "tos.organization.bratya-v-i-i-linnik-printing-saint-petersburg",
                "tos.organization.druckerei-c-g-naumann-leipzig",
                "tos.organization.prometey-publishing-saint-petersburg",
                "tos.organization.energiya-typolithography-saint-petersburg",
                "tos.organization.reader-journal-editorial-office-moscow",
                "tos.organization.m-m-stasyulevich-printing-saint-petersburg",
            },
            {
                agent["normalized_agent_ref"]
                for claim in provision_claims
                for agent in claim["object"]["agents"]
                if "normalized_agent_ref" in agent
            },
        )
        self.assertTrue(
            all(
                claim["object"]["temporal"]["role"] == "statement_date"
                and claim["review_status"] == "unreviewed"
                and claim["visibility"] == "public_metadata_only"
                for claim in provision_claims
            )
        )

    def test_zarathustra_parts_1_to_4_provision_identity_is_exact_and_distinct(
        self,
    ) -> None:
        claim = json.loads(
            ZARATHUSTRA_PART_1_PROVISION_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()[0]
        )
        self.assertEqual(
            "tos.edition.friedrich-nietzsche.also-sprach-zarathustra."
            "chemnitz-schmeitzner-1883-part-1",
            claim["subject_ref"],
        )
        self.assertEqual("authority_record", claim["object"]["statement_basis"])
        self.assertEqual("Schmeitzner; Chemnitz; 1883", claim["object"]["reported_statement"])
        self.assertEqual(
            "tos.place.chemnitz",
            claim["object"]["places"][0]["normalized_place_ref"],
        )
        self.assertEqual(
            "tos.organization.ernst-schmeitzner-verlagsbuchhandlung-chemnitz",
            claim["object"]["agents"][0]["normalized_agent_ref"],
        )
        self.assertEqual(
            {
                "kind": "date",
                "calendar": "gregorian",
                "value": "1883",
                "precision": "year",
                "role": "statement_date",
                "source_posture": "catalog_supplied",
            },
            claim["object"]["temporal"],
        )
        self.assertIn("parts II and III", claim["object"]["activity_warning"])
        self.assertEqual("reported", claim["epistemic_status"])
        self.assertEqual("unreviewed", claim["review_status"])
        self.assertEqual("public_metadata_only", claim["visibility"])

        part_1 = json.loads(
            (ZARATHUSTRA_PART_1_EDITION_ROOT / "edition.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual([claim["claim_id"]], part_1["provision_activity_claim_refs"])
        expected_extensions = (
            (
                ZARATHUSTRA_PART_2_EDITION_ROOT,
                ZARATHUSTRA_PART_2_PROVISION_CLAIMS_PATH,
                "chemnitz-schmeitzner-1883-part-2",
                "1883",
                "exact part-II DTA source description",
            ),
            (
                ZARATHUSTRA_PART_3_EDITION_ROOT,
                ZARATHUSTRA_PART_3_PROVISION_CLAIMS_PATH,
                "chemnitz-schmeitzner-1884-part-3",
                "1884",
                "exact part-III DTA source description",
            ),
        )
        extension_claim_ids = set()
        for edition_root, claim_path, subject_suffix, year, warning in expected_extensions:
            edition = json.loads(
                (edition_root / "edition.json").read_text(encoding="utf-8")
            )
            extension_claim = json.loads(
                claim_path.read_text(encoding="utf-8").splitlines()[0]
            )
            extension_claim_ids.add(extension_claim["claim_id"])
            self.assertTrue(extension_claim["subject_ref"].endswith(subject_suffix))
            self.assertEqual(
                [extension_claim["claim_id"]],
                edition["provision_activity_claim_refs"],
            )
            self.assertEqual(
                f"Schmeitzner; Chemnitz; {year}",
                extension_claim["object"]["reported_statement"],
            )
            self.assertEqual(year, extension_claim["object"]["temporal"]["value"])
            self.assertIn(warning, extension_claim["object"]["activity_warning"])
            self.assertEqual("authority_record", extension_claim["object"]["statement_basis"])
            self.assertEqual("reported", extension_claim["epistemic_status"])
            self.assertEqual("unreviewed", extension_claim["review_status"])
            self.assertEqual("public_metadata_only", extension_claim["visibility"])
        self.assertEqual(2, len(extension_claim_ids))
        self.assertNotIn(claim["claim_id"], extension_claim_ids)

        chemnitz = json.loads(CHEMNITZ_PLACE_PATH.read_text(encoding="utf-8"))
        schmeitzner = json.loads(
            SCHMEITZNER_ORGANIZATION_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual("place", chemnitz["record_type"])
        self.assertEqual(
            [{"scheme": "GND", "value": "4029702-0", "source_ref": "https://d-nb.info/gnd/4029702-0", "status": "unverified"}],
            chemnitz["external_identifiers"],
        )
        self.assertEqual("organization", schmeitzner["record_type"])
        self.assertEqual("1063670306", schmeitzner["external_identifiers"][0]["value"])
        self.assertNotEqual("118823698", schmeitzner["external_identifiers"][0]["value"])
        self.assertIn("Person Ernst Schmeitzner, GND 118823698", schmeitzner["notes"])

        discovery = json.loads(
            ZARATHUSTRA_PART_1_PROVISION_DISCOVERY_PATH.read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual("reconciled", discovery["status"])
        self.assertTrue(discovery["general_web_search_is_last_resort"])
        self.assertEqual(
            "channel-general-web-zarathustra-provision-refresh",
            discovery["channels"][-1]["channel_id"],
        )
        self.assertIn(
            "tos-discovery-result.dnb-gnd-1063670306-schmeitzner-verlagsbuchhandlung",
            discovery["selected_result_ids"],
        )
        self.assertNotIn(
            "tos-discovery-result.dnb-gnd-118823698-ernst-schmeitzner-person",
            discovery["selected_result_ids"],
        )

        extension_discovery = json.loads(
            ZARATHUSTRA_PARTS_2_3_PROVISION_DISCOVERY_PATH.read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual("reconciled", extension_discovery["status"])
        self.assertTrue(extension_discovery["general_web_search_is_last_resort"])
        self.assertEqual(
            "channel-general-web-zarathustra-parts-2-3-provision-last",
            extension_discovery["channels"][-1]["channel_id"],
        )
        self.assertEqual(
            {
                "tos-discovery-result.dta-zarathustra-part-2-provision-statement",
                "tos-discovery-result.dta-zarathustra-part-3-provision-statement",
            },
            {
                result_id
                for result_id in extension_discovery["selected_result_ids"]
                if result_id.startswith("tos-discovery-result.dta-zarathustra-part-")
            },
        )
        self.assertIn(
            "tos-discovery-result.dnb-gnd-118823698-ernst-schmeitzner-person-negative",
            extension_discovery["rejected_result_ids"],
        )
        research = ZARATHUSTRA_PARTS_2_3_PROVISION_RESEARCH_PATH.read_text(
            encoding="utf-8"
        )
        self.assertIn("not from label propagation", research)
        self.assertIn("No physical-title-page transcription", research)

        part_4_edition = json.loads(
            (ZARATHUSTRA_PART_4_EDITION_ROOT / "edition.json").read_text(
                encoding="utf-8"
            )
        )
        part_4_claim = json.loads(
            ZARATHUSTRA_PART_4_PROVISION_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()[0]
        )
        self.assertEqual(
            "tos.edition.friedrich-nietzsche.also-sprach-zarathustra."
            "leipzig-naumann-1891-part-4",
            part_4_claim["subject_ref"],
        )
        self.assertEqual(
            [part_4_claim["claim_id"]],
            part_4_edition["provision_activity_claim_refs"],
        )
        self.assertEqual(
            "Naumann; Leipzig; 1891",
            part_4_claim["object"]["reported_statement"],
        )
        self.assertEqual(
            "tos.place.leipzig",
            part_4_claim["object"]["places"][0]["normalized_place_ref"],
        )
        self.assertEqual(
            "tos.organization.c-g-naumann-verlag-leipzig",
            part_4_claim["object"]["agents"][0]["normalized_agent_ref"],
        )
        self.assertEqual("1891", part_4_claim["object"]["temporal"]["value"])
        self.assertEqual("statement_date", part_4_claim["object"]["temporal"]["role"])
        self.assertIn("March-1892 actual delivery", part_4_claim["object"]["activity_warning"])
        self.assertNotIn(part_4_claim["claim_id"], extension_claim_ids)
        self.assertEqual("unreviewed", part_4_claim["review_status"])
        self.assertEqual("public_metadata_only", part_4_claim["visibility"])

        part_4_discovery = json.loads(
            ZARATHUSTRA_PART_4_PROVISION_DISCOVERY_PATH.read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual("reconciled", part_4_discovery["status"])
        self.assertEqual(
            [1, 2, 3, 4, 5, 6],
            [channel["sequence"] for channel in part_4_discovery["channels"]],
        )
        self.assertEqual(
            "channel-general-web-zarathustra-part-4-provision-last",
            part_4_discovery["channels"][-1]["channel_id"],
        )
        self.assertIn(
            "tos-discovery-result.dta-zarathustra-part-4-provision-statement",
            part_4_discovery["selected_result_ids"],
        )
        self.assertIn(
            "tos-discovery-result.dnb-gnd-16034133-4-naumann-printer-negative",
            part_4_discovery["rejected_result_ids"],
        )
        self.assertFalse(part_4_discovery["technical_access_bypass_used"])
        part_4_research = ZARATHUSTRA_PART_4_PROVISION_RESEARCH_PATH.read_text(
            encoding="utf-8"
        )
        for boundary in ("1885", "1890", "1891", "1892"):
            self.assertIn(boundary, part_4_research)
        self.assertIn("forty-five", part_4_research)
        self.assertIn("not inferred from parts I–III", part_4_research)

    def test_naumann_1893_provision_separates_publisher_and_printer(
        self,
    ) -> None:
        edition = json.loads(
            (NAUMANN_1893_EDITION_ROOT / "edition.json").read_text(
                encoding="utf-8"
            )
        )
        claims = [
            json.loads(line)
            for line in NAUMANN_1893_PROVISION_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
        ]
        self.assertEqual(2, len(claims))
        self.assertEqual(
            {claim["claim_id"] for claim in claims},
            set(edition["provision_activity_claim_refs"]),
        )
        self.assertEqual(
            "Zweite Auflage, mit Portrait und Brieffacsimile des Autors.",
            edition["edition_statement"],
        )

        by_kind = {claim["object"]["provision_kind"]: claim for claim in claims}
        self.assertEqual({"publication", "manufacture"}, set(by_kind))
        publication = by_kind["publication"]
        manufacture = by_kind["manufacture"]
        exact_statement = "LEIPZIG / Druck und Verlag von C. G. Naumann. / 1893."
        self.assertEqual(
            exact_statement, publication["object"]["transcribed_statement"]
        )
        self.assertEqual(
            exact_statement, manufacture["object"]["transcribed_statement"]
        )
        self.assertEqual(
            "tos.organization.c-g-naumann-verlag-leipzig",
            publication["object"]["agents"][0]["normalized_agent_ref"],
        )
        self.assertEqual(
            "tos.organization.druckerei-c-g-naumann-leipzig",
            manufacture["object"]["agents"][0]["normalized_agent_ref"],
        )
        self.assertEqual("publisher", publication["object"]["agents"][0]["role"])
        self.assertEqual("printer", manufacture["object"]["agents"][0]["role"])
        self.assertEqual(
            "publication_place", publication["object"]["places"][0]["role"]
        )
        self.assertEqual(
            "manufacture_place", manufacture["object"]["places"][0]["role"]
        )
        self.assertTrue(
            all(
                claim["object"]["places"][0]["normalized_place_ref"]
                == "tos.place.leipzig"
                and claim["object"]["temporal"]["value"] == "1893"
                and claim["object"]["temporal"]["role"] == "statement_date"
                and claim["review_status"] == "unreviewed"
                and claim["visibility"] == "public_metadata_only"
                for claim in claims
            )
        )

        anchors = [
            json.loads(line)
            for line in NAUMANN_1893_PROVISION_ANCHORS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
        ]
        self.assertEqual([46, 46, 47], [row["selectors"][0]["page"] for row in anchors])
        self.assertEqual(
            {"61c947e5aff76a64d82600cc52dcb25ff1b5862530d3a99c96824da885c1e6cf"},
            {row["file_sha256"] for row in anchors},
        )
        self.assertTrue(all(row["status"] == "proposed" for row in anchors))

        publisher = json.loads(NAUMANN_1893_PUBLISHER_PATH.read_text(encoding="utf-8"))
        printer = json.loads(NAUMANN_1893_PRINTER_PATH.read_text(encoding="utf-8"))
        self.assertNotEqual(publisher["record_id"], printer["record_id"])
        self.assertEqual("1072998033", publisher["external_identifiers"][0]["value"])
        self.assertEqual("16034133-4", printer["external_identifiers"][0]["value"])
        self.assertEqual("provisional", publisher["identity_status"])
        self.assertEqual("provisional", printer["identity_status"])
        self.assertEqual("no_equivalence_claim", publisher["same_as_posture"])
        self.assertEqual("no_equivalence_claim", printer["same_as_posture"])

        discovery = json.loads(
            NAUMANN_1893_PROVISION_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual([1, 2, 3, 4, 5], [row["sequence"] for row in discovery["channels"]])
        self.assertEqual(
            "channel-general-web-naumann-1893-last",
            discovery["channels"][-1]["channel_id"],
        )
        self.assertIn(
            "tos-discovery-result.dnb-gnd-16034133-4-naumann-printer",
            discovery["selected_result_ids"],
        )
        self.assertFalse(discovery["technical_access_bypass_used"])

        pdf_rights = json.loads(
            (
                NAUMANN_1893_EDITION_ROOT
                / "items/internet-archive-image-container-pdf/rights.json"
            ).read_text(encoding="utf-8")
        )
        epub_rights = json.loads(
            (
                NAUMANN_1893_EDITION_ROOT
                / "items/internet-archive-cornell-auto-epub/rights.json"
            ).read_text(encoding="utf-8")
        )
        self.assertEqual(
            "public_domain_reviewed", pdf_rights["assessment_status"]
        )
        self.assertEqual("conflicting_evidence", epub_rights["assessment_status"])
        for item_rights in (pdf_rights, epub_rights):
            self.assertEqual(["DE", "US"], item_rights["jurisdictions_reviewed"])
            self.assertEqual("local_only", item_rights["visibility"])
            self.assertEqual(
                "not_authorized", item_rights["redistribution_posture"]
            )
            self.assertEqual(
                "local_research_only", item_rights["derivative_posture"]
            )
            self.assertEqual("unreviewed", item_rights["review_status"])
            self.assertEqual(4, item_rights["record_version"])
            self.assertIn(
                "https://www.copyright.gov/title17/92chap1.html#104a",
                item_rights["source_refs"],
            )

        pdf_layers = {
            layer["layer_id"].rsplit(".layer.", 1)[1]: layer
            for layer in pdf_rights["layer_assessments"]
        }
        self.assertEqual(
            {
                "original-work",
                "peter-gast-preface",
                "edition-presentation",
                "portrait",
                "letter-facsimile",
                "digital-scan",
                "metadata",
            },
            set(pdf_layers),
        )
        for layer_name in set(pdf_layers) - {"metadata"}:
            self.assertEqual(
                "public_domain_reviewed",
                pdf_layers[layer_name]["assessment_status"],
            )
        self.assertEqual(
            "copyright_undetermined", pdf_layers["metadata"]["assessment_status"]
        )
        for layer_name in {
            "original-work",
            "peter-gast-preface",
            "edition-presentation",
            "portrait",
            "letter-facsimile",
        }:
            self.assertIn("§104A", pdf_layers[layer_name]["rationale"])

        epub_layers = {
            layer["layer_id"].rsplit(".layer.", 1)[1]: layer
            for layer in epub_rights["layer_assessments"]
        }
        self.assertEqual(
            "in_copyright",
            epub_layers["internet-archive-notice"]["assessment_status"],
        )
        self.assertEqual(
            "not_authorized",
            epub_layers["internet-archive-notice"]["redistribution_posture"],
        )
        self.assertEqual(
            "copyright_undetermined",
            epub_layers["package-navigation-style"]["assessment_status"],
        )
        for layer_name in {
            "original-work",
            "peter-gast-preface",
            "edition-presentation",
            "automatic-ocr-text",
        }:
            self.assertIn("§104A", epub_layers[layer_name]["rationale"])
        self.assertTrue(
            any(
                "Uebersetzungsrecht vorbehalten" in row
                for layer in pdf_rights["layer_assessments"]
                for row in layer["restrictions"]
            )
        )
        research = NAUMANN_1893_PROVISION_RESEARCH_PATH.read_text(encoding="utf-8")
        self.assertIn("one universal", research)
        self.assertIn("historical rights statement", research)
        rights_research = NAUMANN_1893_RIGHTS_RESEARCH_PATH.read_text(
            encoding="utf-8"
        )
        self.assertIn("domestic pre-1931", rights_research)
        self.assertIn("17 U.S.C. §104A", rights_research)
        self.assertIn("only a JavaScript shell", rights_research)

        pdf_plan = json.loads(
            (
                REPO_ROOT
                / "ToS/source-witnesses/server-import/plans/"
                "naumann-1893-image-pdf.server-import.json"
            ).read_text(encoding="utf-8")
        )
        epub_plan = json.loads(
            (
                REPO_ROOT
                / "ToS/source-witnesses/server-import/plans/"
                "naumann-1893-auto-epub.server-import.json"
            ).read_text(encoding="utf-8")
        )
        for plan, rights_path in (
            (
                pdf_plan,
                NAUMANN_1893_EDITION_ROOT
                / "items/internet-archive-image-container-pdf/rights.json",
            ),
            (
                epub_plan,
                NAUMANN_1893_EDITION_ROOT
                / "items/internet-archive-cornell-auto-epub/rights.json",
            ),
        ):
            self.assertEqual(3, plan["contract_version"])
            self.assertEqual("metadata-only", plan["publication_status"])
            self.assertFalse(plan["payload_transfer_authorized"])
            self.assertFalse(plan["operator_transfer_approval"]["approved"])
            self.assertEqual(
                hashlib.sha256(rights_path.read_bytes()).hexdigest(),
                plan["rights_policy"]["rights_record_sha256"],
            )
            self.assertIn(
                "https://www.copyright.gov/title17/92chap1.html#104a",
                plan["rights_policy"]["permission_or_license_refs"],
            )

    def test_jenseits_1886_provision_separates_publisher_and_printer(
        self,
    ) -> None:
        edition = json.loads(
            (JENSEITS_1886_EDITION_ROOT / "edition.json").read_text(
                encoding="utf-8"
            )
        )
        claims = [
            json.loads(line)
            for line in JENSEITS_1886_PROVISION_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
        ]
        self.assertEqual(2, len(claims))
        self.assertEqual(
            {claim["claim_id"] for claim in claims},
            set(edition["provision_activity_claim_refs"]),
        )
        self.assertIsNone(edition["edition_statement"])

        by_kind = {claim["object"]["provision_kind"]: claim for claim in claims}
        self.assertEqual({"publication", "manufacture"}, set(by_kind))
        publication = by_kind["publication"]
        manufacture = by_kind["manufacture"]
        exact_statement = "Leipzig / Druck und Verlag von C. G. Naumann. / 1886."
        self.assertEqual(
            exact_statement, publication["object"]["transcribed_statement"]
        )
        self.assertEqual(
            exact_statement, manufacture["object"]["transcribed_statement"]
        )
        self.assertEqual(
            {
                "role": "publisher",
                "literal_form": "C. G. Naumann",
                "normalized_agent_ref": (
                    "tos.organization.c-g-naumann-verlag-leipzig"
                ),
            },
            publication["object"]["agents"][0],
        )
        self.assertEqual(
            {
                "role": "printer",
                "literal_form": "C. G. Naumann",
                "normalized_agent_ref": (
                    "tos.organization.druckerei-c-g-naumann-leipzig"
                ),
            },
            manufacture["object"]["agents"][0],
        )
        self.assertEqual(
            "publication_place", publication["object"]["places"][0]["role"]
        )
        self.assertEqual(
            "manufacture_place", manufacture["object"]["places"][0]["role"]
        )
        self.assertTrue(
            all(
                claim["object"]["places"][0]["normalized_place_ref"]
                == "tos.place.leipzig"
                and claim["object"]["temporal"]["value"] == "1886"
                and claim["object"]["temporal"]["role"] == "statement_date"
                and claim["review_status"] == "unreviewed"
                and claim["visibility"] == "public_metadata_only"
                for claim in claims
            )
        )

        anchors = [
            json.loads(line)
            for line in JENSEITS_1886_PROVISION_ANCHORS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
        ]
        self.assertEqual([3, 4], [row["selectors"][0]["page"] for row in anchors])
        self.assertEqual(
            {"6ae316c90f958d09045fea27b2430b86623ebb85f8a27146099d028775cdc80a"},
            {row["file_sha256"] for row in anchors},
        )
        self.assertTrue(all(row["status"] == "proposed" for row in anchors))

        discovery = json.loads(
            JENSEITS_1886_PROVISION_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual(
            [1, 2, 3, 4, 5, 6],
            [row["sequence"] for row in discovery["channels"]],
        )
        self.assertEqual(
            "university-catalog", discovery["channels"][2]["channel_type"]
        )
        self.assertEqual(
            "channel-general-web-jenseits-1886-last",
            discovery["channels"][-1]["channel_id"],
        )
        self.assertIn(
            "tos-discovery-result.dnb-gnd-16034133-4-naumann-printer-jenseits",
            discovery["selected_result_ids"],
        )
        self.assertEqual(
            ["tos-discovery-result.abebooks-naumann-publisher-list-rejected"],
            discovery["rejected_result_ids"],
        )
        self.assertFalse(discovery["technical_access_bypass_used"])

        rights = json.loads(
            (JENSEITS_1886_ITEM_ROOT / "rights.json").read_text(encoding="utf-8")
        )
        self.assertEqual("conflicting_evidence", rights["assessment_status"])
        self.assertEqual(["DE", "US"], rights["jurisdictions_reviewed"])
        self.assertEqual("local_only", rights["visibility"])
        self.assertEqual("not_authorized", rights["redistribution_posture"])
        self.assertEqual("local_research_only", rights["derivative_posture"])
        self.assertEqual("unreviewed", rights["review_status"])
        self.assertEqual(4, rights["record_version"])
        self.assertTrue(
            any("Alle Rechte vorbehalten" in row for row in rights["restrictions"])
        )
        research = JENSEITS_1886_PROVISION_RESEARCH_PATH.read_text(
            encoding="utf-8"
        )
        self.assertIn("historical rights evidence only", research)
        self.assertIn("General web search ran last", research)

        layered_research = JENSEITS_1886_LAYERED_RIGHTS_RESEARCH_PATH.read_text(
            encoding="utf-8"
        )
        ordered_sections = [
            "## Classical and official documentation",
            "## Established scholarship, cases, and institutional practice",
            "## Fresh and currently relevant checks",
            "## General web search, last",
        ]
        positions = [layered_research.index(section) for section in ordered_sections]
        self.assertEqual(sorted(positions), positions)
        self.assertIn("zero matches", layered_research)
        self.assertIn("different physical volume", layered_research)
        self.assertIn("No restricted row", layered_research)

    def test_genealogie_1892_provision_separates_distant_source_surfaces(
        self,
    ) -> None:
        edition = json.loads(
            (GENEALOGIE_1892_EDITION_ROOT / "edition.json").read_text(
                encoding="utf-8"
            )
        )
        claims = [
            json.loads(line)
            for line in GENEALOGIE_1892_PROVISION_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
        ]
        self.assertEqual(2, len(claims))
        self.assertEqual(
            {claim["claim_id"] for claim in claims},
            set(edition["provision_activity_claim_refs"]),
        )
        self.assertEqual("Zweite Auflage", edition["edition_statement"])

        by_kind = {claim["object"]["provision_kind"]: claim for claim in claims}
        self.assertEqual({"publication", "manufacture"}, set(by_kind))
        publication = by_kind["publication"]
        manufacture = by_kind["manufacture"]
        self.assertEqual(
            "LEIPZIG / Verlag von C. G. Naumann. / 1892.",
            publication["object"]["transcribed_statement"],
        )
        self.assertEqual(
            "LEIPZIG / Druck von C. G. Naumann.",
            manufacture["object"]["transcribed_statement"],
        )
        self.assertEqual(
            "tos.organization.c-g-naumann-verlag-leipzig",
            publication["object"]["agents"][0]["normalized_agent_ref"],
        )
        self.assertEqual("publisher", publication["object"]["agents"][0]["role"])
        self.assertEqual(
            "tos.organization.druckerei-c-g-naumann-leipzig",
            manufacture["object"]["agents"][0]["normalized_agent_ref"],
        )
        self.assertEqual("printer", manufacture["object"]["agents"][0]["role"])
        self.assertIn("printer line is undated", manufacture["object"]["activity_warning"])
        self.assertTrue(
            all(
                claim["object"]["places"][0]["normalized_place_ref"]
                == "tos.place.leipzig"
                and claim["object"]["temporal"]["value"] == "1892"
                and claim["object"]["temporal"]["role"] == "statement_date"
                and claim["review_status"] == "unreviewed"
                for claim in claims
            )
        )

        anchors = [
            json.loads(line)
            for line in GENEALOGIE_1892_PROVISION_ANCHORS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
        ]
        self.assertEqual([5, 204], [row["selectors"][0]["page"] for row in anchors])
        self.assertEqual(
            {"5705dbc4f32faa924919fd533962c931e92462d72dab5183610eb68adeecac03"},
            {row["file_sha256"] for row in anchors},
        )
        self.assertTrue(all(row["status"] == "proposed" for row in anchors))

        discovery = json.loads(
            GENEALOGIE_1892_PROVISION_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual(
            [1, 2, 3, 4, 5, 6],
            [row["sequence"] for row in discovery["channels"]],
        )
        self.assertEqual(
            "channel-general-web-genealogie-1892-last",
            discovery["channels"][-1]["channel_id"],
        )
        self.assertIn(
            "tos-discovery-result.dnb-gnd-16034133-4-naumann-printer-genealogie",
            discovery["selected_result_ids"],
        )
        self.assertIn(
            "tos-discovery-result.hackett-2026-genealogy-translation-edition-rejected",
            discovery["rejected_result_ids"],
        )
        self.assertFalse(discovery["technical_access_bypass_used"])

        rights = json.loads(
            (GENEALOGIE_1892_ITEM_ROOT / "rights.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual("conflicting_evidence", rights["assessment_status"])
        self.assertEqual("local_only", rights["visibility"])
        self.assertEqual("not_authorized", rights["redistribution_posture"])
        research = GENEALOGIE_1892_PROVISION_RESEARCH_PATH.read_text(
            encoding="utf-8"
        )
        self.assertIn("printer line itself is undated", research)
        self.assertIn("General web search ran last", research)

    def test_antonovsky_1913_provision_separates_publisher_and_printer(
        self,
    ) -> None:
        edition = json.loads(
            (ANTONOVSKY_1913_EDITION_ROOT / "edition.json").read_text(
                encoding="utf-8"
            )
        )
        claims = [
            json.loads(line)
            for line in ANTONOVSKY_1913_PROVISION_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
        ]
        self.assertEqual(2, len(claims))
        self.assertEqual(
            {claim["claim_id"] for claim in claims},
            set(edition["provision_activity_claim_refs"]),
        )
        self.assertEqual(
            "Так говорил Заратустра, издание «Жизнь для всех», "
            "Санкт-Петербург, 1913",
            edition["preferred_label"],
        )

        by_kind = {claim["object"]["provision_kind"]: claim for claim in claims}
        self.assertEqual({"publication", "manufacture"}, set(by_kind))
        publication = by_kind["publication"]
        manufacture = by_kind["manufacture"]
        self.assertEqual("manifestation_transcription", publication["object"]["statement_basis"])
        self.assertEqual("item_observation", manufacture["object"]["statement_basis"])
        self.assertEqual(
            "ИЗДАНІЕ «ЖИЗНЬ ДЛЯ ВСѢХЪ». С. ПЕТЕРБУРГЪ. 1913.",
            publication["object"]["transcribed_statement"],
        )
        self.assertEqual(
            "Типографія Бр. В. и И. Линникъ, Гончарная, 7. / "
            "С. ПЕТЕРБУРГЪ. 1913.",
            manufacture["object"]["transcribed_statement"],
        )
        self.assertEqual(
            {
                "role": "publisher",
                "literal_form": "«ЖИЗНЬ ДЛЯ ВСѢХЪ»",
                "normalized_agent_ref":
                    "tos.organization.zhizn-dlya-vsekh-saint-petersburg",
            },
            publication["object"]["agents"][0],
        )
        self.assertEqual(
            {
                "role": "printer",
                "literal_form": "Типографія Бр. В. и И. Линникъ",
                "normalized_agent_ref":
                    "tos.organization.bratya-v-i-i-linnik-printing-saint-petersburg",
            },
            manufacture["object"]["agents"][0],
        )
        self.assertEqual(
            "tos.place.saint-petersburg",
            publication["object"]["places"][0]["normalized_place_ref"],
        )
        self.assertEqual(
            "tos.place.saint-petersburg",
            manufacture["object"]["places"][0]["normalized_place_ref"],
        )
        self.assertEqual("publication_place", publication["object"]["places"][0]["role"])
        self.assertEqual("manufacture_place", manufacture["object"]["places"][0]["role"])
        self.assertIn("title page alone does not name a publisher", publication["object"]["activity_warning"])
        self.assertIn("does not repeat the city or year", manufacture["object"]["activity_warning"])
        self.assertTrue(
            all(
                claim["epistemic_status"] == "observed"
                and claim["review_status"] == "unreviewed"
                and claim["visibility"] == "public_metadata_only"
                for claim in claims
            )
        )

        anchors = [
            json.loads(line)
            for line in ANTONOVSKY_1913_PROVISION_ANCHORS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
        ]
        self.assertEqual([5, 7, 8], [anchor["selectors"][0]["page"] for anchor in anchors])
        self.assertEqual(
            {"687716bc25ebf2281b967ebb0c6cf16b043c2d40bd16833d57d6dcf260d3476b"},
            {anchor["file_sha256"] for anchor in anchors},
        )
        self.assertTrue(all(anchor["status"] == "proposed" for anchor in anchors))

        place = json.loads(SAINT_PETERSBURG_PLACE_PATH.read_text(encoding="utf-8"))
        publisher = json.loads(
            ZHIZN_DLYA_VSEKH_ORGANIZATION_PATH.read_text(encoding="utf-8")
        )
        printer = json.loads(
            LINNIK_PRINTING_ORGANIZATION_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual("4267026-3", place["external_identifiers"][0]["value"])
        self.assertNotEqual(publisher["record_id"], printer["record_id"])
        self.assertEqual([], publisher["external_identifiers"])
        self.assertEqual([], printer["external_identifiers"])
        self.assertNotIn("Vladimir Posse", json.dumps(claims, ensure_ascii=False))

        discovery = json.loads(
            ANTONOVSKY_1913_PROVISION_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual([1, 2, 3, 4, 5, 6], [channel["sequence"] for channel in discovery["channels"]])
        self.assertEqual(
            "channel-general-web-antonovsky-provision-last",
            discovery["channels"][-1]["channel_id"],
        )
        self.assertIn(
            "tos-discovery-result.prlib-964713-posse-person-negative",
            discovery["rejected_result_ids"],
        )
        self.assertFalse(discovery["technical_access_bypass_used"])
        research = ANTONOVSKY_1913_PROVISION_RESEARCH_PATH.read_text(encoding="utf-8")
        for exact_form in (
            "ИЗДАНІЕ «ЖИЗНЬ ДЛЯ ВСѢХЪ».",
            "С. ПЕТЕРБУРГЪ.",
            "Типографія Бр. В. и И. Линникъ",
        ):
            self.assertIn(exact_form, research)

    def test_antonovsky_1913_positive_rights_do_not_open_local_payload(
        self,
    ) -> None:
        manifest = json.loads(
            (ANTONOVSKY_1913_ITEM_ROOT / "item.manifest.json").read_text(
                encoding="utf-8"
            )
        )
        rights = json.loads(
            (ANTONOVSKY_1913_ITEM_ROOT / "rights.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual("local_gitignored_payload", manifest["storage_posture"])
        self.assertEqual("local_only", manifest["visibility"])
        self.assertEqual(
            "687716bc25ebf2281b967ebb0c6cf16b043c2d40bd16833d57d6dcf260d3476b",
            manifest["payload_files"][0]["sha256"],
        )

        self.assertEqual("public_domain_reviewed", rights["assessment_status"])
        self.assertEqual(["RU", "US"], rights["jurisdictions_reviewed"])
        self.assertEqual("local_only", rights["visibility"])
        self.assertEqual("not_authorized", rights["redistribution_posture"])
        self.assertEqual("local_research_only", rights["derivative_posture"])
        self.assertEqual("unreviewed", rights["review_status"])
        self.assertEqual(4, rights["record_version"])

        layers_by_role = {
            layer["layer_role"]: layer
            for layer in rights["layer_assessments"]
        }
        self.assertEqual(
            {
                "original_work",
                "translation",
                "preface",
                "edition_presentation",
                "digital_scan",
                "embedded_text",
            },
            set(layers_by_role),
        )
        for layer in layers_by_role.values():
            self.assertEqual(
                "public_domain_reviewed",
                layer["assessment_status"],
            )
            self.assertEqual(["RU", "US"], layer["jurisdictions_reviewed"])
            self.assertEqual("unreviewed", layer["review_status"])
        for role in ("original_work", "translation", "preface"):
            self.assertEqual(
                "expired",
                layers_by_role[role]["term"]["calculation_status"],
            )
            self.assertIn(
                "§104A",
                " ".join(
                    (
                        layers_by_role[role]["term"]["basis"],
                        layers_by_role[role]["rationale"],
                    )
                ),
            )
        original_work = layers_by_role["original_work"]
        self.assertEqual("1970-12-31", original_work["term"]["ends_on"])
        self.assertIn("§104A(h)(8)(C)(i)", original_work["term"]["basis"])
        self.assertIn("Germany", original_work["term"]["basis"])
        self.assertIn("1950", original_work["term"]["basis"])
        self.assertIn(
            "https://www.copyright.gov/circs/circ38b.pdf",
            original_work["source_refs"],
        )
        self.assertEqual(
            "1983-12-31",
            layers_by_role["edition_presentation"]["term"]["ends_on"],
        )
        for role in ("digital_scan", "embedded_text"):
            self.assertEqual(
                "not_applicable",
                layers_by_role[role]["term"]["calculation_status"],
            )
            self.assertEqual(
                "local_research_only",
                layers_by_role[role]["server_processing_posture"],
            )
        self.assertIn(
            "not evidence of OCR correctness",
            layers_by_role["embedded_text"]["restrictions"][0],
        )

        server_plan = json.loads(
            ANTONOVSKY_1913_SERVER_PLAN_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual(
            "public-domain-reviewed",
            server_plan["rights_policy"]["assessment_status"],
        )
        self.assertEqual(
            "unreviewed",
            server_plan["rights_policy"]["review_status"],
        )
        self.assertEqual(
            ["RU", "US"],
            server_plan["rights_policy"]["jurisdictions_reviewed"],
        )
        self.assertEqual("metadata-only", server_plan["access_class"])
        self.assertEqual("blocked-rights", server_plan["server_import_status"])
        self.assertEqual("metadata-only", server_plan["publication_status"])
        self.assertFalse(server_plan["payload_transfer_authorized"])
        self.assertFalse(server_plan["operator_transfer_approval"]["approved"])
        self.assertEqual(4, server_plan["contract_version"])
        self.assertEqual(
            "4e7bb76702557d5f7379f186959dbfd03c9318695d3008ab8529197e83707c5b",
            server_plan["rights_policy"]["rights_record_sha256"],
        )
        self.assertEqual(
            [
                "tos.event.server-import-plan.antonovsky-1913-wikimedia-commons."
                "source-country-endpoint-correction.2026-08-10"
            ],
            server_plan["provenance_event_refs"],
        )
        self.assertTrue(
            all(
                row["state"] == "prohibited"
                for key, row in server_plan["allowed_derivatives"].items()
                if key not in {"lexical_index", "search_projection", "graph_projection"}
            )
        )

        research = ANTONOVSKY_1913_RIGHTS_RESEARCH_PATH.read_text(
            encoding="utf-8"
        )
        self.assertIn("Classical and official documentation", research)
        self.assertIn("Established scholarship and practice", research)
        self.assertIn("Fresh and currently relevant checks", research)
        self.assertIn("General web search, last", research)
        self.assertIn("Rights status, source-text quality", research)
        self.assertIn("separate Wikisource transcription", research)
        self.assertIn("no operator transfer approval", research)
        self.assertIn("17 U.S.C. §104A", research)
        self.assertIn("cannot rest on the domestic pre-1931 shorthand", research)
        self.assertIn(
            "https://www.copyright.gov/title17/92chap1.html",
            server_plan["rights_policy"]["permission_or_license_refs"],
        )
        item_events = [
            json.loads(line)
            for line in (ANTONOVSKY_1913_ITEM_ROOT / "provenance.jsonl")
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
        ]
        item_event = item_events[-1]
        self.assertEqual(4, item_event["event_version"])
        self.assertEqual(
            "17-usc-104a-h-8-c-i-first-publication-country",
            item_event["method"]["configuration"]["source_country_definition"],
        )
        self.assertEqual(
            "1970-12-31",
            item_event["method"]["configuration"][
                "original_work_source_country_term_ends_on"
            ],
        )
        self.assertFalse(
            item_event["method"]["configuration"]["aggregate_assessment_status_changed"]
        )
        server_events = [
            json.loads(line)
            for line in (
                REPO_ROOT / "ToS/source-witnesses/server-import/provenance.jsonl"
            )
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
        ]
        server_event = next(
            event
            for event in reversed(server_events)
            if event["event_id"] == server_plan["provenance_event_refs"][0]
        )
        self.assertEqual(4, server_event["event_version"])
        self.assertEqual(
            "17-usc-104a-h-8-c-i-first-publication-country",
            server_event["method"]["configuration"]["source_country_definition"],
        )
        self.assertFalse(
            server_event["method"]["configuration"]["aggregate_rights_status_changed"]
        )

    def test_antonovsky_1911_is_exact_local_revision_lineage_witness(
        self,
    ) -> None:
        expression = json.loads(
            (ANTONOVSKY_1911_EXPRESSION_ROOT / "expression.json").read_text(
                encoding="utf-8"
            )
        )
        edition = json.loads(
            (ANTONOVSKY_1911_EDITION_ROOT / "edition.json").read_text(
                encoding="utf-8"
            )
        )
        manifest = json.loads(
            (ANTONOVSKY_1911_ITEM_ROOT / "item.manifest.json").read_text(
                encoding="utf-8"
            )
        )
        rights = json.loads(
            (ANTONOVSKY_1911_ITEM_ROOT / "rights.json").read_text(
                encoding="utf-8"
            )
        )
        inventory = json.loads(
            (ANTONOVSKY_1911_ITEM_ROOT / "resource-inventory.json").read_text(
                encoding="utf-8"
            )
        )

        self.assertEqual("verified", expression["identity_status"])
        self.assertEqual("no_equivalence_claim", expression["same_as_posture"])
        self.assertEqual("4-е изд.", edition["edition_statement"])
        self.assertEqual(
            "tos.item.friedrich-nietzsche.also-sprach-zarathustra."
            "ru-antonovsky-1911.rsl-neb-scan-pdf",
            manifest["item_id"],
        )
        payload = manifest["payload_files"][0]
        self.assertEqual(62952283, payload["byte_size"])
        self.assertEqual(
            "57518b50e24fee37b3e9c151e853c36ad34258f51a8b65b8233742d69017db69",
            payload["sha256"],
        )
        self.assertEqual(153, inventory["files"][0]["summary"]["page_count"])
        self.assertFalse(inventory["source_text_included"])
        self.assertEqual("copyright_undetermined", rights["assessment_status"])
        self.assertEqual(["RU", "US"], rights["jurisdictions_reviewed"])
        self.assertEqual("local_only", rights["visibility"])
        self.assertEqual("not_authorized", rights["redistribution_posture"])
        self.assertEqual("local_research_only", rights["derivative_posture"])
        self.assertIsNone(rights["rights_statement_uri"])
        self.assertEqual("unreviewed", rights["review_status"])
        self.assertEqual(3, rights["record_version"])

        layers_by_role = {
            layer["layer_role"]: layer
            for layer in rights["layer_assessments"]
        }
        self.assertEqual(
            {
                "original_work",
                "translation",
                "preface",
                "edition_presentation",
                "digital_scan",
                "embedded_text",
            },
            set(layers_by_role),
        )
        for role in (
            "original_work",
            "translation",
            "preface",
            "edition_presentation",
        ):
            self.assertEqual(
                "public_domain_reviewed",
                layers_by_role[role]["assessment_status"],
            )
            self.assertEqual(
                "authorized_with_conditions",
                layers_by_role[role]["redistribution_posture"],
            )
        for role in ("digital_scan", "embedded_text"):
            self.assertEqual(
                "copyright_undetermined",
                layers_by_role[role]["assessment_status"],
            )
            self.assertEqual(
                "not_authorized",
                layers_by_role[role]["redistribution_posture"],
            )
            self.assertEqual(
                "local_research_only",
                layers_by_role[role]["server_processing_posture"],
            )
        self.assertEqual(
            "1970-12-31",
            layers_by_role["original_work"]["term"]["ends_on"],
        )
        original_work = layers_by_role["original_work"]
        self.assertIn("§104A(h)(8)(C)(i)", original_work["term"]["basis"])
        self.assertIn("Germany", original_work["term"]["basis"])
        self.assertIn("1950", original_work["term"]["basis"])
        self.assertIn(
            "https://www.copyright.gov/circs/circ38b.pdf",
            original_work["source_refs"],
        )
        for role in ("translation", "preface"):
            self.assertEqual(
                "1963-12-31",
                layers_by_role[role]["term"]["ends_on"],
            )
        self.assertEqual(
            "1981-12-31",
            layers_by_role["edition_presentation"]["term"]["ends_on"],
        )

        responsibility = json.loads(
            ANTONOVSKY_1911_RESPONSIBILITY_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()[0]
        )
        self.assertEqual("translated_by", responsibility["predicate"])
        self.assertEqual("tos.agent.yuri-antonovsky", responsibility["object"])
        self.assertEqual(
            [responsibility["claim_id"]],
            expression["responsibility_claim_refs"],
        )

        provision_claims = [
            json.loads(line)
            for line in ANTONOVSKY_1911_PROVISION_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
        ]
        by_kind = {
            claim["object"]["provision_kind"]: claim
            for claim in provision_claims
        }
        self.assertEqual({"publication", "manufacture"}, set(by_kind))
        self.assertEqual(
            "tos.organization.prometey-publishing-saint-petersburg",
            by_kind["publication"]["object"]["agents"][0][
                "normalized_agent_ref"
            ],
        )
        self.assertEqual(
            "tos.organization.energiya-typolithography-saint-petersburg",
            by_kind["manufacture"]["object"]["agents"][0][
                "normalized_agent_ref"
            ],
        )
        self.assertIn(
            "printer line is undated",
            by_kind["manufacture"]["object"]["activity_warning"],
        )

        anchors = [
            json.loads(line)
            for line in ANTONOVSKY_1911_SOURCE_ANCHORS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
        ]
        self.assertEqual([1, 2, 4], [row["selectors"][0]["page"] for row in anchors])
        self.assertEqual(
            {payload["sha256"]},
            {row["file_sha256"] for row in anchors},
        )
        self.assertTrue(all(row["status"] == "proposed" for row in anchors))

        discovery = json.loads(
            ANTONOVSKY_1911_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual(
            [1, 2, 3, 4],
            [channel["sequence"] for channel in discovery["channels"]],
        )
        self.assertEqual(
            "channel-general-web-antonovsky-1911-last",
            discovery["channels"][-1]["channel_id"],
        )
        self.assertTrue(discovery["channels"][0]["results"][0]["acquisition"]["downloaded"])
        self.assertIn(
            "tos-discovery-result.azbuka-2026-yuri-antonovsky-negative",
            discovery["rejected_result_ids"],
        )
        self.assertFalse(discovery["technical_access_bypass_used"])

        server_plan = json.loads(
            ANTONOVSKY_1911_SERVER_PLAN_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual("metadata-only", server_plan["access_class"])
        self.assertEqual("blocked-rights", server_plan["server_import_status"])
        self.assertEqual(
            "rights-unknown",
            server_plan["rights_policy"]["assessment_status"],
        )
        self.assertEqual(
            ["RU", "US"],
            server_plan["rights_policy"]["jurisdictions_reviewed"],
        )
        self.assertFalse(server_plan["payload_transfer_authorized"])
        self.assertFalse(server_plan["operator_transfer_approval"]["approved"])
        self.assertEqual(3, server_plan["contract_version"])
        self.assertEqual(
            "ebf9a2e82f2dc75a43c73df998d3addc36564cdc1bcd3279630e8fc5404768a4",
            server_plan["rights_policy"]["rights_record_sha256"],
        )
        self.assertEqual(
            [
                "tos.event.server-import-plan.antonovsky-prometey-1911-rsl-neb-"
                "scan-pdf.source-country-endpoint-correction.2026-08-10"
            ],
            server_plan["provenance_event_refs"],
        )

        register = json.loads(
            (GOLD_ROOT / "translation-reference-register.v1.json").read_text(
                encoding="utf-8"
            )
        )
        entry = next(
            row
            for row in register["entries"]
            if row["reference_id"] == "tos-ref.ru.antonovsky-prometey-1911"
        )
        self.assertEqual("local-item-registered", entry["access"]["acquisition_state"])
        self.assertFalse(entry["access"]["content_ingested_for_translation_lab"])
        self.assertFalse(entry["admission"]["accepted_as_truth"])
        research_text = ANTONOVSKY_1911_RESEARCH_PATH.read_text(encoding="utf-8")
        self.assertIn("three previous editions", research_text)
        self.assertIn("exact direct derivation of 1911 remains", research_text)
        rights_research = ANTONOVSKY_1911_RIGHTS_RESEARCH_PATH.read_text(
            encoding="utf-8"
        )
        self.assertIn("Classical and official documentation", rights_research)
        self.assertIn("Established scholarship and practice", rights_research)
        self.assertIn("Fresh and currently relevant checks", rights_research)
        self.assertIn("General web search, last", rights_research)
        self.assertIn("17 U.S.C. §104A", rights_research)
        self.assertIn("public-domain text", rights_research)
        self.assertIn("publishable PDF", rights_research)
        item_events = [
            json.loads(line)
            for line in (ANTONOVSKY_1911_ITEM_ROOT / "provenance.jsonl")
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
        ]
        item_event = item_events[-1]
        self.assertEqual(3, item_event["event_version"])
        self.assertEqual(
            "17-usc-104a-h-8-c-i-first-publication-country",
            item_event["method"]["configuration"]["source_country_definition"],
        )
        self.assertEqual(
            "1970-12-31",
            item_event["method"]["configuration"][
                "original_work_source_country_term_ends_on"
            ],
        )
        self.assertFalse(
            item_event["method"]["configuration"]["aggregate_assessment_status_changed"]
        )
        server_events = [
            json.loads(line)
            for line in (
                REPO_ROOT / "ToS/source-witnesses/server-import/provenance.jsonl"
            )
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
        ]
        server_event = next(
            event
            for event in reversed(server_events)
            if event["event_id"] == server_plan["provenance_event_refs"][0]
        )
        self.assertEqual(3, server_event["event_version"])
        self.assertEqual(
            "17-usc-104a-h-8-c-i-first-publication-country",
            server_event["method"]["configuration"]["source_country_definition"],
        )
        self.assertFalse(
            server_event["method"]["configuration"]["aggregate_rights_status_changed"]
        )

    def test_reader_1899_is_an_exact_but_fragmentary_uncredited_witness(
        self,
    ) -> None:
        expression = json.loads(
            (READER_1899_EXPRESSION_ROOT / "expression.json").read_text(
                encoding="utf-8"
            )
        )
        edition = json.loads(
            (READER_1899_EDITION_ROOT / "edition.json").read_text(
                encoding="utf-8"
            )
        )
        item = json.loads(
            (READER_1899_ITEM_ROOT / "item.json").read_text(encoding="utf-8")
        )
        manifest = json.loads(
            (READER_1899_ITEM_ROOT / "item.manifest.json").read_text(
                encoding="utf-8"
            )
        )
        inventory = json.loads(
            (READER_1899_ITEM_ROOT / "resource-inventory.json").read_text(
                encoding="utf-8"
            )
        )
        rights = json.loads(
            (READER_1899_ITEM_ROOT / "rights.json").read_text(encoding="utf-8")
        )

        self.assertEqual("provisional", expression["identity_status"])
        self.assertEqual("translation", expression["expression_role"])
        self.assertEqual([], expression["responsibility_claim_refs"])
        self.assertEqual([], expression["derivation_claim_refs"])
        self.assertEqual("no_equivalence_claim", expression["same_as_posture"])
        self.assertEqual("verified", edition["identity_status"])
        self.assertIsNone(edition["edition_statement"])
        self.assertEqual(2, len(edition["provision_activity_claim_refs"]))
        self.assertEqual(1, len(edition["exemplar_claim_refs"]))
        self.assertIn("fragmentary", item["notes"])

        expected_files = [
            (
                155778671,
                "f27a4c1bc95d6452ebe8f13358ded0e0a3012031481285e0239bccb7bfb86b4d",
            ),
            (
                167408539,
                "3b760f94356ecd938e7356b6115c055a8ba38dee029c379166bf9867afc8f818",
            ),
            (
                206863786,
                "d3ce8e66b1f11da99a74362ae2014d73871fe7bb37189b0ea3bf7ff0ed4cef3d",
            ),
        ]
        self.assertEqual(
            expected_files,
            [
                (row["byte_size"], row["sha256"])
                for row in manifest["payload_files"]
            ],
        )
        self.assertEqual("local_only", manifest["visibility"])
        self.assertFalse(inventory["source_text_included"])
        self.assertEqual(
            [7, 7, 8],
            [row["summary"]["page_count"] for row in inventory["files"]],
        )
        self.assertEqual(
            22,
            sum(row["summary"]["resource_count"] for row in inventory["files"]),
        )
        self.assertEqual("copyright_undetermined", rights["assessment_status"])
        self.assertEqual("local_only", rights["visibility"])
        self.assertEqual("not_authorized", rights["redistribution_posture"])
        self.assertEqual("local_research_only", rights["derivative_posture"])
        self.assertEqual(["RU"], rights["jurisdictions_reviewed"])
        self.assertEqual(2, rights["record_version"])
        layers_by_role = {
            layer["layer_role"]: layer
            for layer in rights["layer_assessments"]
        }
        self.assertEqual(
            {
                "original_work",
                "translation",
                "edition_presentation",
                "digital_scan",
                "other",
            },
            set(layers_by_role),
        )
        for role in ("original_work", "translation"):
            self.assertEqual(
                "public_domain_reviewed",
                layers_by_role[role]["assessment_status"],
            )
            self.assertEqual(
                "authorized_with_conditions",
                layers_by_role[role]["redistribution_posture"],
            )
            self.assertEqual(
                ["RU"],
                layers_by_role[role]["jurisdictions_reviewed"],
            )
        self.assertEqual(
            "operator_statement",
            layers_by_role["translation"]["assessment_basis"],
        )
        for role in ("edition_presentation", "digital_scan", "other"):
            self.assertEqual(
                "copyright_undetermined",
                layers_by_role[role]["assessment_status"],
            )
            self.assertEqual(
                "not_authorized",
                layers_by_role[role]["redistribution_posture"],
            )
            self.assertEqual(
                "local_research_only",
                layers_by_role[role]["server_processing_posture"],
            )

        provision_claims = [
            json.loads(line)
            for line in READER_1899_PROVISION_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
        ]
        by_kind = {
            claim["object"]["provision_kind"]: claim
            for claim in provision_claims
        }
        self.assertEqual({"publication", "distribution"}, set(by_kind))
        self.assertEqual(
            "tos.organization.reader-journal-editorial-office-moscow",
            by_kind["publication"]["object"]["agents"][0][
                "normalized_agent_ref"
            ],
        )
        distributor = by_kind["distribution"]["object"]["agents"][0]
        self.assertEqual("Д. П. Ефимова", distributor["literal_form"])
        self.assertNotIn("normalized_agent_ref", distributor)

        anchors = [
            json.loads(line)
            for line in READER_1899_SOURCE_ANCHORS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
        ]
        self.assertEqual(1, len(anchors))
        self.assertEqual(1, anchors[0]["selectors"][0]["page"])
        self.assertEqual("proposed", anchors[0]["status"])
        self.assertEqual(expected_files[0][1], anchors[0]["file_sha256"])

        discovery = json.loads(READER_1899_DISCOVERY_PATH.read_text(encoding="utf-8"))
        self.assertEqual("incomplete", discovery["status"])
        self.assertEqual(
            [1, 2, 3, 4],
            [channel["sequence"] for channel in discovery["channels"]],
        )
        self.assertEqual(
            "general-web-search", discovery["channels"][-1]["channel_type"]
        )
        self.assertTrue(
            discovery["channels"][0]["results"][0]["acquisition"]["downloaded"]
        )
        self.assertFalse(discovery["technical_access_bypass_used"])

        request = json.loads(READER_1899_REQUEST_PATH.read_text(encoding="utf-8"))
        self.assertEqual("draft-not-sent", request["request_status"])
        self.assertFalse(request["human_send_approval"])
        self.assertIsNone(request["sent_at"])
        self.assertEqual("none", request["response"]["state"])
        for permission in ("ocr_or_transcription", "indexing", "embeddings"):
            self.assertEqual("requested", request["requested_permissions"][permission])
        for permission in (
            "source_redistribution",
            "derivative_publication",
            "server_processing",
        ):
            self.assertEqual(
                "not-requested", request["requested_permissions"][permission]
            )

        server_plan = json.loads(
            READER_1899_SERVER_PLAN_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual("metadata-only", server_plan["access_class"])
        self.assertEqual("blocked-rights", server_plan["server_import_status"])
        self.assertFalse(server_plan["payload_transfer_authorized"])
        self.assertEqual(
            ["RU"],
            server_plan["rights_policy"]["jurisdictions_reviewed"],
        )
        self.assertEqual(2, server_plan["contract_version"])
        self.assertTrue(
            all(
                row["state"] == "prohibited"
                for key, row in server_plan["allowed_derivatives"].items()
                if key not in {"lexical_index", "search_projection", "graph_projection"}
            )
        )

        research = READER_1899_RESEARCH_PATH.read_text(encoding="utf-8")
        self.assertIn("The digital object is not a complete book.", research)
        self.assertIn("does not name a translator", research)
        self.assertIn("cataloged 236-page object is complete online", research)
        rights_research = READER_1899_RIGHTS_RESEARCH_PATH.read_text(
            encoding="utf-8"
        )
        self.assertIn("cannot be inherited from Nani", rights_research)
        self.assertIn("General web search, last", rights_research)
        self.assertIn("public_domain_reviewed", rights_research)
        self.assertIn("digital fragment selection", rights_research)

    def test_nani_1899_is_exact_parallel_local_soil_without_text_promotion(
        self,
    ) -> None:
        expression = json.loads(
            (NANI_1899_EXPRESSION_ROOT / "expression.json").read_text(
                encoding="utf-8"
            )
        )
        edition = json.loads(
            (NANI_1899_EDITION_ROOT / "edition.json").read_text(
                encoding="utf-8"
            )
        )
        item = json.loads(
            (NANI_1899_ITEM_ROOT / "item.json").read_text(encoding="utf-8")
        )
        manifest = json.loads(
            (NANI_1899_ITEM_ROOT / "item.manifest.json").read_text(
                encoding="utf-8"
            )
        )
        inventory = json.loads(
            (NANI_1899_ITEM_ROOT / "resource-inventory.json").read_text(
                encoding="utf-8"
            )
        )
        rights = json.loads(
            (NANI_1899_ITEM_ROOT / "rights.json").read_text(encoding="utf-8")
        )
        snapshot = json.loads(
            (NANI_1899_ITEM_ROOT / "source-metadata-snapshot.json").read_text(
                encoding="utf-8"
            )
        )
        agent = json.loads(NANI_AGENT_PATH.read_text(encoding="utf-8"))
        printer = json.loads(
            STASYULEVICH_PRINTING_ORGANIZATION_PATH.read_text(encoding="utf-8")
        )

        self.assertEqual("verified", expression["identity_status"])
        self.assertEqual("ru", expression["language"])
        self.assertEqual("translation", expression["expression_role"])
        self.assertEqual([], expression["derivation_claim_refs"])
        self.assertEqual(1, len(expression["responsibility_claim_refs"]))
        self.assertEqual(1, len(expression["embodiment_claim_refs"]))

        self.assertEqual("verified", edition["identity_status"])
        self.assertIsNone(edition["edition_statement"])
        self.assertEqual([], edition["publication_claim_refs"])
        self.assertEqual(1, len(edition["provision_activity_claim_refs"]))
        self.assertEqual(1, len(edition["exemplar_claim_refs"]))
        self.assertIn("XIV, [2], 105", edition["notes"])
        self.assertIn("103", edition["notes"])

        self.assertEqual("verified", item["identity_status"])
        self.assertEqual("local_gitignored_payload", manifest["storage_posture"])
        self.assertEqual("local_only", manifest["visibility"])
        self.assertEqual(1, len(manifest["payload_files"]))
        payload = manifest["payload_files"][0]
        self.assertEqual(16228737, payload["byte_size"])
        self.assertEqual(
            "044be8c36536c751d1f846109b4e99534323138b7ad829e6266ef3150d1e5704",
            payload["sha256"],
        )
        self.assertFalse(payload["container_member"])

        summary = inventory["files"][0]["summary"]
        self.assertFalse(inventory["source_text_included"])
        self.assertEqual(60, summary["page_count"])
        self.assertEqual(96, summary["image_resource_count"])
        self.assertEqual(38, summary["distinct_page_geometry_count"])
        self.assertEqual(60, len(inventory["files"][0]["resources"]))

        self.assertEqual("copyright_undetermined", rights["assessment_status"])
        self.assertEqual("not_authorized", rights["redistribution_posture"])
        self.assertEqual("local_research_only", rights["derivative_posture"])
        self.assertEqual("local_only", rights["visibility"])
        self.assertIsNone(rights["access_request_ref"])
        self.assertEqual(["RU"], rights["jurisdictions_reviewed"])
        self.assertEqual(2, rights["record_version"])
        layers_by_role = {
            layer["layer_role"]: layer
            for layer in rights["layer_assessments"]
        }
        self.assertEqual(
            {
                "original_work",
                "translation",
                "preface",
                "edition_presentation",
                "digital_scan",
                "embedded_text",
                "annotation",
            },
            set(layers_by_role),
        )
        for role in ("original_work", "translation", "preface"):
            self.assertEqual(
                "public_domain_reviewed",
                layers_by_role[role]["assessment_status"],
            )
            self.assertEqual(
                "authorized_with_conditions",
                layers_by_role[role]["redistribution_posture"],
            )
            self.assertEqual(["RU"], layers_by_role[role]["jurisdictions_reviewed"])
        for role in (
            "edition_presentation",
            "digital_scan",
            "embedded_text",
            "annotation",
        ):
            self.assertEqual(
                "copyright_undetermined",
                layers_by_role[role]["assessment_status"],
            )
            self.assertEqual(
                "not_authorized",
                layers_by_role[role]["redistribution_posture"],
            )
        self.assertEqual(
            "local_research_only",
            layers_by_role["digital_scan"]["server_processing_posture"],
        )

        self.assertEqual("С. П. Нани", agent["preferred_label"])
        self.assertEqual("provisional", agent["identity_status"])
        self.assertEqual([], agent["external_identifiers"])
        self.assertEqual("provisional", printer["identity_status"])
        self.assertIn("printing-house", printer["notes"])
        self.assertIn("No publisher role", printer["notes"])

        responsibility = [
            json.loads(line)
            for line in NANI_1899_RESPONSIBILITY_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
        ]
        self.assertEqual(1, len(responsibility))
        self.assertEqual("translated_by", responsibility[0]["predicate"])
        self.assertEqual("tos.agent.s-p-nani", responsibility[0]["object"])
        self.assertEqual("unreviewed", responsibility[0]["review_status"])

        provision = [
            json.loads(line)
            for line in NANI_1899_PROVISION_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
        ]
        self.assertEqual(1, len(provision))
        activity = provision[0]["object"]
        self.assertEqual("manufacture", activity["provision_kind"])
        self.assertEqual("statement_date", activity["temporal"]["role"])
        self.assertEqual("1899", activity["temporal"]["value"])
        self.assertEqual(
            "tos.organization.m-m-stasyulevich-printing-saint-petersburg",
            activity["agents"][0]["normalized_agent_ref"],
        )
        self.assertNotIn("publisher", activity["agents"][0]["role"])
        self.assertIn("18 October 1898", activity["activity_warning"])

        anchors = [
            json.loads(line)
            for line in NANI_1899_SOURCE_ANCHORS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
        ]
        self.assertEqual(3, len(anchors))
        self.assertTrue(all(anchor["status"] == "proposed" for anchor in anchors))
        self.assertEqual(
            [59, 60],
            [
                selector["page"]
                for selector in anchors[-1]["selectors"]
            ],
        )

        self.assertEqual(60, snapshot["download"]["pages"])
        self.assertIsNone(
            snapshot["observed_extent_boundary"]["complete_cataloged_extent_returned"]
        )
        self.assertEqual(
            103,
            snapshot["observed_extent_boundary"]["last_visible_printed_page_number"],
        )

        discovery = json.loads(NANI_1899_DISCOVERY_PATH.read_text(encoding="utf-8"))
        self.assertEqual("incomplete", discovery["status"])
        self.assertEqual([1, 2, 3, 4], [row["sequence"] for row in discovery["channels"]])
        self.assertEqual("general-web-search", discovery["channels"][-1]["channel_type"])
        self.assertTrue(discovery["channels"][0]["results"][0]["acquisition"]["downloaded"])
        self.assertFalse(discovery["technical_access_bypass_used"])

        server_plan = json.loads(
            NANI_1899_SERVER_PLAN_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual("metadata-only", server_plan["access_class"])
        self.assertEqual("blocked-rights", server_plan["server_import_status"])
        self.assertFalse(server_plan["payload_transfer_authorized"])
        for key in ("ocr", "transcription", "page_images", "alignments", "translations", "embeddings"):
            self.assertEqual("prohibited", server_plan["allowed_derivatives"][key]["state"])

        research = NANI_1899_RESEARCH_PATH.read_text(encoding="utf-8")
        self.assertIn("The censor date is not the publication", research)
        self.assertIn("no completeness", research)
        self.assertIn("German Expression", research)
        self.assertIn("or authorize", research)
        rights_research = NANI_1899_RIGHTS_RESEARCH_PATH.read_text(
            encoding="utf-8"
        )
        self.assertIn("The answer cannot be one scalar", rights_research)
        self.assertIn("General web search, last", rights_research)
        self.assertIn("public_domain_reviewed", rights_research)
        self.assertIn("aggregate Item/File", rights_research)

    def test_antonovsky_revision_lineage_is_bounded_and_access_gap_stays_unsent(
        self,
    ) -> None:
        expression_root = (
            REPO_ROOT
            / "ToS/source-witnesses/works/friedrich-nietzsche/"
            "also-sprach-zarathustra/expressions"
        )
        expression_ids = {}
        expected_embodiment_claims = {
            "1900": [
                "tos.claim.topology.expression-edition.friedrich-nietzsche."
                "also-sprach-zarathustra.ru-antonovsky-1900.embodied-by."
                "saint-petersburg-unknown-publisher-1900"
            ],
            "1903": [
                "tos.claim.topology.expression-edition.friedrich-nietzsche."
                "also-sprach-zarathustra.ru-antonovsky-1903.embodied-by."
                "saint-petersburg-altshuler-typography-1903-second-corrected"
            ],
            "1907": [
                "tos.claim.topology.expression-edition.friedrich-nietzsche."
                "also-sprach-zarathustra.ru-antonovsky-1907.embodied-by."
                "saint-petersburg-vaisberg-gershunin-typography-1907-third"
            ],
        }
        for year in ("1900", "1903", "1907"):
            payload = json.loads(
                (expression_root / f"ru-antonovsky-{year}/expression.json").read_text(
                    encoding="utf-8"
                )
            )
            self.assertEqual("provisional", payload["identity_status"])
            self.assertEqual(
                expected_embodiment_claims[year],
                payload["embodiment_claim_refs"],
            )
            expression_ids[year] = payload["record_id"]

        edition = json.loads(
            (ANTONOVSKY_1900_EDITION_ROOT / "edition.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual("provisional", edition["identity_status"])
        self.assertEqual(
            [expression_ids["1900"]], edition["embodies_expression_refs"]
        )
        self.assertEqual([], edition["exemplar_claim_refs"])
        self.assertEqual([], edition["publication_claim_refs"])
        self.assertIsNone(edition["edition_statement"])
        self.assertFalse((ANTONOVSKY_1900_EDITION_ROOT / "items").exists())
        self.assertIn("[s.n.]", edition["notes"])
        self.assertEqual(2, edition["record_version"])
        self.assertEqual(
            {
                "01003693380",
                "003693380",
                "07NLR_LMS004843721",
                "004843721",
                "128/318",
            },
            {
                identifier["value"]
                for identifier in edition["external_identifiers"]
                if identifier["scheme"]
                in {
                    "RSL public record",
                    "RSL MARC 001",
                    "RNL Primo record",
                    "RNL system number",
                    "RNL shelfmark",
                }
                and identifier["status"] == "verified"
            },
        )
        self.assertIn("rc\\1717107", edition["notes"])
        self.assertIn("not ToS Items", edition["notes"])
        self.assertIn("[3], XI", edition["notes"])
        self.assertIn("[4], XII", edition["notes"])
        self.assertIn("remain unresolved", edition["notes"])

        edition_1903 = json.loads(
            (ANTONOVSKY_1903_EDITION_ROOT / "edition.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual("provisional", edition_1903["identity_status"])
        self.assertEqual(
            [expression_ids["1903"]], edition_1903["embodies_expression_refs"]
        )
        self.assertEqual("2-е изд., испр.", edition_1903["edition_statement"])
        self.assertEqual([], edition_1903["exemplar_claim_refs"])
        self.assertEqual([], edition_1903["publication_claim_refs"])
        self.assertFalse((ANTONOVSKY_1903_EDITION_ROOT / "items").exists())
        self.assertIn("тип. Альтшулера", edition_1903["notes"])
        self.assertIn("not converted into a publisher", edition_1903["notes"])
        self.assertEqual(
            {"129/5943", "38.35.6.32"},
            {
                identifier["value"]
                for identifier in edition_1903["external_identifiers"]
                if identifier["scheme"] == "RNL shelfmark"
                and identifier["status"] == "verified"
            },
        )
        self.assertIn("07NLR_LMS004843722", edition_1903["notes"])
        self.assertIn("not ToS Items", edition_1903["notes"])
        self.assertIn("TEMP", edition_1903["notes"])

        edition_1907 = json.loads(
            (ANTONOVSKY_1907_EDITION_ROOT / "edition.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual("provisional", edition_1907["identity_status"])
        self.assertEqual(
            [expression_ids["1907"]], edition_1907["embodies_expression_refs"]
        )
        self.assertEqual("3-е изд.", edition_1907["edition_statement"])
        self.assertEqual([], edition_1907["exemplar_claim_refs"])
        self.assertEqual([], edition_1907["publication_claim_refs"])
        self.assertFalse((ANTONOVSKY_1907_EDITION_ROOT / "items").exists())
        self.assertEqual(
            ["000199_000009_003693382"],
            [
                identifier["value"]
                for identifier in edition_1907["external_identifiers"]
                if identifier["scheme"] == "RuNEB record"
                and identifier["status"] == "verified"
            ],
        )
        self.assertIn(
            "тип. Ф. Вайсберга и П. Гершунина",
            edition_1907["notes"],
        )
        self.assertIn("removed or replaced", edition_1907["notes"])
        self.assertIn("not converted into a publisher", edition_1907["notes"])
        self.assertIn("VIII, 363 с.", edition_1907["notes"])
        self.assertIn("not transcribed", edition_1907["notes"])
        self.assertEqual(
            {"67133", "67133/82"},
            {
                identifier["value"]
                for identifier in edition_1907["external_identifiers"]
                if identifier["scheme"] in {"RNL GAK divider", "RNL GAK card"}
                and identifier["status"] == "verified"
            },
        )
        self.assertEqual(
            {"17.145.5.1", "17.145.5.1а", "17.145.5.1 Б"},
            {
                identifier["value"]
                for identifier in edition_1907["external_identifiers"]
                if identifier["scheme"] == "RNL shelfmark"
                and identifier["status"] == "verified"
            },
        )
        self.assertIn("07NLR_LMS004843723", edition_1907["notes"])
        self.assertIn("not ToS Items", edition_1907["notes"])
        self.assertIn("V 106/216", edition_1907["notes"])
        self.assertEqual(4, edition_1907["record_version"])
        self.assertEqual(
            {
                ("RNL RUSMARC 001", "v19\\rc\\1717109"),
                ("RSL shelfmark reported by RNL", "V 106/216"),
            },
            {
                (identifier["scheme"], identifier["value"])
                for identifier in edition_1907["external_identifiers"]
                if identifier["scheme"]
                in {"RNL RUSMARC 001", "RSL shelfmark reported by RNL"}
                and identifier["status"] == "verified"
            },
        )
        self.assertNotIn(
            "01003693382",
            {
                identifier["value"]
                for identifier in edition_1907["external_identifiers"]
            },
        )
        self.assertIn("RNL-reported RSL shelfmark", edition_1907["notes"])
        self.assertIn("not a current RSL public record", edition_1907["notes"])
        self.assertIn("БАН row has no call number", edition_1907["notes"])

        exact_discovery = json.loads(
            ANTONOVSKY_1900_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual("incomplete", exact_discovery["status"])
        self.assertEqual(
            "tos.discovery.antonovsky-1900-lnb-physical-holding.2026-08-01",
            exact_discovery["supersedes_discovery_ref"],
        )
        self.assertEqual(
            [1, 2, 3, 4, 5, 6, 7, 8],
            [channel["sequence"] for channel in exact_discovery["channels"]],
        )
        self.assertEqual(
            "general-web-search", exact_discovery["channels"][-1]["channel_type"]
        )
        self.assertFalse(exact_discovery["technical_access_bypass_used"])
        exact_1900_crosswalk = exact_discovery["channels"][1]["results"][0]
        self.assertEqual(
            {
                "01003693380",
                "003693380",
                "07NLR_LMS004843721",
                "004843721",
            },
            {
                identifier["value"]
                for identifier in exact_1900_crosswalk["identifiers"]
                if identifier["scheme"]
                in {
                    "RSL public record",
                    "RSL MARC 001",
                    "RNL Primo record",
                    "RNL system number",
                }
            },
        )
        exact_1900_holdings = exact_discovery["channels"][2]["results"][0]
        self.assertEqual(
            {
                "FB Рб 18/414",
                "OMF 810-83/174-7 (1900, ч. 1-4)",
                "128/318",
                "1.40686",
            },
            {
                identifier["value"]
                for identifier in exact_1900_holdings["identifiers"]
            },
        )
        exact_1900_discrepancy = exact_discovery["channels"][2]["results"][1]
        self.assertEqual("defer", exact_1900_discrepancy["decision"])
        self.assertIn("[3] XI", exact_1900_discrepancy["rationale"])
        self.assertIn("[4], XII", exact_1900_discrepancy["rationale"])
        for rejected_result_id in (
            "tos-discovery-result.antonovsky-1900-rnl-free-text-first-page-false-negative",
            "tos-discovery-result.antonovsky-1900-rsl-generic-read-modal-false-positive",
            "tos-discovery-result.antonovsky-1900-open-libraries-no-exact-item-v2",
            "tos-discovery-result.antonovsky-1900-cinii-1981-not-target",
        ):
            self.assertIn(
                rejected_result_id,
                exact_discovery["rejected_result_ids"],
            )
        self.assertTrue(
            all(
                not result["acquisition"]["downloaded"]
                for channel in exact_discovery["channels"]
                for result in channel["results"]
            )
        )

        exact_1903_discovery = json.loads(
            ANTONOVSKY_1903_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual("incomplete", exact_1903_discovery["status"])
        self.assertEqual(
            "tos.discovery.antonovsky-1903-rsl-edition-holdings.2026-08-01",
            exact_1903_discovery["supersedes_discovery_ref"],
        )
        self.assertEqual(
            [1, 2, 3, 4, 5, 6, 7, 8],
            [channel["sequence"] for channel in exact_1903_discovery["channels"]],
        )
        self.assertEqual(
            "general-web-search",
            exact_1903_discovery["channels"][-1]["channel_type"],
        )
        self.assertFalse(exact_1903_discovery["technical_access_bypass_used"])
        exact_1903_crosswalk = exact_1903_discovery["channels"][1]["results"][0]
        self.assertEqual(
            {
                "01003693381",
                "003693381",
                "07NLR_LMS004843722",
                "004843722",
            },
            {
                identifier["value"]
                for identifier in exact_1903_crosswalk["identifiers"]
                if identifier["scheme"]
                in {
                    "RSL public record",
                    "RSL MARC 001",
                    "RNL Primo record",
                    "RNL system number",
                }
            },
        )
        exact_1903_holdings = exact_1903_discovery["channels"][2]["results"][0]
        self.assertEqual(
            {
                "FB L 31/57",
                "FB Рб 33/442",
                "FB T 97/44",
                "OMF 801-85/11097-8",
                "129/5943",
                "38.35.6.32",
            },
            {
                identifier["value"]
                for identifier in exact_1903_holdings["identifiers"]
            },
        )
        self.assertIn(
            "tos-discovery-result.antonovsky-1903-rnl-undefined-temp-row",
            exact_1903_discovery["rejected_result_ids"],
        )
        self.assertIn(
            "tos-discovery-result.antonovsky-1903-rsl-generic-read-modal-false-positive",
            exact_1903_discovery["rejected_result_ids"],
        )
        self.assertTrue(
            all(
                not result["acquisition"]["downloaded"]
                for channel in exact_1903_discovery["channels"]
                for result in channel["results"]
            )
        )

        exact_1907_discovery = json.loads(
            ANTONOVSKY_1907_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual("incomplete", exact_1907_discovery["status"])
        self.assertEqual(
            [1, 2, 3, 4, 5, 6],
            [channel["sequence"] for channel in exact_1907_discovery["channels"]],
        )
        self.assertEqual(
            "general-web-search",
            exact_1907_discovery["channels"][-1]["channel_type"],
        )
        self.assertFalse(exact_1907_discovery["technical_access_bypass_used"])
        self.assertEqual(
            "metadata-only",
            exact_1907_discovery["channels"][0]["results"][0]["availability"],
        )

        rnl_1907_discovery = json.loads(
            ANTONOVSKY_1907_RNL_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual("incomplete", rnl_1907_discovery["status"])
        self.assertEqual(
            "tos.discovery.antonovsky-1907-rnl-gak-holding.2026-08-10",
            rnl_1907_discovery["supersedes_discovery_ref"],
        )
        self.assertEqual(
            [1, 2, 3, 4, 5, 6],
            [channel["sequence"] for channel in rnl_1907_discovery["channels"]],
        )
        self.assertEqual(
            "general-web-search",
            rnl_1907_discovery["channels"][-1]["channel_type"],
        )
        self.assertFalse(rnl_1907_discovery["technical_access_bypass_used"])
        current_record = rnl_1907_discovery["channels"][1]["results"][0]
        self.assertEqual("select", current_record["decision"])
        self.assertEqual("metadata-only", current_record["availability"])
        self.assertEqual(
            {"07NLR_LMS004843723", "004843723"},
            {
                identifier["value"]
                for identifier in current_record["identifiers"]
                if identifier["scheme"]
                in {"RNL Primo record", "RNL system number"}
            },
        )
        current_holdings = rnl_1907_discovery["channels"][2]["results"][0]
        self.assertIn("Three current item rows", current_holdings["rationale"])
        self.assertIn("В хранении", current_holdings["rationale"])
        self.assertIn("or ToS Item custody", current_holdings["rationale"])
        access_result = rnl_1907_discovery["channels"][4]["results"][0]
        self.assertIn("recommendation channel only", access_result["rationale"])
        self.assertIn(
            "rights permission separate", access_result["rationale"]
        )
        self.assertTrue(
            all(
                not result["acquisition"]["downloaded"]
                for channel in rnl_1907_discovery["channels"]
                for result in channel["results"]
            )
        )

        reported_rsl_discovery = json.loads(
            ANTONOVSKY_1907_RNL_REPORTED_RSL_DISCOVERY_PATH.read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual("incomplete", reported_rsl_discovery["status"])
        self.assertEqual(
            "tos.discovery.antonovsky-1907-rnl-primo-current-holding.2026-08-10",
            reported_rsl_discovery["supersedes_discovery_ref"],
        )
        self.assertEqual(
            [1, 2, 3, 4, 5, 6, 7],
            [channel["sequence"] for channel in reported_rsl_discovery["channels"]],
        )
        self.assertEqual(
            "general-web-search",
            reported_rsl_discovery["channels"][-1]["channel_type"],
        )
        self.assertFalse(reported_rsl_discovery["technical_access_bypass_used"])
        format_results = reported_rsl_discovery["channels"][0]["results"]
        self.assertEqual(
            {"select"},
            {result["decision"] for result in format_results},
        )
        current_cross_agency_record = reported_rsl_discovery["channels"][1][
            "results"
        ][0]
        self.assertEqual(
            {
                ("RNL RUSMARC 001", "v19\\rc\\1717109"),
                ("RSL shelfmark reported by RNL", "V 106/216"),
            },
            {
                (identifier["scheme"], identifier["value"])
                for identifier in current_cross_agency_record["identifiers"]
                if identifier["scheme"]
                in {"RNL RUSMARC 001", "RSL shelfmark reported by RNL"}
            },
        )
        self.assertIn(
            "not an RSL record or physically inspected Item",
            current_cross_agency_record["rationale"],
        )
        ban_row = reported_rsl_discovery["channels"][1]["results"][1]
        self.assertEqual("defer", ban_row["decision"])
        self.assertEqual([], ban_row["identifiers"])
        rsl_404 = reported_rsl_discovery["channels"][4]["results"][0]
        self.assertEqual("reject", rsl_404["decision"])
        self.assertIn("HTTP 404", rsl_404["rationale"])
        self.assertIn(
            rsl_404["result_id"], reported_rsl_discovery["rejected_result_ids"]
        )
        self.assertTrue(
            all(
                not result["acquisition"]["downloaded"]
                for channel in reported_rsl_discovery["channels"]
                for result in channel["results"]
            )
        )

        exact_request = json.loads(
            ANTONOVSKY_1900_REQUEST_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual("draft-not-sent", exact_request["request_status"])
        self.assertEqual(2, exact_request["record_version"])
        self.assertFalse(exact_request["human_send_approval"])
        self.assertIsNone(exact_request["sent_at"])
        self.assertEqual("none", exact_request["response"]["state"])
        self.assertEqual(
            "not-requested",
            exact_request["requested_permissions"]["source_redistribution"],
        )
        self.assertEqual(
            "not-requested",
            exact_request["requested_permissions"]["derivative_publication"],
        )
        self.assertIn(
            "[3], XI",
            exact_request["material"]["requested_portion"],
        )
        self.assertIn(
            "[4], XII",
            exact_request["material"]["requested_portion"],
        )
        self.assertIn(
            "tos.event.access-request-scope-reconciliation."
            "antonovsky-1900-rsl-rnl-lnb-current-holdings.2026-08-10",
            exact_request["provenance_event_refs"],
        )

        exact_1903_request = json.loads(
            ANTONOVSKY_1903_REQUEST_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual("draft-not-sent", exact_1903_request["request_status"])
        self.assertFalse(exact_1903_request["human_send_approval"])
        self.assertIsNone(exact_1903_request["sent_at"])
        self.assertEqual("none", exact_1903_request["response"]["state"])
        for permission in (
            "source_redistribution",
            "derivative_publication",
            "server_processing",
        ):
            self.assertEqual(
                "not-requested",
                exact_1903_request["requested_permissions"][permission],
            )

        rnl_1903_request = json.loads(
            ANTONOVSKY_1903_RNL_REQUEST_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual("draft-not-sent", rnl_1903_request["request_status"])
        self.assertFalse(rnl_1903_request["human_send_approval"])
        self.assertIsNone(rnl_1903_request["sent_at"])
        self.assertEqual("none", rnl_1903_request["response"]["state"])
        self.assertIn(
            "два экземпляра",
            rnl_1903_request["material"]["requested_portion"],
        )
        self.assertIn(
            "только как отдельная рекомендация",
            rnl_1903_request["material"]["requested_portion"],
        )
        for permission in (
            "source_redistribution",
            "derivative_publication",
            "server_processing",
        ):
            self.assertEqual(
                "not-requested",
                rnl_1903_request["requested_permissions"][permission],
            )

        exact_1907_request = json.loads(
            ANTONOVSKY_1907_REQUEST_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual("draft-not-sent", exact_1907_request["request_status"])
        self.assertFalse(exact_1907_request["human_send_approval"])
        self.assertIsNone(exact_1907_request["sent_at"])
        self.assertEqual("none", exact_1907_request["response"]["state"])
        self.assertEqual(2, exact_1907_request["record_version"])
        self.assertIn(
            "V 106/216 действующим шифром именно этого издания",
            exact_1907_request["material"]["requested_portion"],
        )
        self.assertIn(
            ("RSL shelfmark reported by RNL", "V 106/216"),
            {
                (identifier["scheme"], identifier["value"])
                for identifier in exact_1907_request["material"]["identifiers"]
            },
        )
        self.assertIn(
            "tos.event.access-request-scope-reconciliation."
            "antonovsky-1907-rnl-reported-rsl-holding.2026-08-10",
            exact_1907_request["provenance_event_refs"],
        )
        for permission in (
            "source_redistribution",
            "derivative_publication",
            "server_processing",
        ):
            self.assertEqual(
                "not-requested",
                exact_1907_request["requested_permissions"][permission],
            )

        rnl_1907_request = json.loads(
            ANTONOVSKY_1907_RNL_REQUEST_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual("draft-not-sent", rnl_1907_request["request_status"])
        self.assertFalse(rnl_1907_request["human_send_approval"])
        self.assertIsNone(rnl_1907_request["sent_at"])
        self.assertEqual("none", rnl_1907_request["response"]["state"])
        self.assertIn(
            "три экземпляра",
            rnl_1907_request["material"]["requested_portion"],
        )
        self.assertIn(
            "только как отдельная рекомендация",
            rnl_1907_request["material"]["requested_portion"],
        )
        self.assertEqual(2, rnl_1907_request["record_version"])
        for permission in (
            "source_redistribution",
            "derivative_publication",
            "server_processing",
        ):
            self.assertEqual(
                "not-requested",
                rnl_1907_request["requested_permissions"][permission],
            )

        claim_path = (
            REPO_ROOT
            / "ToS/source-witnesses/relations/expression-derivation/"
            "expression-derivation-claims.jsonl"
        )
        claims = [
            json.loads(line)
            for line in claim_path.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        pairs = {(claim["subject_ref"], claim["object"]) for claim in claims}
        self.assertEqual(
            {
                (expression_ids["1903"], expression_ids["1900"]),
                (expression_ids["1907"], expression_ids["1903"]),
            },
            pairs,
        )
        for claim in claims:
            self.assertEqual("relation", claim["claim_type"])
            self.assertEqual("reported", claim["epistemic_status"])
            self.assertEqual("unreviewed", claim["review_status"])
            self.assertEqual("not_collated", claim["qualifiers"]["collation_status"])
            self.assertFalse(claim["qualifiers"]["transitive"])
            self.assertTrue(claim["qualifiers"]["asymmetric"])
            self.assertTrue(claim["qualifiers"]["irreflexive"])
            self.assertFalse(claim["qualifiers"]["equivalence_inferred"])

        discovery = json.loads(
            (
                REPO_ROOT
                / "ToS/source-witnesses/discovery/runs/"
                "antonovsky-1907-revision-witness.2026-08-01.v1.json"
            ).read_text(encoding="utf-8")
        )
        self.assertEqual("incomplete", discovery["status"])
        self.assertEqual("general-web-search", discovery["channels"][-1]["channel_type"])
        self.assertFalse(discovery["technical_access_bypass_used"])

        request = json.loads(
            (
                REPO_ROOT
                / "ToS/source-witnesses/access-requests/public-ledger/"
                "antonovsky-1907-blok-library.access-request.json"
            ).read_text(encoding="utf-8")
        )
        self.assertEqual("draft-not-sent", request["request_status"])
        self.assertFalse(request["human_send_approval"])
        self.assertIsNone(request["sent_at"])
        self.assertEqual("not-requested", request["requested_permissions"]["source_redistribution"])
        self.assertFalse(request["personal_or_confidential_data_committed"])

    def test_mysl_translator_identities_resolve_asymmetrically_without_claim_drift(
        self,
    ) -> None:
        svasyan = json.loads(
            (AGENT_ROOT / "k-a-svasyan/agent.json").read_text(encoding="utf-8")
        )
        polilov = json.loads(
            (AGENT_ROOT / "n-polilov/agent.json").read_text(encoding="utf-8")
        )
        flerova = json.loads(
            (AGENT_ROOT / "v-a-flerova/agent.json").read_text(encoding="utf-8")
        )

        self.assertEqual("Карен Араевич Свасьян", svasyan["preferred_label"])
        self.assertEqual("verified", svasyan["identity_status"])
        self.assertEqual(2, svasyan["record_version"])
        self.assertEqual(
            ["120452367"],
            [
                identifier["value"]
                for identifier in svasyan["external_identifiers"]
                if identifier["scheme"] == "GND"
            ],
        )
        self.assertEqual(
            ["К. А. Свасьян"],
            [variant["value"] for variant in svasyan["variant_labels"]],
        )

        self.assertEqual("Николай Николаевич Полилов", polilov["preferred_label"])
        self.assertEqual("verified", polilov["identity_status"])
        self.assertEqual(2, polilov["record_version"])
        self.assertEqual(
            ["1012315509"],
            [
                identifier["value"]
                for identifier in polilov["external_identifiers"]
                if identifier["scheme"] == "GND"
            ],
        )
        self.assertEqual(
            {"Н. Полилов", "Н. Н. Полилов"},
            {variant["value"] for variant in polilov["variant_labels"]},
        )

        self.assertEqual("В. А. Флёрова", flerova["preferred_label"])
        self.assertEqual("provisional", flerova["identity_status"])
        self.assertEqual([], flerova["external_identifiers"])
        self.assertIn("born in 1913", flerova["notes"])
        self.assertIn("must not be normalized", flerova["notes"])

        discovery = json.loads(
            MYSL_TRANSLATOR_IDENTITY_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual("reconciled", discovery["status"])
        self.assertTrue(discovery["general_web_search_is_last_resort"])
        self.assertEqual(
            "channel-general-web-mysl-translator-identities-last",
            discovery["channels"][-1]["channel_id"],
        )
        self.assertEqual(
            {
                "tos-discovery-result.dnb-gnd-120452367-karen-swassjan",
                "tos-discovery-result.dnb-gnd-1012315509-n-polilov",
            },
            {
                result_id
                for result_id in discovery["selected_result_ids"]
                if result_id.startswith("tos-discovery-result.dnb-gnd-")
            },
        )
        self.assertIn(
            "tos-discovery-result.general-web-vera-aleksandrovna-flerova-collision",
            discovery["rejected_result_ids"],
        )

        claims = [
            json.loads(line)
            for line in MYSL_RESPONSIBILITY_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
        ]
        translated_by = [claim for claim in claims if claim["predicate"] == "translated_by"]
        self.assertEqual(7, len(translated_by))
        reconciled_translator_claims = [
            claim
            for claim in translated_by
            if claim["object"]
            in {
                "tos.agent.k-a-svasyan",
                "tos.agent.n-polilov",
                "tos.agent.v-a-flerova",
            }
        ]
        self.assertEqual(5, len(reconciled_translator_claims))
        self.assertEqual(
            {
                "tos.agent.k-a-svasyan",
                "tos.agent.n-polilov",
                "tos.agent.v-a-flerova",
            },
            {claim["object"] for claim in reconciled_translator_claims},
        )
        self.assertTrue(
            all(
                claim["review_status"] == "unreviewed"
                and claim["visibility"] == "public_metadata_only"
                for claim in reconciled_translator_claims
            )
        )

        research = MYSL_TRANSLATOR_IDENTITY_RESEARCH_PATH.read_text(
            encoding="utf-8"
        )
        self.assertIn("The object and claim counts must remain unchanged", research)
        self.assertIn("chronologically impossible", research)

    def test_antonovsky_identity_corrects_the_speaking_path_without_claim_drift(
        self,
    ) -> None:
        corrected_path = AGENT_ROOT / "yuliy-antonovsky/agent.json"
        legacy_path = AGENT_ROOT / "yuri-antonovsky/agent.json"
        self.assertTrue(corrected_path.is_file())
        self.assertFalse(legacy_path.exists())

        antonovsky = json.loads(corrected_path.read_text(encoding="utf-8"))
        self.assertEqual("tos.agent.yuri-antonovsky", antonovsky["record_id"])
        self.assertEqual(
            "Юлий Михайлович Антоновский", antonovsky["preferred_label"]
        )
        self.assertEqual("verified", antonovsky["identity_status"])
        self.assertEqual(3, antonovsky["record_version"])
        self.assertEqual(
            ["123235553"],
            [
                identifier["value"]
                for identifier in antonovsky["external_identifiers"]
                if identifier["scheme"] == "GND"
            ],
        )
        variants = {
            variant["value"]: variant["status"]
            for variant in antonovsky["variant_labels"]
        }
        self.assertEqual("verified", variants["Ю. М. Антоновский"])
        self.assertEqual("verified", variants["Antonovskij, Julij Michajlovič"])
        self.assertEqual("rejected", variants["Yuri M. Antonovsky"])
        self.assertIn("legacy ToS record ID", antonovsky["notes"])
        self.assertIn("not a forename assertion", antonovsky["notes"])

        discovery = json.loads(
            ANTONOVSKY_IDENTITY_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual("reconciled", discovery["status"])
        self.assertTrue(discovery["general_web_search_is_last_resort"])
        self.assertEqual(
            [1, 2, 3, 4],
            [channel["sequence"] for channel in discovery["channels"]],
        )
        self.assertEqual(
            "channel-general-web-antonovsky-identity-last",
            discovery["channels"][-1]["channel_id"],
        )
        self.assertIn(
            "tos-discovery-result.dnb-gnd-123235553-julij-antonovskij",
            discovery["selected_result_ids"],
        )
        self.assertIn(
            "tos-discovery-result.dnb-gnd-1089232756-michail-jakovlevich-antonovskij-negative",
            discovery["rejected_result_ids"],
        )

        mysl_claims = [
            json.loads(line)
            for line in MYSL_RESPONSIBILITY_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
        ]
        exact_claims = [
            json.loads(line)
            for line in ANTONOVSKY_1913_RESPONSIBILITY_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
        ]
        cultural_revolution_claims = [
            json.loads(line)
            for line in ANTONOVSKY_2007_RESPONSIBILITY_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
        ]
        antonovsky_1911_claims = [
            json.loads(line)
            for line in ANTONOVSKY_1911_RESPONSIBILITY_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
        ]
        antonovsky_claims = [
            claim
            for claim in [
                *mysl_claims,
                *antonovsky_1911_claims,
                *exact_claims,
                *cultural_revolution_claims,
            ]
            if claim["predicate"] == "translated_by"
            and claim["object"] == "tos.agent.yuri-antonovsky"
        ]
        self.assertEqual(5, len(antonovsky_claims))
        self.assertEqual(
            {
                "tos.claim.expression.also-sprach-zarathustra.ru-antonovsky-1911.translated-by-yuri-antonovsky",
                "tos.claim.expression.also-sprach-zarathustra.ru-antonovsky-1913.translated-by-yuri-antonovsky",
                "tos.claim.expression.also-sprach-zarathustra.ru-antonovsky-cultural-revolution-2007.translated-by-yuri-antonovsky",
                "tos.claim.expression.mysl-1996-volume-2.also-sprach-zarathustra.translated-by-yuri-antonovsky",
                "tos.claim.expression.mysl-1996-volume-2.ecce-homo.translated-by-yuri-antonovsky",
            },
            {claim["claim_id"] for claim in antonovsky_claims},
        )
        self.assertTrue(
            all(
                claim["review_status"] == "unreviewed"
                and claim["visibility"] == "public_metadata_only"
                for claim in antonovsky_claims
            )
        )

        research = ANTONOVSKY_IDENTITY_RESEARCH_PATH.read_text(encoding="utf-8")
        self.assertIn("retain `tos.agent.yuri-antonovsky`", research)
        self.assertIn("agents/yuliy-antonovsky/", research)
        self.assertIn("general web search last", research)

    def test_antonovsky_1913_translation_responsibility_returns_to_page_7(
        self,
    ) -> None:
        anchor = json.loads(
            ANTONOVSKY_1913_TITLE_ANCHORS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()[0]
        )
        claim = json.loads(
            ANTONOVSKY_1913_RESPONSIBILITY_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()[0]
        )
        expression = json.loads(
            (ANTONOVSKY_1913_EXPRESSION_ROOT / "expression.json").read_text(
                encoding="utf-8"
            )
        )

        self.assertEqual(
            "tos.anchor.also-sprach-zarathustra.ru-antonovsky-1913."
            "title-page-translator-credit",
            anchor["anchor_id"],
        )
        self.assertEqual("proposed", anchor["status"])
        self.assertEqual(7, anchor["selectors"][0]["page"])
        self.assertEqual(
            "687716bc25ebf2281b967ebb0c6cf16b043c2d40bd16833d57d6dcf260d3476b",
            anchor["file_sha256"],
        )
        self.assertEqual("translated_by", claim["predicate"])
        self.assertEqual(expression["record_id"], claim["subject_ref"])
        self.assertEqual("tos.agent.yuri-antonovsky", claim["object"])
        self.assertIn(anchor["anchor_id"], claim["evidence_refs"])
        self.assertIn(
            ANTONOVSKY_1913_TRANSLATION_RESEARCH_PATH.relative_to(
                REPO_ROOT
            ).as_posix(),
            claim["evidence_refs"],
        )
        self.assertEqual("observed", claim["epistemic_status"])
        self.assertEqual("unreviewed", claim["review_status"])
        self.assertEqual("public_metadata_only", claim["visibility"])
        self.assertEqual([claim["claim_id"]], expression["responsibility_claim_refs"])
        serialized = json.dumps(claim, ensure_ascii=False)
        self.assertNotIn("Юрий Михайлович", serialized)
        self.assertNotIn("same_as", serialized)

    def test_antonovsky_2007_translation_responsibility_preserves_edition_state(
        self,
    ) -> None:
        anchors = [
            json.loads(line)
            for line in ANTONOVSKY_2007_RESPONSIBILITY_ANCHORS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
        ]
        claim = json.loads(
            ANTONOVSKY_2007_RESPONSIBILITY_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()[0]
        )
        expression = json.loads(
            (ANTONOVSKY_2007_EXPRESSION_ROOT / "expression.json").read_text(
                encoding="utf-8"
            )
        )
        discovery = json.loads(
            ANTONOVSKY_2007_TRANSLATION_DISCOVERY_PATH.read_text(
                encoding="utf-8"
            )
        )

        self.assertEqual(
            [3, 4],
            [anchor["selectors"][0]["page"] for anchor in anchors],
        )
        self.assertTrue(all(anchor["status"] == "proposed" for anchor in anchors))
        self.assertTrue(
            all(
                anchor["file_sha256"]
                == "49614015865f8c65a68746b1a6dc9a8b7f036d817b2e04b1f3b94fec8ef7b0c2"
                for anchor in anchors
            )
        )
        self.assertEqual("translated_by", claim["predicate"])
        self.assertEqual(expression["record_id"], claim["subject_ref"])
        self.assertEqual("tos.agent.yuri-antonovsky", claim["object"])
        self.assertEqual(
            {anchor["anchor_id"] for anchor in anchors},
            {
                evidence_ref
                for evidence_ref in claim["evidence_refs"]
                if evidence_ref.startswith("tos.anchor.")
            },
        )
        self.assertEqual("no_equivalence_claim", expression["same_as_posture"])
        self.assertIn("newly edited", expression["notes"])
        self.assertEqual([claim["claim_id"]], expression["responsibility_claim_refs"])
        self.assertEqual(
            [1, 2, 3, 4, 5, 6],
            [channel["sequence"] for channel in discovery["channels"]],
        )
        self.assertIn(
            "tos-discovery-result.azbuka-2026-yuri-antonovsky-negative-control",
            discovery["rejected_result_ids"],
        )
        discovery_events = [
            json.loads(line)
            for line in DISCOVERY_PROVENANCE_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
        ]
        self.assertEqual(
            1,
            sum(
                event["event_id"]
                == "tos.event.discovery.also-sprach-zarathustra."
                "ru-antonovsky-cultural-revolution-2007."
                "translation-responsibility.2026-08-01"
                for event in discovery_events
            ),
        )
        self.assertIn(
            ANTONOVSKY_2007_TRANSLATION_DISCOVERY_PATH.name,
            DISCOVERY_RUNS_README_PATH.read_text(encoding="utf-8"),
        )
        research = ANTONOVSKY_2007_TRANSLATION_RESEARCH_PATH.read_text(
            encoding="utf-8"
        )
        self.assertIn("local_only", research)
        self.assertIn("not asserted equivalent", research)

    def test_provision_activity_contract_preserves_roles_and_date_boundaries(
        self,
    ) -> None:
        schema = json.loads(
            PROVISION_ACTIVITY_SCHEMA_PATH.read_text(encoding="utf-8")
        )
        validator_class = validator_for(schema)
        validator_class.check_schema(schema)
        validator = validator_class(schema, format_checker=FormatChecker())
        goetzen = json.loads(
            GOETZEN_1889_PROVISION_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()[0]
        )["object"]
        ecce = json.loads(
            ECCE_HOMO_1908_PROVISION_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()[0]
        )["object"]
        self.assertEqual([], list(validator.iter_errors(goetzen)))
        self.assertEqual([], list(validator.iter_errors(ecce)))
        self.assertIn("transcribed_statement", goetzen)
        self.assertNotIn("reported_statement", goetzen)
        self.assertIn("reported_statement", ecce)
        self.assertNotIn("transcribed_statement", ecce)

        invalid_day = copy.deepcopy(goetzen)
        invalid_day["temporal"].update(
            {"value": "1889-02-30", "precision": "day"}
        )
        self.assertTrue(list(validator.iter_errors(invalid_day)))

        flat_publisher = copy.deepcopy(goetzen)
        flat_publisher["publisher"] = "C. G. Naumann"
        self.assertTrue(list(validator.iter_errors(flat_publisher)))

        collapsed_release = copy.deepcopy(goetzen)
        collapsed_release["temporal"]["role"] = "public_release_date"
        self.assertTrue(list(validator.iter_errors(collapsed_release)))

        wrong_role = copy.deepcopy(goetzen)
        wrong_role["agents"][0]["role"] = "printer"
        self.assertTrue(list(validator.iter_errors(wrong_role)))

        reversed_interval = copy.deepcopy(goetzen)
        reversed_interval["temporal"] = {
            "kind": "interval",
            "calendar": "gregorian",
            "start": "1909",
            "end": "1908",
            "start_precision": "year",
            "end_precision": "year",
            "role": "statement_date",
            "source_posture": "reported",
        }
        self.assertEqual([], list(validator.iter_errors(reversed_interval)))
        self.assertEqual(
            ["provision-activity interval starts after it ends"],
            foundation._provision_temporal_issues(reversed_interval),
        )

    def test_first_publication_chronology_preserves_distinct_ordering_facets(
        self,
    ) -> None:
        claims = {
            claim["subject_ref"]: claim
            for line in WORK_CHRONOLOGY_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
            for claim in (json.loads(line),)
        }
        self.assertEqual(7, len(claims))

        zarathustra = claims[
            "tos.work.friedrich-nietzsche.also-sprach-zarathustra"
        ]["object"]
        self.assertEqual(
            {
                "start": "1883",
                "end": "1885",
                "start_precision": "year",
                "end_precision": "year",
                "boundary_meaning": "earliest_stage_to_sequence_completion",
            },
            zarathustra["interval"],
        )
        self.assertEqual("staged_sequence", zarathustra["sequence_posture"])
        self.assertEqual(4, len(zarathustra["stages"]))
        self.assertEqual("private", zarathustra["stages"][-1]["availability"])
        self.assertIn("not the date", zarathustra["ordering_warning"])

        fall_wagner = claims[
            "tos.work.friedrich-nietzsche.der-fall-wagner"
        ]["object"]
        self.assertEqual("1888-09-22", fall_wagner["interval"]["start"])
        self.assertEqual("author_supervised_public", fall_wagner["publication_posture"])

        goetzen = claims[
            "tos.work.friedrich-nietzsche.goetzen-daemmerung"
        ]["object"]
        self.assertEqual("1889-01", goetzen["interval"]["start"])
        self.assertEqual("1889-01-24", goetzen["stages"][0]["date"])
        self.assertIn("commonly reported", goetzen["stages"][0]["date_posture"])
        self.assertIn("November 1888", goetzen["ordering_warning"])

        chronology_schema = json.loads(
            WORK_CHRONOLOGY_SCHEMA_PATH.read_text(encoding="utf-8")
        )
        validator_class = validator_for(chronology_schema)
        validator_class.check_schema(chronology_schema)
        chronology_validator = validator_class(
            chronology_schema,
            format_checker=FormatChecker(),
        )
        self.assertEqual([], list(chronology_validator.iter_errors(goetzen)))

        mismatched_precision = copy.deepcopy(goetzen)
        mismatched_precision["stages"][0]["precision"] = "month"
        self.assertTrue(list(chronology_validator.iter_errors(mismatched_precision)))

        impossible_day = copy.deepcopy(goetzen)
        impossible_day["stages"][0]["date"] = "1889-02-31"
        self.assertTrue(list(chronology_validator.iter_errors(impossible_day)))

        for work_ref in (
            "tos.work.friedrich-nietzsche.der-antichrist",
            "tos.work.friedrich-nietzsche.ecce-homo",
        ):
            self.assertEqual(
                "posthumous_editorial_public",
                claims[work_ref]["object"]["publication_posture"],
            )

        provenance = json.loads(
            WORK_CHRONOLOGY_PROVENANCE_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual(
            "tos.event.annotation.friedrich-nietzsche."
            "first-publication-chronology.2026-07-31",
            provenance["event_id"],
        )
        self.assertEqual(
            hashlib.sha256(WORK_CHRONOLOGY_CLAIMS_PATH.read_bytes()).hexdigest(),
            provenance["outputs"][0]["sha256"],
        )

    def test_source_witness_claim_catalog_rejects_nonpublic_claims(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo_root = Path(temporary)
            claim_path = (
                repo_root
                / catalog_builder.SOURCE_ROOT
                / "works/example/responsibility-claims.jsonl"
            )
            claim_path.parent.mkdir(parents=True)
            claim_path.write_text(
                json.dumps(
                    {
                        "claim_id": "tos.claim.example.local-only",
                        "visibility": "local_only",
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                catalog_builder.CatalogBuildError,
                "is not safe for the tracked claim catalog",
            ):
                catalog_builder.collect_claims(repo_root)

    def test_ekgwb_rights_separate_private_adaptation_from_sharing(self) -> None:
        validator, _ = foundation._schema_validator(
            foundation.RIGHTS_SCHEMA,
            REPO_ROOT,
        )
        rights = json.loads(EKGWB_RIGHTS_PATH.read_text(encoding="utf-8"))

        self.assertEqual([], list(validator.iter_errors(rights)))
        self.assertEqual("conflicting_evidence", rights["assessment_status"])
        self.assertEqual("local_only", rights["visibility"])
        self.assertEqual("metadata_only", rights["redistribution_posture"])
        self.assertEqual("local_research_only", rights["derivative_posture"])
        self.assertTrue(
            any(
                "private local reproduction and adaptation" in permission
                for permission in rights["permissions"]
            )
        )
        self.assertTrue(
            any(
                "adapted material may not be shared" in restriction
                for restriction in rights["restrictions"]
            )
        )
        self.assertEqual("unreviewed", rights["review_status"])

        for ref in [*rights["source_refs"], rights["access_request_ref"]]:
            if ref.startswith("ToS/"):
                self.assertTrue((REPO_ROOT / ref).is_file(), ref)

        access_request = json.loads(
            (REPO_ROOT / rights["access_request_ref"]).read_text(encoding="utf-8")
        )
        self.assertEqual("draft-not-sent", access_request["request_status"])
        self.assertFalse(access_request["human_send_approval"])
        self.assertEqual(
            {
                "ocr_or_transcription": "not-requested",
                "indexing": "not-requested",
                "embeddings": "not-requested",
                "derivative_publication": "not-requested",
                "source_redistribution": "not-requested",
            },
            {
                key: access_request["requested_permissions"][key]
                for key in (
                    "ocr_or_transcription",
                    "indexing",
                    "embeddings",
                    "derivative_publication",
                    "source_redistribution",
                )
            },
        )

    def test_ekgwb_institutional_archive_corroborates_without_admission(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.MATERIAL_DISCOVERY_SCHEMA,
            REPO_ROOT,
        )
        discovery = json.loads(
            EKGWB_INSTITUTIONAL_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual([], list(validator.iter_errors(discovery)))
        self.assertEqual("reconciled", discovery["status"])
        self.assertEqual(3, discovery["record_version"])
        self.assertFalse(discovery["technical_access_bypass_used"])
        self.assertEqual(
            {
                "tos-discovery-result.ekgwb-owner-http-texts-za-i-2026-07-30",
                "tos-discovery-result.arquivo-pt-ekgwb-za-i-20230320222635",
            },
            set(discovery["selected_result_ids"]),
        )

        channels = {
            channel["channel_id"]: channel for channel in discovery["channels"]
        }
        self.assertEqual(
            max(channel["sequence"] for channel in discovery["channels"]),
            channels["channel-general-web-last"]["sequence"],
        )
        owner = channels["channel-nietzsche-source-owner"]["results"][0]
        archive = channels["channel-arquivo-pt"]["results"][0]

        def identifier_map(result: dict) -> dict[str, str]:
            return {
                entry["scheme"]: entry["value"]
                for entry in result["identifiers"]
            }

        owner_ids = identifier_map(owner)
        archive_ids = identifier_map(archive)
        self.assertEqual(
            owner_ids["exact target block SHA-256"],
            archive_ids["exact target block SHA-256"],
        )
        self.assertEqual(
            "20230320222635",
            archive_ids["Arquivo.pt capture timestamp"],
        )
        self.assertEqual(
            "R5QLSPAJMRVEOB36UI4XEV2FTU3JXHYL",
            archive_ids["Arquivo.pt digest"],
        )
        self.assertEqual(
            "WEB-20230320222620763-p100.arquivo.pt.warc.gz",
            archive_ids["Arquivo.pt WARC file"],
        )
        self.assertIn("not publisher-origin authentication", archive["rationale"])
        self.assertIn("not authenticated source admission", owner["rationale"])

        access_request = json.loads(
            (
                REPO_ROOT
                / "ToS/source-witnesses/access-requests/public-ledger/"
                "nietzsche-source-ekgwb.access-request.json"
            ).read_text(encoding="utf-8")
        )
        self.assertIn(
            EKGWB_INSTITUTIONAL_DISCOVERY_PATH.relative_to(REPO_ROOT).as_posix(),
            access_request["material"]["discovery_refs"],
        )
        self.assertEqual("requested", access_request["requested_permissions"]["local_access"])
        self.assertEqual("draft-not-sent", access_request["request_status"])
        self.assertFalse(access_request["human_send_approval"])
        self.assertEqual(5, access_request["record_version"])

    def test_blind_pre_draft_contract_enforces_independent_lane_evidence(self) -> None:
        validator, _ = foundation._schema_validator(
            foundation.TRANSLATION_PRE_DRAFT_ANALYSIS_SCHEMA,
            REPO_ROOT,
        )

        human_packet = _synthetic_pre_draft_packet()
        self.assertEqual([], list(validator.iter_errors(human_packet)))

        contaminated_maker = copy.deepcopy(human_packet)
        contaminated_maker["maker"]["ai_assistance_used"] = True
        contaminated_maker["maker"]["model_refs"] = ["synthetic-model"]
        self.assertTrue(list(validator.iter_errors(contaminated_maker)))

        contaminated_finding = copy.deepcopy(human_packet)
        contaminated_finding["stages"][0]["findings"][0]["maker"]["maker_type"] = "model"
        self.assertTrue(list(validator.iter_errors(contaminated_finding)))

        comparator_visible = copy.deepcopy(human_packet)
        comparator_visible["blindness"]["recognized_comparator_visible"] = True
        self.assertTrue(list(validator.iter_errors(comparator_visible)))

        unsupported_etymology = copy.deepcopy(human_packet)
        unsupported_etymology["stages"][4]["findings"][0]["reference_entry_ids"] = []
        unsupported_etymology["stages"][4]["findings"][0]["citations"] = []
        self.assertTrue(list(validator.iter_errors(unsupported_etymology)))

        incompletely_frozen = copy.deepcopy(human_packet)
        incompletely_frozen["stages"][7]["status"] = "proposed"
        self.assertTrue(list(validator.iter_errors(incompletely_frozen)))

        ai_packet = _synthetic_pre_draft_packet(lane="ai_only")
        self.assertEqual([], list(validator.iter_errors(ai_packet)))

        human_edited_ai = copy.deepcopy(ai_packet)
        human_edited_ai["maker"]["human_editing_used"] = True
        self.assertTrue(list(validator.iter_errors(human_edited_ai)))

    def test_translation_packet_enforces_blind_lifecycle_and_lane_inputs(self) -> None:
        validator, _ = foundation._schema_validator(
            foundation.TRANSLATION_PACKET_SCHEMA,
            REPO_ROOT,
        )

        preparing = _synthetic_translation_packet()
        self.assertEqual([], list(validator.iter_errors(preparing)))

        frozen = _freeze_synthetic_translation_packet(copy.deepcopy(preparing))
        self.assertEqual([], list(validator.iter_errors(frozen)))

        wrong_analysis_lane = copy.deepcopy(frozen)
        wrong_analysis_lane["drafts"][0]["pre_draft_inputs"][0]["lane"] = "ai_only"
        self.assertTrue(list(validator.iter_errors(wrong_analysis_lane)))

        contaminated_human = copy.deepcopy(frozen)
        contaminated_human["drafts"][0]["maker"]["ai_assistance_used"] = True
        contaminated_human["drafts"][0]["maker"]["model_refs"] = ["synthetic-model"]
        self.assertTrue(list(validator.iter_errors(contaminated_human)))

        human_edited_ai = copy.deepcopy(frozen)
        human_edited_ai["drafts"][1]["maker"]["human_editing_used"] = True
        self.assertTrue(list(validator.iter_errors(human_edited_ai)))

        one_machine_alternative = copy.deepcopy(frozen)
        one_machine_alternative["drafts"] = [
            draft
            for draft in one_machine_alternative["drafts"]
            if draft["draft_id"] != "tos-translation-draft.synthetic-alternative-2"
        ]
        self.assertTrue(list(validator.iter_errors(one_machine_alternative)))

        mixed_without_independent_ai = copy.deepcopy(frozen)
        mixed_without_independent_ai["drafts"][-1]["input_drafts"] = [
            mixed_without_independent_ai["drafts"][-1]["input_drafts"][0]
        ]
        self.assertTrue(list(validator.iter_errors(mixed_without_independent_ai)))

        revealed = _reveal_synthetic_comparator(copy.deepcopy(frozen))
        self.assertEqual([], list(validator.iter_errors(revealed)))

        premature_reveal = copy.deepcopy(revealed)
        premature_reveal["packet_status"] = "blind-drafts-frozen"
        self.assertTrue(list(validator.iter_errors(premature_reveal)))

        comparator_as_ground_truth = copy.deepcopy(revealed)
        comparator_as_ground_truth["recognized_comparator"][
            "recognized_translation_is_ground_truth"
        ] = True
        self.assertTrue(list(validator.iter_errors(comparator_as_ground_truth)))

        comparison = _freeze_synthetic_comparison(copy.deepcopy(revealed))
        self.assertEqual([], list(validator.iter_errors(comparison)))

        incomplete_change_ledger = copy.deepcopy(comparison)
        incomplete_change_ledger["post_reveal_changes"].pop()
        self.assertTrue(list(validator.iter_errors(incomplete_change_ledger)))

    def test_semantic_ladder_requires_real_human_sign_gate_before_graph(self) -> None:
        validator, _ = foundation._schema_validator(
            foundation.SEMANTIC_LADDER_PACKET_SCHEMA,
            REPO_ROOT,
        )

        awaiting_human = _synthetic_semantic_packet()
        self.assertEqual([], list(validator.iter_errors(awaiting_human)))
        self.assertEqual(
            [],
            foundation._semantic_ladder_identity_issues(awaiting_human),
        )

        premature_relations = copy.deepcopy(awaiting_human)
        premature_relations["stages"][11] = _synthetic_semantic_stage(
            "relations_between_signs",
            status="proposed",
            body={"claim_refs": ["tos.claim.synthetic-premature-relation"]},
            maker=_synthetic_semantic_maker(),
        )
        self.assertTrue(list(validator.iter_errors(premature_relations)))

        frequency_promoted = copy.deepcopy(awaiting_human)
        frequency_promoted["stages"][9]["body"]["frequency_only_basis"] = True
        self.assertTrue(list(validator.iter_errors(frequency_promoted)))

        accepted = _accept_synthetic_sign(copy.deepcopy(awaiting_human))
        self.assertEqual([], list(validator.iter_errors(accepted)))
        self.assertEqual(
            [],
            foundation._semantic_ladder_identity_issues(accepted),
        )

        mismatched_sign_identity = copy.deepcopy(accepted)
        mismatched_sign_identity["accepted_sign_ref"] = (
            "tos.sign.synthetic-different"
        )
        self.assertIn(
            "human sign decision identity differs from packet accepted_sign_ref",
            foundation._semantic_ladder_identity_issues(
                mismatched_sign_identity
            ),
        )

        simulated_human = copy.deepcopy(accepted)
        simulated_human["stages"][10]["maker"]["performed_by_real_human"] = False
        self.assertTrue(list(validator.iter_errors(simulated_human)))

        contradictory_decision = copy.deepcopy(accepted)
        contradictory_decision["stages"][10]["body"]["decision"] = "reject"
        self.assertTrue(list(validator.iter_errors(contradictory_decision)))

        validator_only_decision = copy.deepcopy(accepted)
        validator_only_decision["stages"][10]["body"][
            "frequency_was_not_sole_basis"
        ] = False
        self.assertTrue(list(validator.iter_errors(validator_only_decision)))

        graph_packet = _project_synthetic_semantic_graph(copy.deepcopy(accepted))
        self.assertEqual([], list(validator.iter_errors(graph_packet)))
        self.assertEqual(
            [],
            foundation._semantic_ladder_identity_issues(graph_packet),
        )

        missing_relation_result = copy.deepcopy(graph_packet)
        missing_relation_result["result"]["relation_refs"] = []
        self.assertIn(
            "relation identities are absent from packet result",
            foundation._semantic_ladder_identity_issues(
                missing_relation_result
            ),
        )

        skipped_counterreading = copy.deepcopy(graph_packet)
        skipped_counterreading["stages"][13] = _synthetic_semantic_stage(
            "competing_readings",
            status="blocked",
            blocker_refs=["counterreading-not-completed"],
        )
        self.assertTrue(list(validator.iter_errors(skipped_counterreading)))

        authoritative_projection = copy.deepcopy(graph_packet)
        authoritative_projection["stages"][14]["body"]["projection_is_authority"] = True
        self.assertTrue(list(validator.iter_errors(authoritative_projection)))

    def test_initial_sign_packet_observes_source_without_language_truth(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.SEMANTIC_LADDER_PACKET_SCHEMA,
            REPO_ROOT,
        )
        packet = json.loads(
            (GOLD_ROOT / "initial-sign-packet.v5.json").read_text(
                encoding="utf-8"
            )
        )

        self.assertEqual([], list(validator.iter_errors(packet)))
        self.assertEqual("observational-analysis", packet["packet_status"])
        self.assertEqual("satisfied", packet["task_specific_source_gate"]["gate_status"])
        self.assertEqual(
            "edition-attested",
            packet["task_specific_source_gate"]["source_reading_status"],
        )
        self.assertTrue(
            packet["task_specific_source_gate"]["source_observation_allowed"]
        )
        self.assertEqual(
            "blocked",
            packet["task_specific_source_gate"]["language_competence_status"],
        )
        self.assertFalse(
            packet["task_specific_source_gate"]["linguistic_claim_review_allowed"]
        )
        self.assertFalse(packet["source_forms"]["source_values_tracked"])
        self.assertIsNone(packet["candidate_ref"])
        self.assertIsNone(packet["accepted_sign_ref"])
        self.assertFalse(packet["assurance_policy"]["human_work_scheduled"])
        self.assertEqual(
            ["source-observed", "source-observed", "source-observed"],
            [stage["status"] for stage in packet["stages"][:3]],
        )
        self.assertTrue(
            all(stage["status"] == "blocked" for stage in packet["stages"][3:])
        )
        self.assertTrue(
            all(stage["source_return_verified"] for stage in packet["stages"][:3])
        )
        self.assertEqual(
            4,
            packet["stages"][1]["body"]["count"],
        )
        self.assertEqual(
            packet["stages"][0]["body"]["occurrence_refs"],
            packet["stages"][1]["body"]["occurrence_refs"],
        )
        self.assertEqual(
            packet["stages"][0]["body"]["occurrence_refs"],
            packet["stages"][2]["body"]["occurrence_refs"],
        )
        self.assertEqual(
            4,
            len(packet["stages"][0]["body"]["occurrence_refs"]),
        )
        self.assertTrue(
            all(stage["body"] == {} for stage in packet["stages"][3:])
        )
        self.assertFalse(packet["result"]["promotion_authorized"])

        tracked_source_form = copy.deepcopy(packet)
        tracked_source_form["source_forms"] = {
            "diplomatic_local_ref": "ToS/local-content/semantic/fabricated.txt",
            "diplomatic_sha256": "a" * 64,
            "normalized_local_ref": "ToS/local-content/semantic/fabricated.txt",
            "normalized_sha256": "a" * 64,
            "source_values_tracked": True,
        }
        self.assertTrue(list(validator.iter_errors(tracked_source_form)))

        fabricated_human_debt = copy.deepcopy(packet)
        fabricated_human_debt["assurance_policy"]["human_work_scheduled"] = True
        self.assertTrue(list(validator.iter_errors(fabricated_human_debt)))

        fabricated_language_review = copy.deepcopy(packet)
        fabricated_language_review["task_specific_source_gate"][
            "linguistic_claim_review_allowed"
        ] = True
        self.assertTrue(list(validator.iter_errors(fabricated_language_review)))

        fabricated_promotion = copy.deepcopy(packet)
        fabricated_promotion["packet_status"] = "manual-sign-accepted"
        fabricated_promotion["accepted_sign_ref"] = "tos.sign.fabricated"
        fabricated_promotion["result"]["promotion_authorized"] = True
        self.assertTrue(list(validator.iter_errors(fabricated_promotion)))

    def test_selected_form_recurrence_stays_source_only(self) -> None:
        plan_path = GOLD_ROOT / "semantic-source-recurrence-plan.v1.json"
        receipt_path = GOLD_ROOT / "semantic-source-recurrence-receipt.v1.json"
        provenance_path = (
            GOLD_ROOT / "provenance.semantic-source-recurrence-v1.jsonl"
        )
        packet_path = GOLD_ROOT / "initial-sign-packet.v5.json"
        plan = json.loads(plan_path.read_text(encoding="utf-8"))
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        packet = json.loads(packet_path.read_text(encoding="utf-8"))
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))

        expected_tuple = {
            "occurrence_count": 145,
            "part_range": 4,
            "section_range": 59,
            "page_range": 96,
            "part_dp_millionths": 111588,
            "maximum_part_share_millionths": 351724,
            "source_editorial_occurrence_count": 0,
            "unsectioned_occurrence_count": 0,
        }
        self.assertEqual("frozen-before-output", plan["status"])
        self.assertEqual(expected_tuple, plan["expected_tracked_recurrence_tuple"])
        self.assertEqual(expected_tuple, receipt["observed_tuple"])
        self.assertEqual(
            [17, 32, 45, 51],
            [row["occurrence_count"] for row in receipt["parts"]],
        )
        self.assertEqual(145, receipt["verification"]["raw_offset_return_count"])
        self.assertEqual(
            145,
            receipt["verification"]["raw_offset_return_match_count"],
        )
        self.assertTrue(receipt["verification"]["complete_occurrence_census"])
        self.assertTrue(
            receipt["verification"]["independent_part_size_aware_recalculation"]
        )
        self.assertFalse(receipt["content_exposure"]["tracked_exact_strings"])
        self.assertFalse(
            receipt["content_exposure"]["tracked_occurrence_positions"]
        )
        self.assertFalse(receipt["packet_effect"]["packet_changed"])
        self.assertFalse(receipt["packet_effect"]["ladder_stage_changed"])
        self.assertFalse(receipt["packet_effect"]["human_work_scheduled"])
        self.assertFalse(receipt["packet_effect"]["promotion_authorized"])
        self.assertEqual("observational-analysis", packet["packet_status"])
        self.assertTrue(
            all(stage["status"] == "blocked" for stage in packet["stages"][3:])
        )
        self.assertEqual(
            receipt["provenance_event_ref"],
            provenance["event_id"],
        )
        self.assertIn(
            {
                "ref": receipt["local_bundle"]["ref"],
                "role": "ignored-private-complete-raw-witness-recurrence-bundle",
                "sha256": receipt["local_bundle"]["sha256"],
            },
            provenance["outputs"],
        )
        encoded = json.dumps(receipt, ensure_ascii=False, sort_keys=True)
        for forbidden in (
            '"exact_form"',
            '"normalized_form"',
            '"occurrence_id"',
            '"text_node_path"',
            '"token_ordinal"',
            '"start_offset"',
            '"end_offset"',
            "/srv/",
            "/home/",
        ):
            self.assertNotIn(forbidden, encoded)

    def test_semantic_ladder_allows_proposals_without_fake_language_review(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.SEMANTIC_LADDER_PACKET_SCHEMA,
            REPO_ROOT,
        )
        packet = _synthetic_semantic_packet()
        gate = packet["task_specific_source_gate"]
        gate["language_competence_status"] = "blocked"
        gate["language_competence_evidence_refs"] = []
        gate["linguistic_claim_review_allowed"] = False
        packet["packet_status"] = "observational-analysis"
        packet["assurance_policy"]["human_work_scheduled"] = False

        self.assertEqual([], list(validator.iter_errors(packet)))

        fabricated_review = copy.deepcopy(packet)
        fabricated_review["stages"][3]["review_status"] = "accepted"
        self.assertTrue(list(validator.iter_errors(fabricated_review)))

        fabricated_human_acceptance = _accept_synthetic_sign(copy.deepcopy(packet))
        self.assertTrue(list(validator.iter_errors(fabricated_human_acceptance)))

    def test_discovery_access_and_server_boundaries_fail_closed(self) -> None:
        discovery_validator, _ = foundation._schema_validator(
            foundation.MATERIAL_DISCOVERY_SCHEMA,
            REPO_ROOT,
        )
        discovery = _synthetic_discovery_record()
        self.assertEqual([], list(discovery_validator.iter_errors(discovery)))
        self.assertEqual([], foundation._discovery_decision_issues(discovery))

        bypassed = copy.deepcopy(discovery)
        bypassed["technical_access_bypass_used"] = True
        self.assertTrue(list(discovery_validator.iter_errors(bypassed)))

        false_download = copy.deepcopy(discovery)
        false_download["channels"][0]["results"][0]["acquisition"]["downloaded"] = True
        self.assertTrue(list(discovery_validator.iter_errors(false_download)))

        missing_selection = copy.deepcopy(discovery)
        missing_selection["selected_result_ids"] = []
        self.assertIn(
            "selected_result_ids do not match results whose decision is select",
            foundation._discovery_decision_issues(missing_selection),
        )

        duplicate_result = copy.deepcopy(discovery)
        duplicate_result["channels"][0]["results"].append(
            copy.deepcopy(duplicate_result["channels"][0]["results"][0])
        )
        self.assertIn(
            "duplicate discovery result_id: tos-discovery-result.synthetic-1",
            foundation._discovery_decision_issues(duplicate_result),
        )

        access_validator, _ = foundation._schema_validator(
            foundation.ACCESS_REQUEST_SCHEMA,
            REPO_ROOT,
        )
        access_path = (
            REPO_ROOT
            / "ToS/source-witnesses/access-requests/public-ledger/"
            "nietzsche-woerterbuch.access-request.json"
        )
        access_record = json.loads(access_path.read_text(encoding="utf-8"))
        self.assertEqual([], list(access_validator.iter_errors(access_record)))
        self.assertEqual("draft-not-sent", access_record["request_status"])
        self.assertFalse(access_record["human_send_approval"])

        falsely_sent = copy.deepcopy(access_record)
        falsely_sent["request_status"] = "sent"
        self.assertTrue(list(access_validator.iter_errors(falsely_sent)))

        falsely_granted = copy.deepcopy(access_record)
        falsely_granted["request_status"] = "permission-granted"
        falsely_granted["human_send_approval"] = True
        falsely_granted["sent_at"] = "2026-07-23T01:00:00Z"
        falsely_granted["private_correspondence_ref"] = (
            "ToS/source-witnesses/access-requests/private/synthetic/request.eml"
        )
        self.assertTrue(list(access_validator.iter_errors(falsely_granted)))

        server_validator, _ = foundation._schema_validator(
            foundation.SERVER_IMPORT_SCHEMA,
            REPO_ROOT,
        )
        plan_paths = sorted(
            (REPO_ROOT / "ToS/source-witnesses/server-import/plans").glob("*.json")
        )
        manifest_paths = sorted(
            (REPO_ROOT / "ToS/source-witnesses").rglob("item.manifest.json")
        )
        self.assertEqual(len(manifest_paths), len(plan_paths))
        for plan_path in plan_paths:
            plan = json.loads(plan_path.read_text(encoding="utf-8"))
            self.assertEqual([], list(server_validator.iter_errors(plan)))
            self.assertEqual("blocked-rights", plan["server_import_status"])
            self.assertEqual("metadata-only", plan["access_class"])
            self.assertFalse(plan["payload_transfer_authorized"])
            self.assertFalse(plan["operator_transfer_approval"]["approved"])

        unauthorized_transfer = json.loads(plan_paths[0].read_text(encoding="utf-8"))
        unauthorized_transfer["payload_transfer_authorized"] = True
        self.assertTrue(list(server_validator.iter_errors(unauthorized_transfer)))

        unreviewed_import = json.loads(plan_paths[0].read_text(encoding="utf-8"))
        unreviewed_import["access_class"] = "controlled-research"
        unreviewed_import["payload_transfer_authorized"] = True
        unreviewed_import["operator_transfer_approval"] = {
            "approved": True,
            "approved_by_real_human": True,
            "approved_at": "2026-07-23T01:00:00Z",
            "approval_ref": "synthetic-human-approval",
        }
        unreviewed_import["server_import_status"] = "imported"
        unreviewed_import["server_receipt_refs"] = ["synthetic-server-receipt"]
        self.assertTrue(list(server_validator.iter_errors(unreviewed_import)))

        private_route = REPO_ROOT / "ToS/source-witnesses/access-requests/private/README.md"
        private_example = REPO_ROOT / "ToS/source-witnesses/access-requests/private/request/message.eml"
        self.assertFalse(foundation._git_ignored(REPO_ROOT, private_route))
        self.assertTrue(foundation._git_ignored(REPO_ROOT, private_example))

    def test_dta_open_license_remains_separate_from_local_transfer(self) -> None:
        rights_validator, _ = foundation._schema_validator(
            foundation.RIGHTS_SCHEMA,
            REPO_ROOT,
        )
        server_validator, _ = foundation._schema_validator(
            foundation.SERVER_IMPORT_SCHEMA,
            REPO_ROOT,
        )
        item_and_plan_paths = (
            (
                "expressions/de-schmeitzner-1883-part-1/editions/"
                "chemnitz-schmeitzner-1883-part-1/items/"
                "dta-sbb-corrected-tei-p5",
                "dta-zarathustra-part-1-tei.server-import.json",
            ),
            (
                "expressions/de-schmeitzner-1883-part-2/editions/"
                "chemnitz-schmeitzner-1883-part-2/items/"
                "dta-sbb-corrected-tei-p5",
                "dta-zarathustra-part-2-tei.server-import.json",
            ),
            (
                "expressions/de-schmeitzner-1884-part-3/editions/"
                "chemnitz-schmeitzner-1884-part-3/items/"
                "dta-sbb-corrected-tei-p5",
                "dta-zarathustra-part-3-tei.server-import.json",
            ),
            (
                "expressions/de-naumann-1891-part-4/editions/"
                "leipzig-naumann-1891-part-4/items/"
                "dta-sub-goettingen-corrected-tei-p5",
                "dta-zarathustra-part-4-tei.server-import.json",
            ),
        )
        work_root = (
            REPO_ROOT
            / "ToS/source-witnesses/works/friedrich-nietzsche/"
            "also-sprach-zarathustra"
        )
        prohibited_derivatives = {
            "ocr",
            "transcription",
            "page_images",
            "snippets",
            "embeddings",
            "alignments",
            "translations",
            "annotations",
        }

        for item_relative, plan_name in item_and_plan_paths:
            rights = json.loads(
                (work_root / item_relative / "rights.json").read_text(
                    encoding="utf-8"
                )
            )
            plan = json.loads(
                (
                    REPO_ROOT
                    / "ToS/source-witnesses/server-import/plans"
                    / plan_name
                ).read_text(encoding="utf-8")
            )

            self.assertEqual([], list(rights_validator.iter_errors(rights)))
            self.assertEqual("licensed", rights["assessment_status"])
            self.assertEqual(
                "https://creativecommons.org/licenses/by-sa/4.0/",
                rights["license_uri"],
            )
            self.assertEqual(["DE", "US"], rights["jurisdictions_reviewed"])
            self.assertEqual("local_only", rights["visibility"])
            self.assertEqual("not_authorized", rights["redistribution_posture"])
            self.assertEqual("local_research_only", rights["derivative_posture"])
            self.assertEqual("unreviewed", rights["review_status"])
            self.assertEqual(3, rights["record_version"])

            layers = {
                layer["layer_role"]: layer
                for layer in rights["layer_assessments"]
            }
            self.assertEqual(
                {"original_work", "edition_presentation", "annotation", "metadata"},
                set(layers),
            )
            self.assertEqual(
                "public_domain_reviewed",
                layers["original_work"]["assessment_status"],
            )
            self.assertEqual(
                "public_domain_reviewed",
                layers["edition_presentation"]["assessment_status"],
            )
            self.assertIn("§104A", layers["original_work"]["rationale"])
            self.assertIn("§104A", layers["edition_presentation"]["rationale"])
            self.assertEqual("licensed", layers["annotation"]["assessment_status"])
            self.assertEqual("licensed", layers["metadata"]["assessment_status"])
            self.assertTrue(
                any(
                    "former CC BY-NC 3.0" in restriction
                    for restriction in layers["annotation"]["restrictions"]
                )
            )

            self.assertEqual([], list(server_validator.iter_errors(plan)))
            self.assertEqual("open-licensed", plan["rights_policy"]["assessment_status"])
            self.assertEqual("unreviewed", plan["rights_policy"]["review_status"])
            self.assertEqual("metadata-only", plan["access_class"])
            self.assertEqual("blocked-rights", plan["server_import_status"])
            self.assertEqual("metadata-only", plan["publication_status"])
            self.assertFalse(plan["payload_transfer_authorized"])
            self.assertFalse(plan["operator_transfer_approval"]["approved"])
            self.assertEqual(3, plan["contract_version"])
            self.assertIn(
                "https://www.copyright.gov/title17/92chap1.html#104a",
                plan["rights_policy"]["permission_or_license_refs"],
            )
            for derivative in prohibited_derivatives:
                self.assertEqual(
                    "prohibited",
                    plan["allowed_derivatives"][derivative]["state"],
                )

        research = (
            REPO_ROOT
            / "ToS/research-packets/foundation-laboratory-2026-07/"
            "DTA_ZARATHUSTRA_PARTS_1_4_LAYERED_RIGHTS_ASSESSMENT.md"
        ).read_text(encoding="utf-8")
        ordered_sections = (
            "## Classical and official documentation",
            "## Established scholarship and practice",
            "## Fresh and currently relevant checks",
            "## General web search, last",
        )
        positions = [research.index(section) for section in ordered_sections]
        self.assertEqual(sorted(positions), positions)
        self.assertIn("distinct plain-text form", research)
        self.assertIn("facsimiles are separate", research)
        self.assertIn("domestic pre-1931", research)
        self.assertIn("17 U.S.C. §104A", research)
        self.assertIn(
            "local research payloads stay local",
            " ".join(research.split()),
        )

    def test_provenance_supersession_requires_one_current_event(self) -> None:
        base_id = "tos.event.synthetic.base"
        current_id = "tos.event.synthetic.current"
        base = {"event_id": base_id, "supersedes_event_ref": None}
        current = {
            "event_id": current_id,
            "supersedes_event_ref": base_id,
        }
        issues: list[foundation.Issue] = []
        resolved = foundation._latest_event_in_supersession_lineage(
            {base_id: base, current_id: current},
            base_id,
            location="synthetic",
            issues=issues,
        )
        self.assertIs(current, resolved)
        self.assertEqual([], issues)

        competing_id = "tos.event.synthetic.competing"
        competing = {
            "event_id": competing_id,
            "supersedes_event_ref": base_id,
        }
        resolved = foundation._latest_event_in_supersession_lineage(
            {base_id: base, current_id: current, competing_id: competing},
            base_id,
            location="synthetic",
            issues=issues,
        )
        self.assertIsNone(resolved)
        self.assertIn(
            ("synthetic", f"ambiguous provenance supersession from {base_id}"),
            issues,
        )

    def test_jenseits_1886_witness_opens_transfer_soil_without_acceptance(self) -> None:
        manifest = json.loads(
            (JENSEITS_1886_ITEM_ROOT / "item.manifest.json").read_text(
                encoding="utf-8"
            )
        )
        rights = json.loads(
            (JENSEITS_1886_ITEM_ROOT / "rights.json").read_text(encoding="utf-8")
        )
        inventory = json.loads(
            (JENSEITS_1886_ITEM_ROOT / "resource-inventory.json").read_text(
                encoding="utf-8"
            )
        )
        discovery = json.loads(
            JENSEITS_1886_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        server_plan = json.loads(
            JENSEITS_1886_SERVER_PLAN_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual("digitized_physical_copy", manifest["item_kind"])
        self.assertEqual("local_only", manifest["visibility"])
        self.assertEqual(
            {
                "6ae316c90f958d09045fea27b2430b86623ebb85f8a27146099d028775cdc80a",
                "6227d4a797fb27608386733a9d71fd06c049e5458c9e0687cb582f0c31177be0",
                "ba8f4c91a317a3de03ab1f318860aaba6837d979e1ec99365e6d13def7db5a34",
            },
            {
                payload_file["sha256"]
                for payload_file in manifest["payload_files"]
            },
        )
        self.assertTrue(
            all(
                foundation._git_ignored(
                    REPO_ROOT,
                    JENSEITS_1886_ITEM_ROOT
                    / payload_file["relative_path"],
                )
                for payload_file in manifest["payload_files"]
            )
        )

        self.assertEqual(
            "http://creativecommons.org/publicdomain/mark/1.0/",
            rights["rights_statement_uri"],
        )
        self.assertEqual("conflicting_evidence", rights["assessment_status"])
        self.assertEqual(["DE", "US"], rights["jurisdictions_reviewed"])
        self.assertEqual("unreviewed", rights["review_status"])
        self.assertEqual("not_authorized", rights["redistribution_posture"])
        self.assertEqual("local_research_only", rights["derivative_posture"])
        self.assertEqual(4, rights["record_version"])
        self.assertIn(
            "the operator-held PDF, DjVu XML, and ABBYY XML gzip remain local and are not future-site uploads",
            rights["restrictions"],
        )
        layers = {
            layer["layer_id"].rsplit(".layer.", 1)[1]: layer
            for layer in rights["layer_assessments"]
        }
        self.assertEqual(
            {
                "original-work",
                "edition-presentation",
                "faithful-page-scan",
                "google-generated-cover",
                "harvard-holding-furniture",
                "automatic-historical-ocr-text",
                "ocr-coordinate-xml",
                "pdf-and-derivative-package",
                "metadata",
            },
            set(layers),
        )
        for layer_name in {
            "original-work",
            "edition-presentation",
            "faithful-page-scan",
            "automatic-historical-ocr-text",
        }:
            self.assertEqual(
                "public_domain_reviewed",
                layers[layer_name]["assessment_status"],
            )
        for layer_name in {
            "google-generated-cover",
            "harvard-holding-furniture",
            "ocr-coordinate-xml",
            "pdf-and-derivative-package",
            "metadata",
        }:
            self.assertEqual(
                "copyright_undetermined",
                layers[layer_name]["assessment_status"],
            )
        self.assertEqual(
            {
                "tos.file.sha256.6ae316c90f958d09045fea27b2430b86623ebb85f8a27146099d028775cdc80a",
                "tos.file.sha256.6227d4a797fb27608386733a9d71fd06c049e5458c9e0687cb582f0c31177be0",
                "tos.file.sha256.ba8f4c91a317a3de03ab1f318860aaba6837d979e1ec99365e6d13def7db5a34",
            },
            set(rights["scope_refs"]) - {manifest["item_id"]},
        )

        self.assertFalse(inventory["source_text_included"])
        files_by_profile = {
            file_inventory["profile"]: file_inventory
            for file_inventory in inventory["files"]
        }
        self.assertEqual(
            {
                "pdf_pages_v1",
                "djvu_xml_pages_v1",
                "abbyy_xml_pages_v1",
            },
            set(files_by_profile),
        )
        self.assertTrue(
            all(
                file_inventory["summary"]["page_count"] == 274
                for file_inventory in files_by_profile.values()
            )
        )
        self.assertEqual(
            820,
            files_by_profile["pdf_pages_v1"]["summary"][
                "image_resource_count"
            ],
        )
        self.assertEqual(
            62700,
            files_by_profile["djvu_xml_pages_v1"]["summary"]["word_count"],
        )
        self.assertEqual(
            61704,
            files_by_profile["abbyy_xml_pages_v1"]["summary"]["word_count"],
        )

        self.assertEqual(
            [
                "channel-dnb-jenseits-work-authority",
                "channel-dta-nietzsche-author-corpus",
                "channel-textgrid-jenseits-editions",
                "channel-internet-archive-jenseits",
                "channel-google-books-jenseits-id",
            ],
            [channel["channel_id"] for channel in discovery["channels"]],
        )
        self.assertEqual(
            ["tos-discovery-result.ia-bub-gb-yiuraaaayaaaj"],
            discovery["selected_result_ids"],
        )
        self.assertEqual([], discovery["channels"][-1]["results"])
        self.assertFalse(discovery["technical_access_bypass_used"])

        self.assertEqual("metadata-only", server_plan["access_class"])
        self.assertEqual("blocked-rights", server_plan["server_import_status"])
        self.assertFalse(server_plan["payload_transfer_authorized"])
        self.assertEqual(3, len(server_plan["payload_files"]))
        self.assertEqual("rights-unknown", server_plan["rights_policy"]["assessment_status"])
        self.assertEqual(
            ["DE", "US"],
            server_plan["rights_policy"]["jurisdictions_reviewed"],
        )
        self.assertEqual(4, server_plan["contract_version"])
        self.assertEqual(
            [
                "tos.event.server-import-plan.jenseits-naumann-1886."
                "layered-rights.2026-08-02"
            ],
            server_plan["provenance_event_refs"],
        )
        self.assertEqual(
            "prohibited",
            server_plan["allowed_derivatives"]["transcription"]["state"],
        )

    def test_jenseits_authorial_route_keeps_regions_and_witnesses_distinct(
        self,
    ) -> None:
        discovery = json.loads(
            JENSEITS_AUTHORIAL_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        work = json.loads(
            (
                REPO_ROOT
                / "ToS/source-witnesses/works/friedrich-nietzsche/"
                "jenseits-von-gut-und-boese/work.json"
            ).read_text(encoding="utf-8")
        )

        self.assertEqual(
            [
                "channel-gsa-ores-jenseits-authorial-route",
                "channel-haab-jenseits-correction-copies",
                "channel-nietzsche-source-jenseits-critical-route",
                "channel-erara-jenseits-first-print",
                "channel-established-jenseits-genesis",
                "channel-fresh-jenseits-textual-genetics",
                "channel-general-web-jenseits-authorial-route",
            ],
            [channel["channel_id"] for channel in discovery["channels"]],
        )
        self.assertEqual(13, len(discovery["selected_result_ids"]))
        self.assertEqual([], discovery["rejected_result_ids"])
        self.assertFalse(discovery["technical_access_bypass_used"])
        self.assertEqual([], discovery["channels"][-1]["results"])

        results = {
            result["result_id"]: result
            for channel in discovery["channels"]
            for result in channel["results"]
        }
        self.assertEqual(
            "select",
            results["tos-discovery-result.gsa-71-26-jenseits-d18"][
                "decision"
            ],
        )
        self.assertIn(
            "region",
            results["tos-discovery-result.gsa-jenseits-w-i-3-8-family"][
                "rationale"
            ],
        )
        self.assertIn(
            "partial",
            results["tos-discovery-result.haab-c4615-jenseits-correction-proof"][
                "rationale"
            ],
        )
        self.assertEqual(
            "unknown",
            results["tos-discovery-result.dfga-jenseits-d18"]["availability"],
        )
        self.assertTrue(
            all(
                not result["acquisition"]["downloaded"]
                and result["snapshot"]["state"] == "not-captured"
                for result in results.values()
            )
        )
        self.assertEqual(5, work["record_version"])
        self.assertIn(
            "ToS/source-witnesses/discovery/runs/"
            "jenseits-authorial-witness-route.2026-07-30.v1.json",
            work["source_refs"],
        )
        self.assertIn("no remote Item", work["notes"])
        self.assertIn("semantic authority", work["notes"])

        route = (
            REPO_ROOT
            / "ToS/research-packets/foundation-laboratory-2026-07/"
            "JENSEITS_AUTHORIAL_WITNESS_ROUTE.md"
        ).read_text(encoding="utf-8")
        self.assertIn(
            "## 6. First bounded textual-genetic witness route",
            route,
        )
        self.assertIn(
            "outside the frozen\n  golden-kernel transfer plan",
            route,
        )
        self.assertNotIn("First bounded A/B/C candidate", route)
        self.assertNotIn("§22 A/B/C candidate", route)

    def test_genealogie_1892_witness_advances_source_not_content(self) -> None:
        manifest = json.loads(
            (GENEALOGIE_1892_ITEM_ROOT / "item.manifest.json").read_text(
                encoding="utf-8"
            )
        )
        rights = json.loads(
            (GENEALOGIE_1892_ITEM_ROOT / "rights.json").read_text(
                encoding="utf-8"
            )
        )
        inventory = json.loads(
            (GENEALOGIE_1892_ITEM_ROOT / "resource-inventory.json").read_text(
                encoding="utf-8"
            )
        )
        source_snapshot = json.loads(
            (GENEALOGIE_1892_ITEM_ROOT / "source-metadata-snapshot.json").read_text(
                encoding="utf-8"
            )
        )
        discovery = json.loads(
            GENEALOGIE_1892_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        server_plan = json.loads(
            (
                REPO_ROOT
                / "ToS/source-witnesses/server-import/plans/"
                "genealogie-naumann-1892-wikimedia-commons-unc-scan-pdf."
                "server-import.json"
            ).read_text(encoding="utf-8")
        )

        self.assertEqual("digitized_physical_copy", manifest["item_kind"])
        self.assertEqual("local_gitignored_payload", manifest["storage_posture"])
        self.assertEqual("local_only", manifest["visibility"])
        self.assertEqual(1, len(manifest["payload_files"]))
        payload_file = manifest["payload_files"][0]
        self.assertEqual(13227300, payload_file["byte_size"])
        self.assertEqual(
            "5705dbc4f32faa924919fd533962c931e92462d72dab5183610eb68adeecac03",
            payload_file["sha256"],
        )
        self.assertTrue(
            foundation._git_ignored(
                REPO_ROOT,
                GENEALOGIE_1892_ITEM_ROOT / payload_file["relative_path"],
            )
        )

        self.assertEqual(
            "https://commons.wikimedia.org/wiki/Template:PD-US-expired",
            rights["rights_statement_uri"],
        )
        self.assertEqual("conflicting_evidence", rights["assessment_status"])
        self.assertEqual("unreviewed", rights["review_status"])
        self.assertEqual(["DE", "US"], rights["jurisdictions_reviewed"])
        self.assertEqual("not_authorized", rights["redistribution_posture"])
        self.assertEqual("local_research_only", rights["derivative_posture"])
        self.assertEqual("local_only", rights["visibility"])
        self.assertEqual(2, rights["record_version"])

        layers = {
            layer["layer_id"].rsplit(".layer.", 1)[1]: layer
            for layer in rights["layer_assessments"]
        }
        self.assertEqual(11, len(layers))
        for layer_id in (
            "original-work",
            "edition-presentation",
            "faithful-historical-page-scan",
            "automatic-historical-ocr-text",
        ):
            self.assertEqual(
                "public_domain_reviewed",
                layers[layer_id]["assessment_status"],
            )
        for layer_id in (
            "physical-binding-and-marbled-cover",
            "unc-holding-furniture",
            "ocr-coordinate-and-layout",
            "pdf-derivative-package",
            "internet-archive-lineage-metadata",
        ):
            self.assertEqual(
                "copyright_undetermined",
                layers[layer_id]["assessment_status"],
            )
        self.assertEqual(
            "https://creativecommons.org/publicdomain/zero/1.0/",
            layers["commons-structured-file-metadata"]["license_uri"],
        )
        self.assertEqual(
            "https://creativecommons.org/licenses/by-sa/4.0/",
            layers["commons-unstructured-description"]["license_uri"],
        )

        self.assertFalse(inventory["source_text_included"])
        self.assertEqual(1, len(inventory["files"]))
        inventory_file = inventory["files"][0]
        self.assertEqual("pdf_pages_v1", inventory_file["profile"])
        self.assertEqual(208, inventory_file["summary"]["page_count"])
        self.assertEqual(624, inventory_file["summary"]["image_resource_count"])
        self.assertEqual(6, inventory_file["summary"]["distinct_page_geometry_count"])
        self.assertEqual(208, len(inventory_file["resources"]))

        commons = source_snapshot["wikimedia_commons_record"]
        archive = source_snapshot["internet_archive_source_lineage"]
        self.assertEqual(95720779, commons["page_id"])
        self.assertFalse(commons["copyrighted"])
        self.assertTrue(commons["local_sha1_match"])
        self.assertTrue(
            archive["current_pdf_differs_from_acquired_commons_revision"]
        )
        self.assertFalse(
            source_snapshot["source_visible_scan"]["human_repeat_performed"]
        )

        self.assertEqual(
            [
                "channel-dnb-genealogie-work-authority",
                "channel-dta-genealogie-author-corpus",
                "channel-textgrid-genealogie-corpus",
                "channel-internet-archive-genealogie",
                "channel-wikimedia-commons-genealogie",
                "channel-general-web-genealogie",
            ],
            [channel["channel_id"] for channel in discovery["channels"]],
        )
        self.assertEqual(
            ["tos-discovery-result.wikimedia-commons-genealogie-naumann-1892"],
            discovery["selected_result_ids"],
        )
        self.assertFalse(discovery["technical_access_bypass_used"])

        self.assertEqual("metadata-only", server_plan["access_class"])
        self.assertEqual("blocked-rights", server_plan["server_import_status"])
        self.assertEqual(["DE", "US"], server_plan["rights_policy"]["jurisdictions_reviewed"])
        self.assertEqual(2, server_plan["contract_version"])
        self.assertFalse(server_plan["payload_transfer_authorized"])
        self.assertFalse(server_plan["operator_transfer_approval"]["approved"])
        self.assertEqual(
            "prohibited",
            server_plan["allowed_derivatives"]["transcription"]["state"],
        )
        self.assertEqual(
            "conditional",
            server_plan["allowed_derivatives"]["graph_projection"]["state"],
        )

        rights_research = GENEALOGIE_1892_LAYERED_RIGHTS_RESEARCH_PATH.read_text(
            encoding="utf-8"
        )
        for heading in (
            "## Classical and official documentation",
            "## Established scholarship, cases, and institutional practice",
            "## Fresh and currently relevant checks",
            "## General web search, last",
        ):
            self.assertIn(heading, rights_research)
        self.assertIn("conflicting_evidence", rights_research)
        self.assertIn("current Internet Archive PDF", rights_research)

        catalog_root = REPO_ROOT / "ToS/source-witnesses/catalog"
        catalog_ids = {
            record["record_id"]
            for catalog_name in (
                "works.jsonl",
                "expressions.jsonl",
                "editions.jsonl",
                "items.jsonl",
            )
            for line in (catalog_root / catalog_name)
            .read_text(encoding="utf-8")
            .splitlines()
            if line
            for record in (json.loads(line),)
        }
        self.assertTrue(
            {
                "tos.work.friedrich-nietzsche.zur-genealogie-der-moral",
                "tos.expression.friedrich-nietzsche.zur-genealogie-der-moral."
                "de-naumann-1892-second",
                "tos.edition.friedrich-nietzsche.zur-genealogie-der-moral."
                "leipzig-c-g-naumann-1892-second",
                "tos.item.friedrich-nietzsche.zur-genealogie-der-moral."
                "de-naumann-1892-second.wikimedia-commons-unc-scan-pdf",
            }.issubset(catalog_ids)
        )

    def test_genealogie_authorial_route_keeps_document_stages_distinct(
        self,
    ) -> None:
        discovery = json.loads(
            GENEALOGIE_AUTHORIAL_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        work = json.loads(
            (
                REPO_ROOT
                / "ToS/source-witnesses/works/friedrich-nietzsche/"
                "zur-genealogie-der-moral/work.json"
            ).read_text(encoding="utf-8")
        )

        self.assertEqual(
            [
                "channel-gsa-ores-genealogie-authorial-route",
                "channel-haab-genealogie-correction-copies",
                "channel-basel-genealogie-documentary-edition",
                "channel-nietzsche-source-genealogie-critical-route",
                "channel-established-genealogie-genetics",
                "channel-fresh-genealogie-research",
                "channel-general-web-genealogie-authorial-route",
            ],
            [channel["channel_id"] for channel in discovery["channels"]],
        )
        self.assertEqual(11, len(discovery["selected_result_ids"]))
        self.assertEqual(
            [
                "tos-discovery-result."
                "general-web-genealogie-modern-original-reprints"
            ],
            discovery["rejected_result_ids"],
        )
        self.assertFalse(discovery["technical_access_bypass_used"])

        results = {
            result["result_id"]: result
            for channel in discovery["channels"]
            for result in channel["results"]
        }
        self.assertEqual(
            "select",
            results["tos-discovery-result.gsa-71-27-1-genealogie-d20a"][
                "decision"
            ],
        )
        self.assertEqual(
            "select",
            results["tos-discovery-result.gsa-71-27-2-genealogie-d20b"][
                "decision"
            ],
        )
        self.assertIn(
            "whole-notebook",
            results[
                "tos-discovery-result.gsa-71-157-genealogie-wii1-regions"
            ]["rationale"],
        )
        self.assertIn(
            "partial",
            results[
                "tos-discovery-result.haab-c4616-genealogie-correction-sheets"
            ]["rationale"],
        )
        self.assertEqual(
            "unknown",
            results[
                "tos-discovery-result."
                "nietzsche-source-genealogie-d20-critical-routes"
            ]["availability"],
        )
        self.assertTrue(
            all(
                not result["acquisition"]["downloaded"]
                and result["snapshot"]["state"] == "not-captured"
                for result in results.values()
            )
        )

        self.assertEqual(6, work["record_version"])
        self.assertIn(
            "ToS/source-witnesses/discovery/runs/"
            "genealogie-authorial-witness-route.2026-07-30.v1.json",
            work["source_refs"],
        )
        self.assertIn("partial K 11/C 4616", work["notes"])
        self.assertIn("rather than a historical witness or ToS Item", work["notes"])
        self.assertEqual("no_equivalence_claim", work["same_as_posture"])

        provenance_events = [
            json.loads(line)
            for line in (
                GENEALOGIE_1892_ITEM_ROOT / "provenance.jsonl"
            ).read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        discovery_event = next(
            event
            for event in provenance_events
            if event["event_id"]
            == "tos.event.discovery.genealogie."
            "authorial-witness-route.2026-07-30"
        )
        self.assertEqual(
            "tos.event.discovery.genealogie."
            "authorial-witness-route.2026-07-30",
            discovery_event["event_id"],
        )
        self.assertFalse(
            discovery_event["method"]["configuration"][
                "human_task_created"
            ]
        )
        self.assertEqual(
            0,
            discovery_event["method"]["configuration"][
                "semantic_objects_created"
            ],
        )

        route = (
            REPO_ROOT
            / "ToS/research-packets/foundation-laboratory-2026-07/"
            "GENEALOGIE_AUTHORIAL_WITNESS_ROUTE.md"
        ).read_text(encoding="utf-8")
        self.assertIn("D 20a", route)
        self.assertIn("D 20b", route)
        self.assertIn("K 11 / HAAB C 4616", route)
        self.assertIn("Naumann 1887 first print E 40", route)
        self.assertIn("remote scholarly representation", route)
        self.assertIn("deliberately named rather than called A/B/C", route)
        self.assertNotIn("Genealogie A/B/C witness route", route)

    def test_antichrist_1906_witness_advances_source_not_content(self) -> None:
        manifest = json.loads(
            (ANTICHRIST_1906_ITEM_ROOT / "item.manifest.json").read_text(
                encoding="utf-8"
            )
        )
        rights = json.loads(
            (ANTICHRIST_1906_ITEM_ROOT / "rights.json").read_text(
                encoding="utf-8"
            )
        )
        inventory = json.loads(
            (ANTICHRIST_1906_ITEM_ROOT / "resource-inventory.json").read_text(
                encoding="utf-8"
            )
        )
        source_snapshot = json.loads(
            (ANTICHRIST_1906_ITEM_ROOT / "source-metadata-snapshot.json").read_text(
                encoding="utf-8"
            )
        )
        boundary_map = json.loads(
            (
                ANTICHRIST_1906_COLLECTION_ROOT
                / "structure/work-boundaries/work-boundary-map.json"
            ).read_text(encoding="utf-8")
        )
        discovery = json.loads(
            ANTICHRIST_1906_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        server_plan = json.loads(
            (
                REPO_ROOT
                / "ToS/source-witnesses/server-import/plans/"
                "der-antichrist-naumann-1906-wikimedia-commons-stanford-"
                "scan-djvu.server-import.json"
            ).read_text(encoding="utf-8")
        )

        self.assertEqual("digitized_physical_copy", manifest["item_kind"])
        self.assertEqual("local_gitignored_payload", manifest["storage_posture"])
        self.assertEqual("local_only", manifest["visibility"])
        self.assertEqual(1, len(manifest["payload_files"]))
        payload_file = manifest["payload_files"][0]
        self.assertEqual(24324176, payload_file["byte_size"])
        self.assertEqual(
            "8f61aaecd55339fc3ba11eca24fbeef85e953d1a262200b36e59d5fbc545ca9d",
            payload_file["sha256"],
        )
        self.assertTrue(
            foundation._git_ignored(
                REPO_ROOT,
                ANTICHRIST_1906_ITEM_ROOT / payload_file["relative_path"],
            )
        )

        self.assertEqual(
            "https://commons.wikimedia.org/wiki/Template:PD-old-auto-1923",
            rights["rights_statement_uri"],
        )
        self.assertEqual("conflicting_evidence", rights["assessment_status"])
        self.assertEqual("unreviewed", rights["review_status"])
        self.assertEqual(["DE", "US"], rights["jurisdictions_reviewed"])
        self.assertEqual("not_authorized", rights["redistribution_posture"])
        self.assertEqual("local_research_only", rights["derivative_posture"])
        self.assertEqual("local_only", rights["visibility"])
        self.assertEqual(4, rights["record_version"])

        layers = {
            layer["layer_id"].rsplit(".layer.", 1)[1]: layer
            for layer in rights["layer_assessments"]
        }
        self.assertEqual(11, len(layers))
        for layer_id in (
            "original-work",
            "edition-presentation",
            "faithful-historical-page-scan",
            "automatic-historical-ocr-text",
        ):
            self.assertEqual(
                "public_domain_reviewed",
                layers[layer_id]["assessment_status"],
            )
        self.assertEqual(
            "conflicting_evidence",
            layers["archive-editorial-and-added-matter"]["assessment_status"],
        )
        self.assertIn(
            "§104A(h)(6)(B)",
            layers["original-work"]["term"]["basis"],
        )
        self.assertIn(
            "provider's pre-1931 statement is evidence",
            layers["original-work"]["term"]["basis"],
        )
        self.assertIn(
            "2001-12-31",
            layers["original-work"]["term"]["basis"],
        )
        self.assertIn(
            "§104A(h)(6)(B)",
            layers["edition-presentation"]["term"]["basis"],
        )
        self.assertIn(
            "2001-12-31",
            layers["edition-presentation"]["term"]["basis"],
        )
        self.assertIn(
            "2001-12-31",
            layers["archive-editorial-and-added-matter"]["term"]["basis"],
        )
        self.assertNotIn(
            "inside the current United States pre-1931 public-domain boundary",
            layers["original-work"]["term"]["basis"],
        )
        self.assertNotIn(
            "inside the current United States pre-1931 public-domain boundary",
            layers["edition-presentation"]["term"]["basis"],
        )
        for layer_id in (
            "stanford-binding-and-holding-furniture",
            "ocr-coordinate-and-layout",
            "djvu-derivative-package",
            "internet-archive-lineage-metadata",
        ):
            self.assertEqual(
                "copyright_undetermined",
                layers[layer_id]["assessment_status"],
            )
        self.assertEqual(
            "https://creativecommons.org/publicdomain/zero/1.0/",
            layers["commons-structured-file-metadata"]["license_uri"],
        )
        self.assertEqual(
            "https://creativecommons.org/licenses/by-sa/4.0/",
            layers["commons-unstructured-description"]["license_uri"],
        )

        self.assertFalse(inventory["source_text_included"])
        self.assertEqual(1, len(inventory["files"]))
        inventory_file = inventory["files"][0]
        self.assertEqual("djvu_pages_v1", inventory_file["profile"])
        self.assertEqual(523, inventory_file["summary"]["page_count"])
        self.assertEqual(
            1,
            inventory_file["summary"]["distinct_page_geometry_count"],
        )
        self.assertEqual(523, len(inventory_file["resources"]))
        self.assertEqual(
            list(range(1, 524)),
            [
                resource["locator"]["page_index"]
                for resource in inventory_file["resources"]
            ],
        )
        self.assertTrue(
            all(
                resource["resource_kind"] == "djvu_page"
                and resource["locator"]["width_pixels"] == 4034
                and resource["locator"]["height_pixels"] == 5834
                and resource["locator"]["resolution_dpi"] == 600
                for resource in inventory_file["resources"]
            )
        )

        commons = source_snapshot["wikimedia_commons_record"]
        archive = source_snapshot["internet_archive_source_lineage"]
        self.assertEqual(16394410, commons["page_id"])
        self.assertFalse(commons["copyrighted"])
        self.assertFalse(commons["attribution_required"])
        self.assertTrue(commons["local_sha1_match"])
        self.assertEqual(
            "confirmed-source-visible-conflict",
            archive["metadata_contamination_status"],
        )
        self.assertFalse(
            source_snapshot["source_visible_scan"]["human_repeat_performed"]
        )

        self.assertEqual(
            "partial_membership_representation",
            boundary_map["coverage_posture"],
        )
        self.assertEqual(1, len(boundary_map["members"]))
        member = boundary_map["members"][0]
        self.assertEqual("tos.work.friedrich-nietzsche.der-antichrist", member["work_ref"])
        self.assertEqual(4, member["source_sequence"])
        self.assertEqual((228, 329), (member["start_page"], member["end_page"]))
        self.assertIsNone(member["responsibility_claim_ref"])
        self.assertEqual(
            [(1, 227), (330, 523)],
            [
                (section["start_page"], section["end_page"])
                for section in boundary_map["unrepresented_sections"]
            ],
        )
        self.assertIn(
            "textual identity with the 1888 manuscript, 1895 first printing, or any critical edition",
            boundary_map["does_not_establish"],
        )

        self.assertEqual(
            [
                "channel-dnb-der-antichrist-work-authority",
                "channel-dta-der-antichrist-author-corpus",
                "channel-textgrid-der-antichrist-corpus",
                "channel-internet-archive-der-antichrist",
                "channel-google-books-der-antichrist-1895-cornell",
                "channel-wikimedia-commons-naumann-1906-volume-8",
                "channel-hadw-critical-commentary-der-antichrist",
                "channel-wikisource-naumann-1906-volume-8",
                "channel-general-web-der-antichrist",
            ],
            [channel["channel_id"] for channel in discovery["channels"]],
        )
        self.assertEqual(
            [
                "tos-discovery-result.wikimedia-commons-"
                "nietzsches-werke-band-8-naumann-1906"
            ],
            discovery["selected_result_ids"],
        )
        self.assertFalse(discovery["technical_access_bypass_used"])

        self.assertEqual("metadata-only", server_plan["access_class"])
        self.assertEqual("blocked-rights", server_plan["server_import_status"])
        self.assertEqual(
            ["DE", "US"],
            server_plan["rights_policy"]["jurisdictions_reviewed"],
        )
        self.assertEqual(4, server_plan["contract_version"])
        self.assertEqual(
            [
                "tos.event.server-import-plan.der-antichrist-naumann-1906."
                "subsection-correction.2026-08-10"
            ],
            server_plan["provenance_event_refs"],
        )
        self.assertFalse(server_plan["payload_transfer_authorized"])
        self.assertFalse(server_plan["operator_transfer_approval"]["approved"])
        self.assertEqual(
            "prohibited",
            server_plan["allowed_derivatives"]["transcription"]["state"],
        )
        self.assertEqual(
            "conditional",
            server_plan["allowed_derivatives"]["graph_projection"]["state"],
        )

        rights_research = ANTICHRIST_1906_LAYERED_RIGHTS_RESEARCH_PATH.read_text(
            encoding="utf-8"
        )
        for heading in (
            "## Classical and official documentation",
            "## Established scholarship, cases, and institutional practice",
            "## Fresh and currently relevant checks",
            "## General web search, last",
        ):
            self.assertIn(heading, rights_research)
        self.assertIn("conflicting_evidence", rights_research)
        self.assertIn("490 `TXTz`", rights_research)
        self.assertIn("pages 228-329", rights_research)
        self.assertIn("§104A(h)(6)(B)", rights_research)
        self.assertIn("ending at 2001-12-31", rights_research)
        self.assertIn("provider assertion", rights_research)
        self.assertIn(
            "independent United States foreign-work term conclusion",
            rights_research,
        )

        provenance_events = [
            json.loads(line)
            for line in (ANTICHRIST_1906_ITEM_ROOT / "provenance.jsonl")
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
        ]
        layered_event = next(
            event
            for event in provenance_events
            if event["event_id"]
            == "tos.event.rights-assessment.antichrist-naumann-1906."
            "wikimedia-commons-stanford-scan-djvu.layered.2026-08-02"
        )
        self.assertEqual(
            11,
            layered_event["method"]["configuration"]["layers_assessed"],
        )
        self.assertEqual(
            490,
            layered_event["method"]["configuration"][
                "embedded_txtz_chunk_signatures"
            ],
        )
        self.assertFalse(
            layered_event["method"]["configuration"][
                "partial_member_boundary_changed"
            ]
        )
        correction_event = next(
            event
            for event in provenance_events
            if event["event_id"]
            == "tos.event.rights-assessment.nietzsches-werke-band-8."
            "naumann-1906.wikimedia-commons-stanford-scan-djvu."
            "subsection-correction.2026-08-10"
        )
        self.assertEqual(4, correction_event["event_version"])
        self.assertEqual(
            "tos.event.rights-assessment.antichrist-naumann-1906."
            "wikimedia-commons-stanford-scan-djvu."
            "uraa-correction.2026-08-10",
            correction_event["supersedes_event_ref"],
        )
        correction_configuration = correction_event["method"]["configuration"]
        self.assertEqual(
            "17-usc-104a-h-6-b",
            correction_configuration["restored_work_definition_ref"],
        )
        self.assertEqual(
            "17-usc-104a-g",
            correction_configuration["proclamation_subsection_ref"],
        )
        self.assertEqual(
            "17-usc-104a-g-6-b",
            correction_configuration["citation_corrected_from"],
        )
        self.assertEqual(
            "17-usc-104a-h-6-b",
            correction_configuration["citation_corrected_to"],
        )
        self.assertFalse(
            correction_configuration["aggregate_assessment_status_changed"]
        )
        self.assertFalse(correction_configuration["term_endpoints_changed"])
        self.assertFalse(correction_configuration["payload_read"])
        self.assertEqual(
            "17-usc-104a-source-country-term-and-restored-remainder",
            correction_configuration["united_states_historical_route"],
        )
        self.assertEqual(
            "2001-12-31",
            correction_configuration["possible_restored_1906_term_ends_on"],
        )
        self.assertFalse(
            correction_configuration[
                "provider_pre_1931_statement_used_as_independent_legal_conclusion"
            ]
        )
        self.assertFalse(
            correction_configuration["payload_read_during_correction"]
        )

        catalog_root = REPO_ROOT / "ToS/source-witnesses/catalog"
        catalog_ids = {
            record["record_id"]
            for catalog_name in (
                "works.jsonl",
                "expressions.jsonl",
                "editions.jsonl",
                "collections.jsonl",
                "items.jsonl",
            )
            for line in (catalog_root / catalog_name)
            .read_text(encoding="utf-8")
            .splitlines()
            if line
            for record in (json.loads(line),)
        }
        self.assertTrue(
            {
                "tos.work.friedrich-nietzsche.der-antichrist",
                "tos.expression.friedrich-nietzsche.der-antichrist."
                "de-naumann-1906-volume-8",
                "tos.edition.friedrich-nietzsche.nietzsches-werke-band-8."
                "leipzig-c-g-naumann-1906",
                "tos.collection.friedrich-nietzsche."
                "nietzsches-werke-erste-abtheilung-band-8-naumann-1906",
                "tos.item.friedrich-nietzsche.nietzsches-werke-band-8."
                "naumann-1906.wikimedia-commons-stanford-scan-djvu",
            }.issubset(catalog_ids)
        )

    def test_antichrist_navigation_item_rights_correction_stays_local(
        self,
    ) -> None:
        navigation_root = (
            ANTICHRIST_1906_COLLECTION_ROOT
            / "editions/leipzig-c-g-naumann-1906/items/"
            "internet-archive-google-stanford-djvu-xml"
        )
        rights = json.loads(
            (navigation_root / "rights.json").read_text(encoding="utf-8")
        )
        manifest = json.loads(
            (navigation_root / "item.manifest.json").read_text(encoding="utf-8")
        )
        server_plan = json.loads(
            (
                REPO_ROOT
                / "ToS/source-witnesses/server-import/plans/"
                "der-antichrist-naumann-1906-internet-archive-google-"
                "stanford-djvu-xml.server-import.json"
            ).read_text(encoding="utf-8")
        )

        self.assertEqual(4, rights["record_version"])
        self.assertEqual("conflicting_evidence", rights["assessment_status"])
        self.assertEqual("local_only", rights["visibility"])
        self.assertEqual("not_authorized", rights["redistribution_posture"])
        work_layer = next(
            layer
            for layer in rights["layer_assessments"]
            if layer["layer_id"].endswith(".original-work")
        )
        self.assertEqual(
            "public_domain_reviewed",
            work_layer["assessment_status"],
        )
        self.assertIn("§104A(h)(6)(B)", work_layer["term"]["basis"])
        self.assertIn("2001-12-31", work_layer["term"]["basis"])
        self.assertIn(
            "provider evidence",
            work_layer["term"]["basis"],
        )

        payloads = {
            payload["relative_path"]: payload for payload in manifest["payload_files"]
        }
        self.assertEqual(
            {
                "payload/nietzscheswerke00nietgoog_djvu.xml": (
                    8882082,
                    "2307ace28af92da2b0128a5ef750e995d83a5655359e0debd3adb0cf1044b8c7",
                ),
                "payload/nietzscheswerke00nietgoog_jp2.zip": (
                    79087792,
                    "fa52999956bb9190e54ef2d52ed03dfbbfd4f1e91c0034319c341d48702d19a9",
                ),
                "payload/nietzscheswerke00nietgoog_scandata.xml": (
                    157669,
                    "5b2c0fe0ec55f1d330a17c066adbc56f2e0bc5b5cd4eb4761fe844601e73a8d5",
                ),
            },
            {
                path: (payload["byte_size"], payload["sha256"])
                for path, payload in payloads.items()
            },
        )

        self.assertEqual(4, server_plan["contract_version"])
        self.assertEqual(
            [
                "tos.event.server-import-plan.der-antichrist-naumann-1906."
                "internet-archive-google-stanford-djvu-xml."
                "subsection-correction.2026-08-10"
            ],
            server_plan["provenance_event_refs"],
        )
        self.assertEqual("metadata-only", server_plan["access_class"])
        self.assertEqual("blocked-rights", server_plan["server_import_status"])
        self.assertFalse(server_plan["payload_transfer_authorized"])
        self.assertFalse(
            server_plan["operator_transfer_approval"][
                "approved_by_real_human"
            ]
        )
        self.assertEqual(
            8,
            sum(
                derivative["state"] == "prohibited"
                for derivative in server_plan["allowed_derivatives"].values()
            ),
        )
        self.assertEqual(
            3,
            sum(
                derivative["state"] == "conditional"
                for derivative in server_plan["allowed_derivatives"].values()
            ),
        )

        provenance_events = [
            json.loads(line)
            for line in (navigation_root / "provenance.jsonl")
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
        ]
        correction_event = next(
            event
            for event in provenance_events
            if event["event_id"].endswith(
                "internet-archive-google-stanford-djvu-xml."
                "subsection-correction.2026-08-10"
            )
        )
        configuration = correction_event["method"]["configuration"]
        self.assertEqual(4, correction_event["event_version"])
        self.assertTrue(
            correction_event["supersedes_event_ref"].endswith(
                "navigation-derivatives.uraa-correction.2026-08-10"
            )
        )
        self.assertEqual(
            "17-usc-104a-h-6-b",
            configuration["restored_work_definition_ref"],
        )
        self.assertEqual(
            "17-usc-104a-g",
            configuration["proclamation_subsection_ref"],
        )
        self.assertEqual(
            "17-usc-104a-g-6-b",
            configuration["citation_corrected_from"],
        )
        self.assertEqual(
            "17-usc-104a-h-6-b",
            configuration["citation_corrected_to"],
        )
        self.assertFalse(configuration["aggregate_assessment_status_changed"])
        self.assertFalse(configuration["term_endpoints_changed"])
        self.assertFalse(configuration["payload_read"])
        self.assertEqual(
            "17-usc-104a-source-country-term-and-restored-remainder",
            configuration["united_states_historical_route"],
        )
        self.assertEqual(
            "2001-12-31",
            configuration["possible_restored_1906_term_ends_on"],
        )
        self.assertFalse(
            configuration[
                "provider_not_in_copyright_fields_used_as_independent_legal_conclusion"
            ]
        )
        self.assertFalse(
            configuration["navigation_item_textually_identified_with_commons_item"]
        )
        self.assertFalse(configuration["payload_read_during_correction"])
        self.assertFalse(configuration["human_legal_review"])

        rights_research = ANTICHRIST_1906_LAYERED_RIGHTS_RESEARCH_PATH.read_text(
            encoding="utf-8"
        )
        self.assertIn("8,882,082", rights_research)
        self.assertIn("79,087,792", rights_research)
        self.assertIn("157,669", rights_research)
        self.assertIn("525 fixity-bound page members", rights_research)
        self.assertIn("525 page leaves", rights_research)
        self.assertIn("separately registered local", rights_research)
        self.assertIn("navigation Item", rights_research)

    def test_antichrist_authorial_route_keeps_regions_and_layers_distinct(
        self,
    ) -> None:
        discovery = json.loads(
            ANTICHRIST_AUTHORIAL_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        work = json.loads(
            (
                REPO_ROOT
                / "ToS/source-witnesses/works/friedrich-nietzsche/"
                "der-antichrist/work.json"
            ).read_text(encoding="utf-8")
        )

        self.assertEqual(
            [
                "channel-gsa-ores-antichrist-authorial-route",
                "channel-basel-overbeck-antichrist-copy",
                "channel-first-print-antichrist-1895",
                "channel-nietzsche-source-antichrist-critical-route",
                "channel-established-antichrist-text-history",
                "channel-fresh-antichrist-research",
                "channel-general-web-antichrist-authorial-route",
            ],
            [channel["channel_id"] for channel in discovery["channels"]],
        )
        self.assertEqual(9, len(discovery["selected_result_ids"]))
        self.assertEqual(
            [
                "tos-discovery-result."
                "general-web-antichrist-original-manuscript-reprints"
            ],
            discovery["rejected_result_ids"],
        )
        self.assertFalse(discovery["technical_access_bypass_used"])

        results = {
            result["result_id"]: result
            for channel in discovery["channels"]
            for result in channel["results"]
        }
        self.assertEqual(
            "select",
            results["tos-discovery-result.gsa-71-29-antichrist-d22"][
                "decision"
            ],
        )
        self.assertIn(
            "whole-notebook",
            results[
                "tos-discovery-result.gsa-antichrist-wii1-wii6-regions"
            ]["rationale"],
        )
        self.assertIn(
            "not AC 63",
            results[
                "tos-discovery-result."
                "gsa-71-32-fol47-antichrist-gesetz-adjunct"
            ]["rationale"],
        )
        self.assertIn(
            "not Nietzsche's autograph",
            results[
                "tos-discovery-result."
                "basel-nl53-a311-overbeck-antichrist-copy"
            ]["rationale"],
        )
        self.assertEqual(
            "unknown",
            results[
                "tos-discovery-result."
                "nietzsche-source-antichrist-live-addresses-timeout"
            ]["availability"],
        )
        self.assertTrue(
            all(
                not result["acquisition"]["downloaded"]
                and result["snapshot"]["state"] == "not-captured"
                for result in results.values()
            )
        )

        self.assertEqual(6, work["record_version"])
        self.assertIn(
            "ToS/source-witnesses/discovery/runs/"
            "antichrist-authorial-witness-route.2026-07-30.v1.json",
            work["source_refs"],
        )
        self.assertIn("NL 53 : A 311", work["notes"])
        self.assertIn("GSA 71/32 fol. 47", work["notes"])
        self.assertEqual("no_equivalence_claim", work["same_as_posture"])

        provenance_events = [
            json.loads(line)
            for line in (
                ANTICHRIST_1906_ITEM_ROOT / "provenance.jsonl"
            ).read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        discovery_event = next(
            event
            for event in provenance_events
            if event["event_id"]
            == "tos.event.discovery.antichrist."
            "authorial-witness-route.2026-07-30"
        )
        self.assertEqual(
            "tos.event.discovery.antichrist."
            "authorial-witness-route.2026-07-30",
            discovery_event["event_id"],
        )
        self.assertFalse(
            discovery_event["method"]["configuration"][
                "human_task_created"
            ]
        )
        self.assertEqual(
            0,
            discovery_event["method"]["configuration"][
                "semantic_objects_created"
            ],
        )

        route = (
            REPO_ROOT
            / "ToS/research-packets/foundation-laboratory-2026-07/"
            "ANTICHRIST_AUTHORIAL_WITNESS_ROUTE.md"
        ).read_text(encoding="utf-8")
        self.assertIn("D 22 / GSA 71/29", route)
        self.assertIn("NL 53 : A 311", route)
        self.assertIn("Koegel/Naumann 1895 first print", route)
        self.assertIn("contested associated adjunct", route)
        self.assertIn("deliberately named rather than called A/B/C", route)
        self.assertNotIn("Antichrist A/B/C witness route", route)

    def test_fall_wagner_authorial_route_preserves_loss_and_issue_layers(
        self,
    ) -> None:
        discovery = json.loads(
            FALL_WAGNER_AUTHORIAL_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        work = json.loads(
            (
                REPO_ROOT
                / "ToS/source-witnesses/works/friedrich-nietzsche/"
                "der-fall-wagner/work.json"
            ).read_text(encoding="utf-8")
        )

        self.assertEqual(
            [
                "channel-gsa-ores-fall-wagner-authorial-route",
                "channel-basel-fall-wagner-fragment",
                "channel-mdz-fall-wagner-1888-existing-item",
                "channel-nietzsche-source-fall-wagner-critical-route",
                "channel-established-fall-wagner-text-history",
                "channel-fresh-fall-wagner-research",
                "channel-general-web-fall-wagner-authorial-route",
            ],
            [channel["channel_id"] for channel in discovery["channels"]],
        )
        self.assertEqual(9, len(discovery["selected_result_ids"]))
        self.assertEqual(
            [
                "tos-discovery-result.gsa-71-28-d21-goetzen-not-fall-wagner",
                "tos-discovery-result."
                "general-web-fall-wagner-reprints-summaries",
            ],
            discovery["rejected_result_ids"],
        )
        self.assertFalse(discovery["technical_access_bypass_used"])

        results = {
            result["result_id"]: result
            for channel in discovery["channels"]
            for result in channel["results"]
        }
        notebook_result = results[
            "tos-discovery-result.gsa-wii6-wii7-fall-wagner-regions"
        ]
        notebook_identifiers = {
            identifier["scheme"]: identifier["value"]
            for identifier in notebook_result["identifiers"]
        }
        self.assertEqual("71/162; 71/163", notebook_identifiers["GSA"])
        self.assertEqual("75271; 75272", notebook_identifiers["ORES"])
        self.assertIn("whole-notebook", notebook_result["rationale"])

        wrong_d21 = results[
            "tos-discovery-result.gsa-71-28-d21-goetzen-not-fall-wagner"
        ]
        self.assertEqual("reject", wrong_d21["decision"])
        self.assertIn("Götzen-Dämmerung", wrong_d21["rationale"])
        self.assertIn(
            "explicitly rejected as a Der Fall Wagner manuscript",
            wrong_d21["rationale"],
        )

        basel = results[
            "tos-discovery-result.basel-nl200-ii1-fall-wagner-fragment"
        ]
        basel_identifiers = {
            identifier["scheme"]: identifier["value"]
            for identifier in basel["identifiers"]
        }
        self.assertEqual("NL 200 : II, 1", basel_identifiers["UB Basel shelfmark"])
        self.assertEqual(
            "10.7891/e-manuscripta-80734",
            basel_identifiers["DOI"],
        )
        self.assertIn("only surviving manuscript fragment", basel["rationale"])
        self.assertIn("not joint authorship", basel["rationale"])
        self.assertEqual(
            "https://www.e-manuscripta.ch/bau/content/titleinfo/2274751",
            basel["declared_rights"]["evidence_url"],
        )

        timeout = results[
            "tos-discovery-result."
            "nietzsche-source-fall-wagner-live-addresses-timeout"
        ]
        self.assertEqual("unknown", timeout["availability"])
        self.assertIn("status 000", timeout["rationale"])
        self.assertTrue(
            all(
                not result["acquisition"]["downloaded"]
                and result["snapshot"]["state"] == "not-captured"
                for result in results.values()
            )
        )

        self.assertEqual(6, work["record_version"])
        self.assertIn(
            "ToS/source-witnesses/discovery/runs/"
            "fall-wagner-authorial-witness-route.2026-07-30.v1.json",
            work["source_refs"],
        )
        self.assertIn("complete 26 June print manuscript", work["notes"])
        self.assertIn("NL 200 : II, 1", work["notes"])
        self.assertIn("nominal Zweite Auflage half", work["notes"])
        self.assertIn("D 21 / GSA 71/28", work["notes"])
        self.assertEqual("no_equivalence_claim", work["same_as_posture"])

        edition = json.loads(
            (FALL_WAGNER_1888_EDITION_ROOT / "edition.json").read_text(
                encoding="utf-8"
            )
        )
        publication_claims = [
            json.loads(line)
            for line in FALL_WAGNER_1888_PUBLICATION_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
        ]
        claims_by_predicate = {
            claim["predicate"]: claim for claim in publication_claims
        }
        self.assertEqual(3, edition["record_version"])
        self.assertEqual(6, len(publication_claims))
        self.assertEqual(
            set(edition["publication_claim_refs"]),
            {claim["claim_id"] for claim in publication_claims},
        )
        self.assertTrue(
            all(
                claim["subject_ref"] == edition["record_id"]
                and claim["claim_type"] == "bibliographic"
                and claim["review_status"] == "unreviewed"
                and claim["reviews"] == []
                and claim["visibility"] == "public_metadata_only"
                for claim in publication_claims
            )
        )
        self.assertEqual(
            "1888",
            claims_by_predicate["title_page_year"]["object"],
        )
        self.assertEqual(
            "observed",
            claims_by_predicate["title_page_year"]["epistemic_status"],
        )
        self.assertEqual(
            "author_supervised_first_publication_edition",
            claims_by_predicate["publication_role"]["object"],
        )
        self.assertEqual(
            {
                "year": 1888,
                "month": 9,
                "precision": "month",
            },
            claims_by_predicate["printing_completed_in"]["object"],
        )
        self.assertEqual(
            "1888-09-22",
            claims_by_predicate["official_publication_on"]["object"],
        )
        self.assertEqual(
            {
                "copies_reported": 1000,
                "production_run_count": 1,
            },
            claims_by_predicate["print_run_extent"]["object"],
        )
        nominal_state = claims_by_predicate[
            "has_nominal_later_issue_state"
        ]["object"]
        self.assertEqual("Zweite Auflage", nominal_state["label"])
        self.assertEqual(
            "latter_half_of_same_1000_copy_run",
            nominal_state["relation_to_run"],
        )
        self.assertFalse(nominal_state["current_item_carries_label"])
        self.assertFalse(nominal_state["separate_tos_item_created"])
        self.assertEqual(
            "unresolved",
            nominal_state["textual_identity_status"],
        )
        self.assertEqual(
            "unresolved",
            nominal_state["textual_difference_status"],
        )

        provenance_events = [
            json.loads(line)
            for line in (
                FALL_WAGNER_1888_ITEM_ROOT / "provenance.jsonl"
            ).read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        event = next(
            event
            for event in provenance_events
            if event["event_id"]
            == "tos.event.discovery.fall-wagner."
            "authorial-witness-route.2026-07-30"
        )
        self.assertEqual(
            "tos.event.discovery.fall-wagner."
            "authorial-witness-route.2026-07-30",
            event["event_id"],
        )
        self.assertFalse(
            event["method"]["configuration"]["human_task_created"]
        )
        self.assertFalse(
            event["method"]["configuration"]["new_item_created"]
        )
        self.assertEqual(
            0,
            event["method"]["configuration"]["source_payloads_acquired"],
        )
        self.assertEqual(
            0,
            event["method"]["configuration"]["semantic_objects_created"],
        )
        claim_event = next(
            event
            for event in provenance_events
            if event["event_id"]
            == "tos.event.annotation.der-fall-wagner."
            "naumann-1888-publication-claims.2026-07-30"
        )
        self.assertEqual(
            "tos.event.annotation.der-fall-wagner."
            "naumann-1888-publication-claims.2026-07-30",
            claim_event["event_id"],
        )
        self.assertEqual(
            {
                "ref": (
                    "ToS/source-witnesses/works/friedrich-nietzsche/"
                    "der-fall-wagner/expressions/de-naumann-1888/"
                    "editions/leipzig-c-g-naumann-1888/"
                    "publication-claims.jsonl"
                ),
                "role": "unreviewed-evidence-bearing-publication-claims",
                "sha256": hashlib.sha256(
                    FALL_WAGNER_1888_PUBLICATION_CLAIMS_PATH.read_bytes()
                ).hexdigest(),
            },
            claim_event["outputs"][0],
        )
        self.assertEqual(
            {
                "source_claims_materialized": 6,
                "publication_claims_reviewed": 0,
                "remote_items_created": 0,
                "source_text_admitted": False,
                "semantic_claims_created": 0,
                "canon_promotion_performed": False,
            },
            claim_event["method"]["configuration"],
        )

        route = (
            REPO_ROOT
            / "ToS/research-packets/foundation-laboratory-2026-07/"
            "FALL_WAGNER_AUTHORIAL_WITNESS_ROUTE.md"
        ).read_text(encoding="utf-8")
        self.assertIn("W II 6 / W II 7 regions only", route)
        self.assertIn("D 21 is explicitly rejected for this work", route)
        self.assertIn("two complete manuscript states", route)
        self.assertIn("sole surviving leaf", route)
        self.assertIn("nominally labelled \"Zweite Auflage\"", route)
        self.assertIn(
            "`textual_identity_status: unresolved`",
            route,
        )
        self.assertIn(
            "`textual_difference_status: unresolved`",
            route,
        )
        self.assertIn("deliberately named rather than called A/B/C", route)
        self.assertNotIn("Fall Wagner A/B/C witness route", route)

    def test_goetzen_authorial_route_preserves_late_additions_and_editorial_states(
        self,
    ) -> None:
        discovery = json.loads(
            GOETZEN_AUTHORIAL_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        work = json.loads(
            (
                REPO_ROOT
                / "ToS/source-witnesses/works/friedrich-nietzsche/"
                "goetzen-daemmerung/work.json"
            ).read_text(encoding="utf-8")
        )

        self.assertEqual(
            [
                "channel-gsa-ores-goetzen-authorial-route",
                "channel-mdz-goetzen-1889-existing-item",
                "channel-nietzsche-source-goetzen-critical-route",
                "channel-established-goetzen-text-history",
                "channel-fresh-goetzen-chronology-method",
                "channel-fresh-goetzen-reconstruction-interpretation",
                "channel-general-web-goetzen-authorial-route",
            ],
            [channel["channel_id"] for channel in discovery["channels"]],
        )
        self.assertEqual(8, len(discovery["selected_result_ids"]))
        self.assertEqual(
            [
                "tos-discovery-result.gutenberg-goetzen-derived-html",
                "tos-discovery-result."
                "general-web-goetzen-reprints-summaries",
            ],
            discovery["rejected_result_ids"],
        )
        self.assertFalse(discovery["technical_access_bypass_used"])

        results = {
            result["result_id"]: result
            for channel in discovery["channels"]
            for result in channel["results"]
        }
        notebook_result = results[
            "tos-discovery-result.gsa-wii6-wii9-goetzen-regions"
        ]
        notebook_identifiers = {
            identifier["scheme"]: identifier["value"]
            for identifier in notebook_result["identifiers"]
        }
        self.assertEqual(
            "71/162; 71/163; 71/164; 71/165",
            notebook_identifiers["GSA"],
        )
        self.assertEqual(
            "75271; 75272; 75273; 75274",
            notebook_identifiers["ORES"],
        )
        self.assertIn("whole-notebook", notebook_result["rationale"])
        self.assertIn("W II 1", notebook_result["rationale"])

        print_manuscript = results[
            "tos-discovery-result."
            "gsa-71-28-d21-goetzen-print-manuscript"
        ]
        manuscript_identifiers = {
            identifier["scheme"]: identifier["value"]
            for identifier in print_manuscript["identifiers"]
        }
        self.assertEqual("71/28", manuscript_identifiers["GSA"])
        self.assertEqual("75099", manuscript_identifiers["ORES"])
        self.assertEqual("D 21", manuscript_identifiers["Mette"])
        self.assertEqual("D-21", manuscript_identifiers["DFGA"])
        self.assertIn(
            "124 JPEG canvases",
            print_manuscript["available_formats"],
        )
        self.assertEqual(
            "Public Domain",
            print_manuscript["declared_rights"]["statement"].split(
                "labels the Götzen-Dämmerung digital object "
            )[-1].rstrip("."),
        )

        reconstruction = results[
            "tos-discovery-result."
            "riera-magnum-in-parvo-reconstruction-2024"
        ]
        self.assertEqual("defer", reconstruction["decision"])
        self.assertIn(
            "editorial reconstruction",
            reconstruction["rationale"],
        )
        self.assertIn(
            "not a Nietzsche-authored published Work",
            reconstruction["rationale"],
        )

        timeout = results[
            "tos-discovery-result."
            "nietzsche-source-goetzen-live-addresses-timeout"
        ]
        self.assertEqual("unknown", timeout["availability"])
        self.assertIn("status 000", timeout["rationale"])
        self.assertTrue(
            all(
                not result["acquisition"]["downloaded"]
                and result["snapshot"]["state"]
                in {
                    "not-captured",
                    "not-needed",
                    "not-permitted",
                }
                for result in results.values()
            )
        )

        self.assertEqual(6, work["record_version"])
        self.assertIn(
            "ToS/source-witnesses/discovery/runs/"
            "goetzen-daemmerung-authorial-witness-route."
            "2026-07-30.v1.json",
            work["source_refs"],
        )
        self.assertIn("D 21 / GSA 71/28", work["notes"])
        self.assertIn("November 1888", work["notes"])
        self.assertIn("1893 second edition", work["notes"])
        self.assertIn("non-author-approved", work["notes"])
        self.assertEqual("no_equivalence_claim", work["same_as_posture"])

        edition = json.loads(
            (GOETZEN_1889_EDITION_ROOT / "edition.json").read_text(
                encoding="utf-8"
            )
        )
        publication_claims = [
            json.loads(line)
            for line in GOETZEN_1889_PUBLICATION_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
        ]
        claims_by_predicate = {
            claim["predicate"]: claim for claim in publication_claims
        }
        self.assertEqual(4, edition["record_version"])
        self.assertEqual(6, len(publication_claims))
        self.assertEqual(
            set(edition["publication_claim_refs"]),
            {claim["claim_id"] for claim in publication_claims},
        )
        self.assertTrue(
            all(
                claim["subject_ref"] == edition["record_id"]
                and claim["claim_type"] == "bibliographic"
                and claim["review_status"] == "unreviewed"
                and claim["reviews"] == []
                for claim in publication_claims
            )
        )
        self.assertEqual(
            "1889",
            claims_by_predicate["title_page_year"]["object"],
        )
        self.assertEqual(
            "observed",
            claims_by_predicate["title_page_year"]["epistemic_status"],
        )
        self.assertEqual(
            "1888-11-13",
            claims_by_predicate["printing_completed_on"]["object"],
        )
        self.assertEqual(
            {
                "date": "1888-11-24",
                "copies_reported": 4,
            },
            claims_by_predicate[
                "author_received_finished_copies_on"
            ]["object"],
        )
        self.assertEqual(
            "1889-01-24",
            claims_by_predicate["public_sale_released_on"]["object"]["date"],
        )
        later_state = claims_by_predicate[
            "has_distinct_later_editorial_state"
        ]["object"]
        self.assertEqual(1893, later_state["publication_year"])
        self.assertEqual("Heinrich Köselitz", later_state["editor"])
        self.assertTrue(later_state["textual_change_reported"])
        self.assertFalse(later_state["author_approval_reported"])
        self.assertFalse(later_state["tos_item_created"])

        provenance_events = [
            json.loads(line)
            for line in (
                GOETZEN_1889_ITEM_ROOT / "provenance.jsonl"
            ).read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        event = next(
            event
            for event in provenance_events
            if event["event_id"]
            == "tos.event.discovery.goetzen-daemmerung."
            "authorial-witness-route.2026-07-30"
        )
        self.assertEqual(
            "tos.event.discovery.goetzen-daemmerung."
            "authorial-witness-route.2026-07-30",
            event["event_id"],
        )
        self.assertFalse(
            event["method"]["configuration"]["human_task_created"]
        )
        self.assertFalse(
            event["method"]["configuration"]["new_item_created"]
        )
        self.assertEqual(
            0,
            event["method"]["configuration"]["source_payloads_acquired"],
        )
        self.assertEqual(
            0,
            event["method"]["configuration"]["semantic_objects_created"],
        )
        claim_event = next(
            event
            for event in provenance_events
            if event["event_id"]
            == "tos.event.annotation.goetzen-daemmerung."
            "naumann-1889-publication-claims.2026-07-30"
        )
        self.assertEqual(
            "tos.event.annotation.goetzen-daemmerung."
            "naumann-1889-publication-claims.2026-07-30",
            claim_event["event_id"],
        )
        self.assertEqual(
            {
                "ref": (
                    "ToS/source-witnesses/works/friedrich-nietzsche/"
                    "goetzen-daemmerung/expressions/de-naumann-1889/"
                    "editions/leipzig-c-g-naumann-1889/"
                    "publication-claims.jsonl"
                ),
                "role": "unreviewed-evidence-bearing-publication-claims",
                "sha256": hashlib.sha256(
                    GOETZEN_1889_PUBLICATION_CLAIMS_PATH.read_bytes()
                ).hexdigest(),
            },
            claim_event["outputs"][0],
        )
        self.assertEqual(
            {
                "source_claims_materialized": 6,
                "publication_claims_reviewed": 0,
                "remote_items_created": 0,
                "source_text_admitted": False,
                "semantic_claims_created": 0,
                "canon_promotion_performed": False,
            },
            claim_event["method"]["configuration"],
        )

        route = (
            REPO_ROOT
            / "ToS/research-packets/foundation-laboratory-2026-07/"
            "GOETZEN_DAEMMERUNG_AUTHORIAL_WITNESS_ROUTE.md"
        ).read_text(encoding="utf-8")
        self.assertIn("D 21 / GSA 71/28", route)
        self.assertIn("Streifzüge eines Unzeitgemässen §§32-44", route)
        self.assertIn("Was ich den Alten verdanke", route)
        self.assertIn("Köselitz-edited 1893 second edition", route)
        self.assertIn("editorial reconstruction", route)
        self.assertIn("deliberately named rather than called A/B/C", route)
        self.assertNotIn("Götzen-Dämmerung A/B/C witness route", route)

    def test_exact_fall_and_goetzen_witnesses_advance_source_not_content(
        self,
    ) -> None:
        cases = [
            {
                "item_root": FALL_WAGNER_1888_ITEM_ROOT,
                "discovery_path": FALL_WAGNER_1888_DISCOVERY_PATH,
                "server_plan": (
                    "der-fall-wagner-naumann-1888-mdz-bamberg-scan-pdf."
                    "server-import.json"
                ),
                "byte_size": 31518299,
                "sha256": (
                    "37fc9eb2d26886be936efe06c7fbeaf9f6dab231b3a81bc2cb5e824e98f984ed"
                ),
                "page_count": 75,
                "image_count": 76,
                "geometry_count": 32,
                "mdz_object_id": "bsb11827837",
                "canvas_count": 74,
                "rights_statement_uri": (
                    "https://rightsstatements.org/vocab/NoC-NC/1.0/"
                ),
                "license_uri": None,
                "redistribution_posture": "not_authorized",
                "assessment_status": "permission_granted",
                "exact_object_layer": "exact-mdz-google-digital-object",
                "server_rights_status": "permission-granted",
                "rights_event_id": (
                    "tos.event.rights-assessment.der-fall-wagner-naumann-"
                    "1888.mdz-bamberg-scan-pdf.uraa-correction.2026-08-10"
                ),
                "server_event_id": (
                    "tos.event.server-import-plan.der-fall-wagner-naumann-"
                    "1888-mdz.uraa-correction.2026-08-10"
                ),
                "channels": [
                    "channel-dnb-der-fall-wagner-work-authority",
                    "channel-dta-der-fall-wagner-author-corpus",
                    "channel-textgrid-der-fall-wagner-corpus",
                    "channel-hadw-der-fall-wagner-text-history",
                    "channel-google-books-der-fall-wagner-1888",
                    "channel-mdz-bamberg-der-fall-wagner-1888",
                    "channel-general-web-der-fall-wagner",
                ],
                "selected_result": (
                    "tos-discovery-result.mdz-bsb11827837-bamberg-"
                    "der-fall-wagner-1888"
                ),
                "catalog_ids": {
                    "tos.work.friedrich-nietzsche.der-fall-wagner",
                    "tos.expression.friedrich-nietzsche.der-fall-wagner."
                    "de-naumann-1888",
                    "tos.edition.friedrich-nietzsche.der-fall-wagner."
                    "leipzig-c-g-naumann-1888",
                    "tos.item.friedrich-nietzsche.der-fall-wagner."
                    "de-naumann-1888.mdz-bamberg-scan-pdf",
                },
            },
            {
                "item_root": GOETZEN_1889_ITEM_ROOT,
                "discovery_path": GOETZEN_1889_DISCOVERY_PATH,
                "server_plan": (
                    "goetzen-daemmerung-naumann-1889-mdz-bsb-scan-pdf."
                    "server-import.json"
                ),
                "byte_size": 68398920,
                "sha256": (
                    "f41a1dee091edd895d1f18a510dc73b48949257e882d3390c6c8f72beeb8d086"
                ),
                "page_count": 165,
                "image_count": 166,
                "geometry_count": 83,
                "mdz_object_id": "bsb00069119",
                "canvas_count": 164,
                "rights_statement_uri": None,
                "license_uri": (
                    "https://creativecommons.org/licenses/by-nc-sa/4.0/"
                ),
                "redistribution_posture": "not_authorized",
                "assessment_status": "licensed",
                "exact_object_layer": "exact-mdz-digital-object",
                "server_rights_status": "open-licensed",
                "rights_event_id": (
                    "tos.event.rights-assessment.goetzen-daemmerung-naumann-"
                    "1889.mdz-bsb-scan-pdf.uraa-correction.2026-08-10"
                ),
                "server_event_id": (
                    "tos.event.server-import-plan.goetzen-daemmerung-naumann-"
                    "1889-mdz.uraa-correction.2026-08-10"
                ),
                "channels": [
                    "channel-dnb-goetzen-daemmerung-work-authority",
                    "channel-dta-goetzen-daemmerung-author-corpus",
                    "channel-textgrid-goetzen-daemmerung-corpus",
                    "channel-hadw-goetzen-daemmerung-text-history",
                    "channel-google-books-goetzen-daemmerung-1889",
                    "channel-mdz-bsb-goetzen-daemmerung-1889",
                    "channel-general-web-goetzen-daemmerung",
                ],
                "selected_result": (
                    "tos-discovery-result.mdz-bsb00069119-"
                    "goetzen-daemmerung-1889"
                ),
                "catalog_ids": {
                    "tos.work.friedrich-nietzsche.goetzen-daemmerung",
                    "tos.expression.friedrich-nietzsche.goetzen-daemmerung."
                    "de-naumann-1889",
                    "tos.edition.friedrich-nietzsche.goetzen-daemmerung."
                    "leipzig-c-g-naumann-1889",
                    "tos.item.friedrich-nietzsche.goetzen-daemmerung."
                    "de-naumann-1889.mdz-bsb-scan-pdf",
                },
            },
        ]

        catalog_root = REPO_ROOT / "ToS/source-witnesses/catalog"
        catalog_ids = {
            record["record_id"]
            for catalog_name in (
                "works.jsonl",
                "expressions.jsonl",
                "editions.jsonl",
                "items.jsonl",
            )
            for line in (catalog_root / catalog_name)
            .read_text(encoding="utf-8")
            .splitlines()
            if line
            for record in (json.loads(line),)
        }
        server_provenance_events = {
            event["event_id"]: event
            for line in (
                REPO_ROOT / "ToS/source-witnesses/server-import/provenance.jsonl"
            )
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
            for event in (json.loads(line),)
        }

        for case in cases:
            with self.subTest(item_root=case["item_root"]):
                item_root = case["item_root"]
                manifest = json.loads(
                    (item_root / "item.manifest.json").read_text(encoding="utf-8")
                )
                rights = json.loads(
                    (item_root / "rights.json").read_text(encoding="utf-8")
                )
                inventory = json.loads(
                    (item_root / "resource-inventory.json").read_text(
                        encoding="utf-8"
                    )
                )
                source_snapshot = json.loads(
                    (item_root / "source-metadata-snapshot.json").read_text(
                        encoding="utf-8"
                    )
                )
                discovery = json.loads(
                    case["discovery_path"].read_text(encoding="utf-8")
                )
                server_plan = json.loads(
                    (
                        REPO_ROOT
                        / "ToS/source-witnesses/server-import/plans"
                        / case["server_plan"]
                    ).read_text(encoding="utf-8")
                )

                self.assertEqual(
                    "digitized_physical_copy",
                    manifest["item_kind"],
                )
                self.assertEqual(
                    "local_gitignored_payload",
                    manifest["storage_posture"],
                )
                self.assertEqual("local_only", manifest["visibility"])
                self.assertEqual(1, len(manifest["payload_files"]))
                payload_file = manifest["payload_files"][0]
                self.assertEqual(case["byte_size"], payload_file["byte_size"])
                self.assertEqual(case["sha256"], payload_file["sha256"])
                self.assertTrue(
                    foundation._git_ignored(
                        REPO_ROOT,
                        item_root / payload_file["relative_path"],
                    )
                )

                self.assertEqual(
                    case["rights_statement_uri"],
                    rights["rights_statement_uri"],
                )
                self.assertEqual(case["license_uri"], rights["license_uri"])
                self.assertEqual(
                    case["assessment_status"],
                    rights["assessment_status"],
                )
                self.assertEqual("unreviewed", rights["review_status"])
                self.assertEqual(["DE", "US"], rights["jurisdictions_reviewed"])
                self.assertEqual(
                    case["redistribution_posture"],
                    rights["redistribution_posture"],
                )
                self.assertEqual(
                    "local_research_only",
                    rights["derivative_posture"],
                )
                self.assertEqual("local_only", rights["visibility"])
                self.assertEqual(3, rights["record_version"])

                layers = {
                    layer["layer_id"].rsplit(".layer.", 1)[1]: layer
                    for layer in rights["layer_assessments"]
                }
                self.assertEqual(5, len(layers))
                for historical_layer_id in (
                    "original-work",
                    "edition-presentation",
                ):
                    historical_layer = layers[historical_layer_id]
                    self.assertIn("§104A", historical_layer["term"]["basis"])
                    self.assertNotIn(
                        "pre-1931",
                        historical_layer["term"]["basis"],
                    )
                    self.assertIn(
                        "https://www.copyright.gov/gatt.html",
                        historical_layer["source_refs"],
                    )
                for layer_id in (
                    "original-work",
                    "edition-presentation",
                    "faithful-historical-page-scan",
                ):
                    self.assertEqual(
                        "public_domain_reviewed",
                        layers[layer_id]["assessment_status"],
                    )
                self.assertEqual(
                    case["assessment_status"],
                    layers[case["exact_object_layer"]]["assessment_status"],
                )
                metadata_layer = layers["bsb-bdr-bibliographic-metadata"]
                self.assertEqual("licensed", metadata_layer["assessment_status"])
                self.assertEqual(
                    "https://creativecommons.org/publicdomain/zero/1.0/",
                    metadata_layer["license_uri"],
                )

                self.assertFalse(inventory["source_text_included"])
                self.assertEqual(1, len(inventory["files"]))
                inventory_file = inventory["files"][0]
                self.assertEqual("pdf_pages_v1", inventory_file["profile"])
                self.assertEqual(
                    case["page_count"],
                    inventory_file["summary"]["page_count"],
                )
                self.assertEqual(
                    case["image_count"],
                    inventory_file["summary"]["image_resource_count"],
                )
                self.assertEqual(
                    case["geometry_count"],
                    inventory_file["summary"]["distinct_page_geometry_count"],
                )
                self.assertEqual(
                    case["page_count"],
                    len(inventory_file["resources"]),
                )
                self.assertEqual(
                    list(range(1, case["page_count"] + 1)),
                    [
                        resource["locator"]["page_index"]
                        for resource in inventory_file["resources"]
                    ],
                )
                self.assertEqual(
                    2,
                    inventory_file["resources"][0]["image_resource_count"],
                )

                mdz = source_snapshot["mdz_digital_object"]
                pdf = source_snapshot["mdz_immediate_pdf"]
                self.assertEqual(case["mdz_object_id"], mdz["digital_object_id"])
                self.assertEqual(case["canvas_count"], mdz["iiif_canvas_count"])
                self.assertEqual(case["page_count"], pdf["pdf_page_count"])
                self.assertFalse(pdf["embedded_book_ocr_present"])
                self.assertEqual(
                    case["sha256"],
                    pdf["sha256"],
                )
                self.assertFalse(
                    source_snapshot["source_visible_scan"][
                        "human_repeat_performed"
                    ]
                )
                self.assertFalse(
                    source_snapshot["scholarly_text_history"][
                        "critical_text_admitted"
                    ]
                )

                self.assertEqual(
                    case["channels"],
                    [channel["channel_id"] for channel in discovery["channels"]],
                )
                self.assertEqual(
                    [case["selected_result"]],
                    discovery["selected_result_ids"],
                )
                self.assertFalse(discovery["technical_access_bypass_used"])

                self.assertEqual("metadata-only", server_plan["access_class"])
                self.assertEqual(
                    "blocked-rights",
                    server_plan["server_import_status"],
                )
                self.assertEqual(
                    case["server_rights_status"],
                    server_plan["rights_policy"]["assessment_status"],
                )
                self.assertEqual(
                    ["DE", "US"],
                    server_plan["rights_policy"]["jurisdictions_reviewed"],
                )
                self.assertEqual(3, server_plan["contract_version"])
                self.assertEqual(
                    hashlib.sha256(
                        (item_root / "rights.json").read_bytes()
                    ).hexdigest(),
                    server_plan["rights_policy"]["rights_record_sha256"],
                )
                self.assertIn(
                    "https://www.copyright.gov/gatt.html",
                    server_plan["rights_policy"]["permission_or_license_refs"],
                )
                self.assertEqual(
                    [case["server_event_id"]],
                    server_plan["provenance_event_refs"],
                )
                self.assertFalse(server_plan["payload_transfer_authorized"])
                self.assertFalse(
                    server_plan["operator_transfer_approval"]["approved"]
                )
                self.assertEqual(
                    "prohibited",
                    server_plan["allowed_derivatives"]["transcription"]["state"],
                )
                self.assertEqual(
                    "conditional",
                    server_plan["allowed_derivatives"]["graph_projection"][
                        "state"
                    ],
                )
                self.assertTrue(case["catalog_ids"].issubset(catalog_ids))

                provenance_events = [
                    json.loads(line)
                    for line in (item_root / "provenance.jsonl")
                    .read_text(encoding="utf-8")
                    .splitlines()
                    if line.strip()
                ]
                rights_event = next(
                    event
                    for event in provenance_events
                    if event["event_id"] == case["rights_event_id"]
                )
                self.assertFalse(
                    rights_event["method"]["configuration"][
                        "operator_local_payload_site_upload"
                    ]
                )
                self.assertEqual(
                    "17 U.S.C. 104A source-country-protection condition",
                    rights_event["method"]["configuration"][
                        "united_states_historical_route"
                    ],
                )
                server_event = server_provenance_events[case["server_event_id"]]
                self.assertFalse(
                    server_event["method"]["configuration"][
                        "aggregate_rights_status_changed"
                    ]
                )
                self.assertFalse(
                    server_event["method"]["configuration"][
                        "payload_transfer_authorized"
                    ]
                )

        rights_research = MDZ_PAIR_LAYERED_RIGHTS_RESEARCH_PATH.read_text(
            encoding="utf-8"
        )
        for heading in (
            "## Classical and official documentation",
            "## Established scholarship, cases, and institutional practice",
            "## Fresh and currently relevant checks",
            "## General web search, last",
        ):
            self.assertIn(heading, rights_research)
        self.assertIn("permission_granted", rights_research)
        self.assertIn("CC BY-NC-SA 4.0", rights_research)
        self.assertIn("five layers", rights_research.lower())
        self.assertIn("No OCR layer is invented", rights_research)
        self.assertIn("17 U.S.C.\n§104A", rights_research)
        self.assertIn("domestic pre-1931", rights_research)
        self.assertIn("Golan v. Holder", rights_research)
        self.assertIn("bytes drifted", rights_research)

    def test_ecce_homo_1908_witness_preserves_editorial_and_rights_boundaries(
        self,
    ) -> None:
        manifest = json.loads(
            (ECCE_HOMO_1908_ITEM_ROOT / "item.manifest.json").read_text(
                encoding="utf-8"
            )
        )
        inventory = json.loads(
            (ECCE_HOMO_1908_ITEM_ROOT / "resource-inventory.json").read_text(
                encoding="utf-8"
            )
        )
        rights = json.loads(
            (ECCE_HOMO_1908_ITEM_ROOT / "rights.json").read_text(
                encoding="utf-8"
            )
        )
        source_snapshot = json.loads(
            (ECCE_HOMO_1908_ITEM_ROOT / "source-metadata-snapshot.json").read_text(
                encoding="utf-8"
            )
        )
        discovery = json.loads(
            ECCE_HOMO_1908_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        server_plan = json.loads(
            (
                REPO_ROOT
                / "ToS/source-witnesses/server-import/plans/"
                "ecce-homo-insel-1908-wikimedia-commons-getty-scan-pdf."
                "server-import.json"
            ).read_text(encoding="utf-8")
        )
        work = json.loads(
            (ECCE_HOMO_WORK_ROOT / "work.json").read_text(encoding="utf-8")
        )
        edition = json.loads(
            (ECCE_HOMO_1908_EDITION_ROOT / "edition.json").read_text(
                encoding="utf-8"
            )
        )
        responsibility_claims = [
            json.loads(line)
            for line in ECCE_HOMO_RESPONSIBILITY_CLAIMS_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
            if line.strip()
        ]
        claims_by_predicate = {
            claim["predicate"]: claim for claim in responsibility_claims
        }

        payload = manifest["payload_files"][0]
        self.assertEqual(9_703_400, payload["byte_size"])
        self.assertEqual(
            "f3058ff02611cc961de1f7c4fbf0ebc0e4427a913da718189b33a0dde11339bc",
            payload["sha256"],
        )
        self.assertTrue(
            foundation._git_ignored(
                REPO_ROOT,
                ECCE_HOMO_1908_ITEM_ROOT / payload["relative_path"],
            )
        )

        inventory_file = inventory["files"][0]
        self.assertEqual("pdf_pages_v1", inventory_file["profile"])
        self.assertEqual(
            {
                "resource_count": 166,
                "page_count": 166,
                "image_resource_count": 498,
                "distinct_page_geometry_count": 5,
            },
            inventory_file["summary"],
        )
        self.assertFalse(inventory["source_text_included"])

        self.assertEqual(
            {
                "dnb_extent": "155 S.",
                "internet_archive_extent": "154 p.",
                "openlibrary_extent": "154 p.",
                "last_visible_printed_page": 154,
                "decision": (
                    "preserve all catalog claims and the source-visible endpoint; "
                    "do not silently normalize the edition extent"
                ),
            },
            source_snapshot["pagination_conflict"],
        )
        observations = source_snapshot["source_visible_scan"]["observations"]
        self.assertTrue(
            any(
                "Nachwort des Herausgebers" in observation
                and "page 137 begins Raoul Richter" in observation
                for observation in observations
            )
        )
        self.assertEqual(
            (
                "admit the 1908 physical witness and its edition-scoped expression "
                "without treating it as an author-final or critical text"
            ),
            source_snapshot["editorial_and_critical_boundary"]["decision"],
        )

        self.assertEqual("in_copyright", rights["assessment_status"])
        self.assertEqual(["DE", "US"], rights["jurisdictions_reviewed"])
        self.assertEqual("not_authorized", rights["redistribution_posture"])
        self.assertEqual("local_research_only", rights["derivative_posture"])
        self.assertEqual(3, rights["record_version"])
        self.assertIn(
            "https://commons.wikimedia.org/wiki/Template:PD-US-expired",
            rights["rights_statement_uri"],
        )
        self.assertTrue(
            any("van de Velde" in restriction for restriction in rights["restrictions"])
        )
        rights_layers = {
            layer["layer_id"].rsplit(".layer.", 1)[1]: layer
            for layer in rights["layer_assessments"]
        }
        self.assertEqual(
            {
                "original-work",
                "richter-editing",
                "richter-afterword",
                "van-de-velde-applied-art",
                "faithful-capture-process",
                "embedded-historical-ocr",
                "exact-commons-digital-object",
                "getty-holding-furniture",
                "commons-structured-metadata",
                "commons-unstructured-description",
                "internet-archive-lineage-package",
            },
            set(rights_layers),
        )
        for layer_id in (
            "original-work",
            "richter-editing",
            "richter-afterword",
            "faithful-capture-process",
            "embedded-historical-ocr",
        ):
            self.assertEqual(
                "public_domain_reviewed",
                rights_layers[layer_id]["assessment_status"],
            )
        for layer_id in (
            "original-work",
            "richter-editing",
            "richter-afterword",
        ):
            term_basis = rights_layers[layer_id]["term"]["basis"]
            self.assertIn("17 U.S.C. Section 104A", term_basis)
            self.assertNotIn("pre-1931", term_basis)
        self.assertEqual(
            "in_copyright",
            rights_layers["van-de-velde-applied-art"]["assessment_status"],
        )
        self.assertEqual(
            "2027-12-31",
            rights_layers["van-de-velde-applied-art"]["term"]["ends_on"],
        )
        self.assertIn(
            "2003-12-31",
            rights_layers["van-de-velde-applied-art"]["term"]["basis"],
        )
        self.assertIn(
            "Section 104A",
            rights_layers["van-de-velde-applied-art"]["term"]["basis"],
        )
        self.assertEqual(
            "in_copyright",
            rights_layers["exact-commons-digital-object"]["assessment_status"],
        )
        self.assertIn(
            "2003-12-31",
            rights_layers["exact-commons-digital-object"]["term"]["basis"],
        )
        self.assertEqual(
            "https://creativecommons.org/publicdomain/zero/1.0/",
            rights_layers["commons-structured-metadata"]["license_uri"],
        )
        self.assertEqual(
            "https://creativecommons.org/licenses/by-sa/4.0/",
            rights_layers["commons-unstructured-description"]["license_uri"],
        )

        self.assertEqual(
            [
                "channel-dnb-ecce-homo-authorities",
                "channel-dta-ecce-homo",
                "channel-textgrid-ecce-homo",
                "channel-hadw-ecce-homo-commentary",
                "channel-current-ecce-homo-scholarship",
                "channel-frontier-ecce-homo-2026",
                "channel-ia-openlibrary-ecce-homo",
                "channel-wikimedia-commons-ecce-homo",
                "channel-general-web-ecce-homo",
            ],
            [channel["channel_id"] for channel in discovery["channels"]],
        )
        self.assertEqual(
            ["tos-discovery-result.wikimedia-commons-ecce-homo-insel-1908"],
            discovery["selected_result_ids"],
        )
        self.assertTrue(discovery["general_web_search_is_last_resort"])
        self.assertFalse(discovery["technical_access_bypass_used"])

        self.assertEqual(6, work["record_version"])
        self.assertEqual(4, edition["record_version"])
        self.assertEqual(4, len(responsibility_claims))
        self.assertEqual(
            set(work["responsibility_claim_refs"]),
            {
                claims_by_predicate["authored_by"]["claim_id"],
            },
        )
        self.assertEqual(
            set(edition["responsibility_claim_refs"]),
            {
                claims_by_predicate["edited_by"]["claim_id"],
                claims_by_predicate["afterword_by"]["claim_id"],
                claims_by_predicate["designed_by"]["claim_id"],
            },
        )
        self.assertTrue(
            all(
                claim["claim_type"] == "bibliographic"
                and claim["assertion_layer"] == "bibliographic_assertion"
                and claim["epistemic_status"] == "observed"
                and claim["review_status"] == "unreviewed"
                and claim["reviews"] == []
                and claim["visibility"] == "public_metadata_only"
                for claim in responsibility_claims
            )
        )
        self.assertEqual(
            "tos.agent.friedrich-nietzsche",
            claims_by_predicate["authored_by"]["object"],
        )
        self.assertEqual(
            "tos.agent.raoul-richter",
            claims_by_predicate["edited_by"]["object"],
        )
        self.assertEqual(
            "tos.agent.raoul-richter",
            claims_by_predicate["afterword_by"]["object"],
        )
        self.assertEqual(
            "tos.agent.henry-van-de-velde",
            claims_by_predicate["designed_by"]["object"],
        )
        expected_agent_gnds = {
            "friedrich-nietzsche": "118587943",
            "raoul-richter": "116512857",
            "henry-van-de-velde": "118626442",
        }
        for agent_slug, expected_gnd in expected_agent_gnds.items():
            agent = json.loads(
                (
                    REPO_ROOT
                    / "ToS/source-witnesses/agents"
                    / agent_slug
                    / "agent.json"
                ).read_text(encoding="utf-8")
            )
            self.assertEqual(
                [expected_gnd],
                [
                    identifier["value"]
                    for identifier in agent["external_identifiers"]
                    if identifier["scheme"] == "GND"
                ],
            )

        self.assertFalse(server_plan["payload_transfer_authorized"])
        self.assertFalse(
            server_plan["operator_transfer_approval"]["approved_by_real_human"]
        )
        self.assertEqual("blocked-rights", server_plan["server_import_status"])
        self.assertEqual("metadata-only", server_plan["publication_status"])
        self.assertEqual(
            "restricted", server_plan["rights_policy"]["assessment_status"]
        )
        self.assertEqual(
            ["DE", "US"], server_plan["rights_policy"]["jurisdictions_reviewed"]
        )
        self.assertEqual(3, server_plan["contract_version"])
        self.assertEqual(
            hashlib.sha256(
                (ECCE_HOMO_1908_ITEM_ROOT / "rights.json").read_bytes()
            ).hexdigest(),
            server_plan["rights_policy"]["rights_record_sha256"],
        )
        self.assertIn(
            "https://www.copyright.gov/title17/92chap1.html#104a",
            server_plan["rights_policy"]["permission_or_license_refs"],
        )
        self.assertIn(
            "https://www.copyright.gov/circs/circ38b.pdf",
            server_plan["rights_policy"]["permission_or_license_refs"],
        )
        self.assertEqual(
            [
                "tos.event.server-import-plan.ecce-homo-insel-1908."
                "uraa-correction.2026-08-10"
            ],
            server_plan["provenance_event_refs"],
        )
        for derivative in (
            "ocr",
            "transcription",
            "page_images",
            "snippets",
            "embeddings",
            "alignments",
            "translations",
            "annotations",
        ):
            self.assertEqual(
                "prohibited",
                server_plan["allowed_derivatives"][derivative]["state"],
            )

        provenance_events = [
            json.loads(line)
            for line in (
                ECCE_HOMO_1908_ITEM_ROOT / "provenance.jsonl"
            ).read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        responsibility_event = next(
            event
            for event in provenance_events
            if event["event_id"]
            == "tos.event.annotation.ecce-homo."
            "insel-1908-responsibility-claims.2026-07-30"
        )
        self.assertEqual(
            "tos.event.annotation.ecce-homo."
            "insel-1908-responsibility-claims.2026-07-30",
            responsibility_event["event_id"],
        )
        self.assertEqual(
            {
                "ref": (
                    "ToS/source-witnesses/works/friedrich-nietzsche/"
                    "ecce-homo/responsibility-claims.jsonl"
                ),
                "role": "unreviewed-evidence-bearing-responsibility-claims",
                "sha256": hashlib.sha256(
                    ECCE_HOMO_RESPONSIBILITY_CLAIMS_PATH.read_bytes()
                ).hexdigest(),
            },
            responsibility_event["outputs"][0],
        )
        self.assertEqual(
            {
                "responsibility_claims_materialized": 4,
                "responsibility_claims_reviewed": 0,
                "agents_resolved": 3,
                "remote_items_created": 0,
                "source_text_admitted": False,
                "semantic_claims_created": 0,
                "canon_promotion_performed": False,
            },
            responsibility_event["method"]["configuration"],
        )
        rights_event = provenance_events[-1]
        self.assertEqual(
            "tos.event.rights-assessment.ecce-homo."
            "insel-1908.uraa-correction.2026-08-10",
            rights_event["event_id"],
        )
        self.assertEqual(
            hashlib.sha256(
                (ECCE_HOMO_1908_ITEM_ROOT / "rights.json").read_bytes()
            ).hexdigest(),
            rights_event["outputs"][0]["sha256"],
        )
        self.assertEqual(
            11, rights_event["method"]["configuration"]["layers_assessed"]
        )
        self.assertFalse(
            rights_event["method"]["configuration"]["source_text_admitted"]
        )
        self.assertFalse(
            rights_event["method"]["configuration"][
                "aggregate_rights_status_changed"
            ]
        )
        self.assertFalse(
            rights_event["method"]["configuration"][
                "provider_pre_1931_statement_used_as_independent_legal_conclusion"
            ]
        )

        server_provenance_events = {
            event["event_id"]: event
            for line in (
                REPO_ROOT / "ToS/source-witnesses/server-import/provenance.jsonl"
            ).read_text(encoding="utf-8").splitlines()
            if line.strip()
            for event in (json.loads(line),)
        }
        server_event = server_provenance_events[
            "tos.event.server-import-plan.ecce-homo-insel-1908."
            "uraa-correction.2026-08-10"
        ]
        self.assertEqual(
            hashlib.sha256(
                (
                    REPO_ROOT
                    / "ToS/source-witnesses/server-import/plans/"
                    "ecce-homo-insel-1908-wikimedia-commons-getty-scan-pdf."
                    "server-import.json"
                ).read_bytes()
            ).hexdigest(),
            server_event["outputs"][0]["sha256"],
        )
        self.assertFalse(
            server_event["method"]["configuration"][
                "aggregate_rights_status_changed"
            ]
        )

        rights_research = ECCE_HOMO_1908_LAYERED_RIGHTS_RESEARCH_PATH.read_text(
            encoding="utf-8"
        )
        ordered_sections = [
            "## Classical and official documentation",
            "## Established scholarship, cases, and institutional practice",
            "## Fresh and currently relevant checks",
            "## General web search, last",
        ]
        positions = [rights_research.index(section) for section in ordered_sections]
        self.assertEqual(sorted(positions), positions)
        self.assertIn("eleven-layer rights model", rights_research)
        self.assertIn("2027-12-31", rights_research)
        self.assertIn("2003-12-31", rights_research)
        self.assertIn("17 U.S.C.\n§104A", rights_research)
        self.assertIn("Golan v. Holder", rights_research)
        self.assertIn("bytes drifted", rights_research)
        self.assertIn("operator-held PDF is never", rights_research)

        responsibility_route = (
            REPO_ROOT
            / "ToS/research-packets/foundation-laboratory-2026-07/"
            "ECCE_HOMO_AUTHORIAL_WITNESS_ROUTE.md"
        ).read_text(encoding="utf-8")
        self.assertIn("Machine-readable responsibility prototype", responsibility_route)
        self.assertIn("`authored_by` Friedrich Nietzsche", responsibility_route)
        self.assertIn("`edited_by` Raoul Richter", responsibility_route)
        self.assertIn("`afterword_by` Raoul", responsibility_route)
        self.assertIn("`designed_by` Henry van de Velde", responsibility_route)
        self.assertIn("does not make the 1908", responsibility_route)

        catalog_ids = {
            record["record_id"]
            for catalog_name in (
                "agents.jsonl",
                "works.jsonl",
                "expressions.jsonl",
                "editions.jsonl",
                "items.jsonl",
            )
            for line in (
                REPO_ROOT / "ToS/source-witnesses/catalog" / catalog_name
            )
            .read_text(encoding="utf-8")
            .splitlines()
            if line
            for record in (json.loads(line),)
        }
        self.assertTrue(
            {
                "tos.work.friedrich-nietzsche.ecce-homo",
                "tos.expression.friedrich-nietzsche.ecce-homo."
                "de-richter-insel-1908",
                "tos.edition.friedrich-nietzsche.ecce-homo."
                "leipzig-insel-verlag-1908",
                "tos.item.friedrich-nietzsche.ecce-homo."
                "de-richter-insel-1908.wikimedia-commons-getty-scan-pdf",
                "tos.agent.friedrich-nietzsche",
                "tos.agent.raoul-richter",
                "tos.agent.henry-van-de-velde",
            }.issubset(catalog_ids)
        )

    def test_golden_kernel_transfer_plan_fails_closed_without_human_kernel(self) -> None:
        validator, _ = foundation._schema_validator(
            foundation.GOLDEN_KERNEL_TRANSFER_PLAN_SCHEMA,
            REPO_ROOT,
        )
        transfer_path = GOLD_ROOT / "transfer-samples.json"
        transfer_plan = json.loads(transfer_path.read_text(encoding="utf-8"))

        self.assertEqual([], list(validator.iter_errors(transfer_plan)))
        self.assertEqual(
            "tos_golden_kernel_transfer_plan_v2",
            transfer_plan["schema_version"],
        )
        self.assertEqual("blocked-not-run", transfer_plan["status"])
        self.assertEqual("blocked", transfer_plan["kernel_evidence_gate"]["gate_status"])
        self.assertFalse(
            transfer_plan["kernel_evidence_gate"]["semantic_transfer_claim_authorized"]
        )
        self.assertEqual(0, transfer_plan["result"]["run_count"])
        self.assertIsNone(transfer_plan["result"]["winner"])
        self.assertEqual([], transfer_plan["result"]["metric_results"])
        self.assertEqual(3, len(transfer_plan["scouting_units"]))
        self.assertEqual([], transfer_plan["target_units"])
        candidates = transfer_plan["candidate_target_units"]
        self.assertEqual(20, len(candidates))
        self.assertEqual(
            {"random": 10, "hard": 10},
            {
                stratum: sum(
                    candidate["stratum"] == stratum
                    for candidate in candidates
                )
                for stratum in ("random", "hard")
            },
        )
        self.assertEqual(
            20,
            len(
                {
                    (candidate["file_ref"], candidate["page"])
                    for candidate in candidates
                }
            ),
        )
        self.assertTrue(
            all(
                candidate["source_review_status"] == "model_source_visible"
                and candidate["target_gold_status"] == "not_started"
                and candidate["frozen_before_variant_outputs"] is True
                and candidate["eligible_for_variant_execution"] is False
                for candidate in candidates
            )
        )
        self.assertFalse(
            transfer_plan["candidate_preparation"]["human_review_performed"]
        )
        self.assertFalse(
            transfer_plan["candidate_preparation"]["variant_outputs_visible"]
        )
        self.assertTrue(
            all(
                unit["unit_kind"] == "title-page"
                and unit["selection_status"] == "scouting-only"
                and unit["eligible_for_semantic_transfer"] is False
                for unit in transfer_plan["scouting_units"]
            )
        )

        false_run = copy.deepcopy(transfer_plan)
        false_run["result"]["run_count"] = 1
        false_run["result"]["run_refs"] = ["synthetic-run"]
        self.assertTrue(list(validator.iter_errors(false_run)))

        false_authority = copy.deepcopy(transfer_plan)
        false_authority["kernel_evidence_gate"][
            "semantic_transfer_claim_authorized"
        ] = True
        self.assertTrue(list(validator.iter_errors(false_authority)))

        premature_ready = copy.deepcopy(transfer_plan)
        premature_ready["status"] = "ready"
        self.assertTrue(list(validator.iter_errors(premature_ready)))

        counters_only_ready = copy.deepcopy(transfer_plan)
        counters_only_ready["status"] = "ready"
        counters_only_ready["kernel_evidence_gate"].update(
            {
                "accepted_source_units": 30,
                "human_double_checked_gold_units": 15,
                "human_accepted_sign_packets": 1,
                "human_accepted_translation_packets": 1,
                "human_double_checked_target_units": 20,
                "gate_status": "satisfied",
                "semantic_transfer_claim_authorized": True,
                "blockers": [],
            }
        )
        self.assertTrue(list(validator.iter_errors(counters_only_ready)))

        semantic_title_page = copy.deepcopy(transfer_plan)
        semantic_title_page["scouting_units"][0][
            "eligible_for_semantic_transfer"
        ] = True
        self.assertTrue(list(validator.iter_errors(semantic_title_page)))

        false_candidate_authority = copy.deepcopy(transfer_plan)
        false_candidate_authority["candidate_target_units"][0][
            "eligible_for_variant_execution"
        ] = True
        self.assertTrue(list(validator.iter_errors(false_candidate_authority)))

        incomplete_metrics = copy.deepcopy(transfer_plan)
        incomplete_metrics["metrics"].pop()
        self.assertTrue(list(validator.iter_errors(incomplete_metrics)))

        events = [
            json.loads(line)
            for line in (GOLD_ROOT / "transfer-provenance.jsonl")
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
        ]
        event = next(
            candidate
            for candidate in events
            if candidate["event_id"] == transfer_plan["provenance_event_ref"]
        )
        output = next(
            output
            for output in event["outputs"]
            if output["ref"].endswith("/transfer-samples.json")
        )
        transfer_digest = hashlib.sha256(transfer_path.read_bytes()).hexdigest()
        self.assertEqual(
            "adad0534a5ce61f3eaa821aaa19fcc7257958c7baac06b75f30b321f65f36cb6",
            transfer_digest,
        )
        self.assertEqual(transfer_digest, output["sha256"])

        report = (
            REPO_ROOT
            / "ToS/research-packets/foundation-laboratory-2026-07/"
            "GOLDEN_KERNEL_TRANSFER_REPORT.md"
        ).read_text(encoding="utf-8")
        self.assertIn(
            "### Deferred *Jenseits* §22 textual-genetic route",
            report,
        )
        self.assertIn(
            "does not modify or supersede `transfer-samples.json`",
            report,
        )
        self.assertIn(
            "The present v1\nplan remains frozen and `blocked-not-run`.",
            report,
        )

    def test_human_gold_requires_materialized_content_and_review_receipts(self) -> None:
        validator, _ = foundation._schema_validator(
            foundation.GOLD_STATUS_SCHEMA,
            REPO_ROOT,
        )
        status = json.loads((GOLD_ROOT / "gold-status.json").read_text(encoding="utf-8"))
        unit = status["units"][0]
        unit["content_sha256"] = "a" * 64
        unit["gold_status"] = "human_double_checked"
        for field, maker in (
            ("human_pass_1", "human:reviewer-one"),
            ("human_pass_2", "human:reviewer-two"),
        ):
            unit[field] = {
                "status": "complete",
                "maker_ref": maker,
                "completed_at": "2026-07-24T02:00:00Z",
                "receipt_ref": f"local-review/{field}.json",
            }
        self.assertEqual([], list(validator.iter_errors(status)))

        for field_path in (
            ("content_sha256",),
            ("human_pass_1", "maker_ref"),
            ("human_pass_1", "completed_at"),
            ("human_pass_1", "receipt_ref"),
            ("human_pass_2", "maker_ref"),
            ("human_pass_2", "completed_at"),
            ("human_pass_2", "receipt_ref"),
        ):
            invalid = copy.deepcopy(status)
            target = invalid["units"][0]
            if len(field_path) == 1:
                target[field_path[0]] = None
            else:
                target[field_path[0]][field_path[1]] = None
            self.assertTrue(
                list(validator.iter_errors(invalid)),
                f"{'.'.join(field_path)} remained optional for human gold",
            )

    def test_manual_gold_assurance_preserves_solo_and_language_boundaries(self) -> None:
        validator, _ = foundation._schema_validator(
            foundation.GOLD_ASSURANCE_SCHEMA,
            REPO_ROOT,
        )
        assurance_path = GOLD_ROOT / "gold-assurance.v2.json"
        assurance = json.loads(assurance_path.read_text(encoding="utf-8"))
        status = json.loads((GOLD_ROOT / "gold-status.json").read_text(encoding="utf-8"))

        self.assertEqual([], list(validator.iter_errors(assurance)))
        self.assertEqual(
            {unit["sample_id"] for unit in status["units"]},
            {unit["sample_id"] for unit in assurance["units"]},
        )
        self.assertEqual(
            {
                unit["sample_id"]: unit["anchor_ref"]
                for unit in status["units"]
            },
            {
                unit["sample_id"]: unit["anchor_ref"]
                for unit in assurance["units"]
            },
        )

        russian_units = [unit for unit in assurance["units"] if unit["language"] == "ru"]
        german_units = [unit for unit in assurance["units"] if unit["language"] == "de"]
        self.assertEqual(10, len(russian_units))
        self.assertEqual(5, len(german_units))
        self.assertTrue(
            all(
                unit["current_assurance"] == "unreviewed"
                and unit["next_route"] == "none"
                for unit in russian_units
            )
        )
        self.assertTrue(
            all(
                unit["competence_level"] == "visual_only"
                and unit["current_assurance"] == "language_competence_blocked"
                and unit["next_route"] == "resolve_language_competence"
                for unit in german_units
            )
        )
        schedule = assurance["human_work_schedule"]
        self.assertFalse(schedule["packet_units_are_human_debt"])
        self.assertEqual("closed_no_human_debt", schedule["current_status"])
        self.assertEqual(0, schedule["human_debt_units"])
        self.assertEqual(
            ["tos-sample-antonovsky-p011"],
            schedule["selected_calibration_unit_ids"],
        )
        self.assertEqual(
            {unit["sample_id"] for unit in assurance["units"]},
            set(schedule["selected_calibration_unit_ids"])
            | set(schedule["unscheduled_unit_ids"]),
        )
        self.assertFalse(schedule["promotion_authorized"])
        self.assertEqual("not_collected", schedule["attestation_status"])
        for field in ("runtime_closure", "runtime_receipt", "source_autosave"):
            local_ref = schedule[field]
            self.assertEqual("abyss-stack", local_ref["owner"])
            self.assertEqual(
                "abyss_machine_artifact_store",
                local_ref["artifact_root"],
            )
            self.assertFalse(Path(local_ref["relative_path"]).is_absolute())
            self.assertNotIn("..", Path(local_ref["relative_path"]).parts)

    def test_private_evidence_handoff_records_bounded_derivative_transition(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.PRIVATE_EVIDENCE_HANDOFF_SCHEMA,
            REPO_ROOT,
        )
        handoff = json.loads(PRIVATE_HANDOFF_PATH.read_text(encoding="utf-8"))
        self.assertEqual([], list(validator.iter_errors(handoff)))
        self.assertEqual(
            [],
            foundation._private_evidence_handoff_issues(
                handoff,
                repo_root=REPO_ROOT,
            ),
        )
        self.assertTrue(handoff["effects"]["raw_read"])
        self.assertTrue(handoff["effects"]["derivative_created"])
        self.assertFalse(handoff["effects"]["publication"])
        self.assertFalse(handoff["destination"]["publication_authority"])
        self.assertEqual(
            "derivative_prepared_private",
            handoff["status"],
        )
        self.assertEqual(
            "operator_goal_authorized",
            handoff["review_gate"]["derivative_preparation_authority"],
        )

        unauthorized_publication = copy.deepcopy(handoff)
        unauthorized_publication["effects"]["publication"] = True
        self.assertTrue(list(validator.iter_errors(unauthorized_publication)))

        missing_prohibition = copy.deepcopy(handoff)
        missing_prohibition["disclosure_policy"]["forbidden_classes"].remove(
            "source_text_or_transcription"
        )
        self.assertTrue(
            foundation._private_evidence_handoff_issues(
                missing_prohibition,
                repo_root=REPO_ROOT,
            )
        )

    def test_public_evidence_derivative_has_payload_and_cross_closes_to_handoff(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.PUBLIC_EVIDENCE_DERIVATIVE_SCHEMA,
            REPO_ROOT,
        )
        handoff = json.loads(PRIVATE_HANDOFF_PATH.read_text(encoding="utf-8"))
        derivative = {
            "$schema": "https://tree-of-sophia.local/ToS/contracts/public-laboratory-evidence-derivative.schema.json",
            "schema_version": "tos_public_laboratory_evidence_derivative_v1",
            "derivative_id": "tos.public-evidence-derivative.synthetic-v1",
            "status": "prepared_local_publication_blocked",
            "handoff_id": handoff["handoff_id"],
            "source_boundary": {
                "evidence_set_id": handoff["source_boundary"]["evidence_set_id"],
                "public_return_handle": handoff["source_boundary"]["public_return_handle"],
                "raw_evidence_embedded": False,
                "source_content_embedded": False,
                "private_locator_embedded": False,
                "unit_level_judgments_embedded": False,
            },
            "public_experiment_id": "tos-ocr-candidate-review-v1",
            "disclosed_classes": handoff["disclosure_policy"]["allowed_classes"],
            "method_family": [
                {"method_id": "synthetic-a", "role": "baseline"}
            ],
            "aggregation": {
                "source_unit_count": 3,
                "candidate_observation_count": 3,
                "minimum_group_size_applied": 3,
                "suppressed_small_cell_count": 0,
            },
            "aggregate_outcome_counts": [
                {"code": "reviewed", "count": 3}
            ],
            "aggregate_error_taxonomy": [],
            "aggregate_machine_cost": [
                {
                    "measurement_id": "synthetic-wall",
                    "method_id": "synthetic-a",
                    "measurement_status": "not_measured",
                    "value": None,
                    "unit": "not_applicable",
                    "confounds": [],
                }
            ],
            "aggregate_human_time_with_confounds": {
                "measurement_status": "not_measured",
                "value": None,
                "unit": "not_applicable",
                "confounds": [],
            },
            "evidence_posture": {
                "raw_preserved": True,
                "negative_results_preserved": True,
                "human_source_visible_review_observed": True,
                "independent_human_gold": False,
                "general_method_winner": False,
                "content_authority": False,
            },
            "public_source_refs": [
                "ToS/research-packets/foundation-laboratory-2026-07/HUMAN_GOLD_REVIEW_PACKET.md"
            ],
            "limitations": [
                "Synthetic fixture contains no source or unit-level evidence.",
                "Synthetic fixture establishes no method quality conclusion.",
                "Synthetic fixture grants no publication or content authority.",
            ],
            "review_gate": {
                "human_publication_approval": False,
                "rights_review": "not_performed",
                "correlation_review": "not_performed",
            },
            "effects": {
                "tracked_derivative_created": True,
                "repository_publication": False,
                "source_text_accepted": False,
                "translation_accepted": False,
                "semantic_claim_accepted": False,
                "rights_cleared": False,
                "canon_promoted": False,
            },
            "claim_limit": "aggregate derivative reports bounded laboratory evidence only; it does not establish source text, translation, semantics, rights, canon, or a method winner",
        }
        self.assertEqual([], list(validator.iter_errors(derivative)))
        self.assertEqual(
            [],
            foundation._public_evidence_derivative_issues(
                derivative,
                handoff=handoff,
            ),
        )

        leaked_path = copy.deepcopy(derivative)
        leaked_path["limitations"].append("private locator /srv/example")
        self.assertTrue(
            foundation._public_evidence_derivative_issues(
                leaked_path,
                handoff=handoff,
            )
        )

    def test_manual_error_ledger_preserves_bounded_aggregate_review_episode(
        self,
    ) -> None:
        ledger_validator, _ = foundation._schema_validator(
            foundation.MANUAL_ERROR_LEDGER_SCHEMA,
            REPO_ROOT,
        )
        provenance_validator, _ = foundation._schema_validator(
            foundation.PROVENANCE_SCHEMA,
            REPO_ROOT,
        )
        records = [
            json.loads(line)
            for line in MANUAL_ERROR_LEDGER_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
        ]
        events = [
            json.loads(line)
            for line in MANUAL_ERROR_LEDGER_PROVENANCE_PATH.read_text(
                encoding="utf-8"
            ).splitlines()
        ]
        for record in records:
            self.assertEqual([], list(ledger_validator.iter_errors(record)))
        for event in events:
            self.assertEqual([], list(provenance_validator.iter_errors(event)))

        self.assertEqual(
            ["ledger_state", "review_episode"],
            [record["record_type"] for record in records],
        )
        episode = records[1]
        scope = episode["review_scope"]
        self.assertEqual(10, scope["source_unit_count"])
        self.assertEqual(30, scope["candidate_observation_count"])
        self.assertEqual(1, scope["independent_pass_count"])
        self.assertTrue(scope["real_human_review_observed"])
        self.assertTrue(scope["solo_reviewer"])
        self.assertTrue(scope["source_visible"])
        self.assertTrue(scope["method_blind"])

        handoff = json.loads(PRIVATE_HANDOFF_PATH.read_text(encoding="utf-8"))
        derivative_path = REPO_ROOT / handoff["destination"]["artifact_path"]
        derivative = json.loads(derivative_path.read_text(encoding="utf-8"))
        self.assertEqual(
            derivative["aggregate_outcome_counts"],
            episode["aggregate_outcome_counts"],
        )
        self.assertEqual(
            derivative["aggregate_error_taxonomy"],
            episode["aggregate_error_taxonomy"],
        )
        self.assertEqual(
            derivative["aggregate_human_time_with_confounds"],
            episode["human_time"],
        )
        self.assertEqual(
            [],
            foundation._manual_error_ledger_issues(
                records,
                handoff=handoff,
                derivative=derivative,
                provenance_events=events,
                repo_root=REPO_ROOT,
            ),
        )

        serialized = json.dumps([episode, events], ensure_ascii=False)
        self.assertNotIn("/srv/", serialized)
        self.assertNotIn("/home/", serialized)
        self.assertNotIn("local-content", serialized)
        self.assertFalse(episode["evidence_boundary"]["source_content_embedded"])
        self.assertFalse(
            episode["evidence_boundary"]["unit_level_judgments_embedded"]
        )
        self.assertEqual(0, episode["adjudication"]["source_transcriptions_accepted"])
        self.assertEqual(0, episode["adjudication"]["independent_gold_units"])
        self.assertFalse(episode["adjudication"]["general_method_winner"])
        self.assertFalse(episode["adjudication"]["content_authority"])
        self.assertFalse(episode["adjudication"]["routine_human_backlog_created"])

        false_promotion = copy.deepcopy(episode)
        false_promotion["adjudication"]["source_transcriptions_accepted"] = 1
        self.assertTrue(list(ledger_validator.iter_errors(false_promotion)))

    def test_transfer_candidate_crosswalk_closes_to_frozen_structural_inputs(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.TRANSFER_CANDIDATE_CROSSWALK_SCHEMA,
            REPO_ROOT,
        )
        crosswalk = json.loads(
            TRANSFER_CANDIDATE_CROSSWALK_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual([], list(validator.iter_errors(crosswalk)))

        loaded_inputs: dict[str, object] = {}
        for name, digest_bound_ref in crosswalk["inputs"].items():
            input_path = REPO_ROOT / digest_bound_ref["ref"]
            self.assertEqual(
                digest_bound_ref["sha256"],
                hashlib.sha256(input_path.read_bytes()).hexdigest(),
            )
            if name in {
                "transfer_plan",
                "target_numbered_unit_map",
                "shared_label_correspondence",
            }:
                loaded_inputs[name] = json.loads(
                    input_path.read_text(encoding="utf-8")
                )

        self.assertEqual(
            [],
            foundation._transfer_candidate_crosswalk_issues(
                crosswalk,
                transfer_plan=loaded_inputs["transfer_plan"],
                target_map=loaded_inputs["target_numbered_unit_map"],
                label_map=loaded_inputs["shared_label_correspondence"],
            ),
        )
        self.assertEqual(8, crosswalk["summary"]["candidate_page_count"])
        self.assertEqual(15, crosswalk["summary"]["possible_pairing_count"])
        self.assertEqual(0, crosswalk["summary"]["human_review_count"])
        self.assertEqual(0, crosswalk["summary"]["eligible_target_unit_count"])

        drifted_page = copy.deepcopy(crosswalk)
        drifted_page["candidates"][0]["target_pdf_page"] += 1
        self.assertIn(
            "tos-target-candidate-jenseits-p0308-random candidate page drifted",
            foundation._transfer_candidate_crosswalk_issues(
                drifted_page,
                transfer_plan=loaded_inputs["transfer_plan"],
                target_map=loaded_inputs["target_numbered_unit_map"],
                label_map=loaded_inputs["shared_label_correspondence"],
            ),
        )

    def test_hierarchical_target_maps_and_crosswalks_close_without_text_authority(
        self,
    ) -> None:
        map_validator, _ = foundation._schema_validator(
            foundation.HIERARCHICAL_TARGET_NUMBERED_UNIT_MAP_SCHEMA,
            REPO_ROOT,
        )
        crosswalk_validator, _ = foundation._schema_validator(
            foundation.TARGET_STRUCTURAL_CROSSWALK_SCHEMA,
            REPO_ROOT,
        )
        expected = {
            "tos.work.friedrich-nietzsche.zur-genealogie-der-moral": {
                "map": (4, 78, 71, 7),
                "crosswalk": (6, 8, 2, 0),
            },
            "tos.work.friedrich-nietzsche.der-antichrist": {
                "map": (1, 62, 52, 10),
                "crosswalk": (6, 12, 5, 0),
            },
        }

        for structure_root in HIERARCHICAL_TARGET_STRUCTURE_ROOTS:
            map_path = structure_root / "hierarchical-numbered-unit-page-map.json"
            crosswalk_path = (
                structure_root / "transfer-candidate-page-crosswalk.v1.json"
            )
            target_map = json.loads(map_path.read_text(encoding="utf-8"))
            crosswalk = json.loads(crosswalk_path.read_text(encoding="utf-8"))
            work_expected = expected[target_map["work_ref"]]

            self.assertEqual([], list(map_validator.iter_errors(target_map)))
            self.assertEqual(
                [],
                foundation._hierarchical_target_numbered_unit_map_issues(
                    target_map
                ),
            )
            for binding_name in ("inventory", "work_boundary"):
                binding = target_map[binding_name]
                bound_path = REPO_ROOT / binding["ref"]
                self.assertEqual(
                    binding["sha256"],
                    hashlib.sha256(bound_path.read_bytes()).hexdigest(),
                )

            map_summary = target_map["summary"]
            self.assertEqual(
                work_expected["map"],
                (
                    map_summary["series_count"],
                    map_summary["numbered_unit_count"],
                    map_summary["ordered_bbox_candidate_match_count"],
                    map_summary["source_visible_override_unit_count"],
                ),
            )
            self.assertFalse(target_map["target_text_included"])
            self.assertFalse(map_summary["human_review_performed"])

            self.assertEqual(
                [], list(crosswalk_validator.iter_errors(crosswalk))
            )
            loaded_inputs: dict[str, object] = {}
            for name, binding in crosswalk["inputs"].items():
                bound_path = REPO_ROOT / binding["ref"]
                self.assertEqual(
                    binding["sha256"],
                    hashlib.sha256(bound_path.read_bytes()).hexdigest(),
                )
                if name == "transfer_plan":
                    loaded_inputs[name] = json.loads(
                        bound_path.read_text(encoding="utf-8")
                    )
            self.assertEqual(
                [],
                foundation._target_structural_crosswalk_issues(
                    crosswalk,
                    transfer_plan=loaded_inputs["transfer_plan"],
                    target_map=target_map,
                ),
            )
            crosswalk_summary = crosswalk["summary"]
            self.assertEqual(
                work_expected["crosswalk"],
                (
                    crosswalk_summary["candidate_page_count"],
                    crosswalk_summary["possible_target_unit_route_count"],
                    crosswalk_summary["page_with_unit_start_count"],
                    crosswalk_summary["source_parallel_route_count"],
                ),
            )
            self.assertEqual(0, crosswalk_summary["human_review_count"])
            self.assertEqual(0, crosswalk_summary["eligible_target_unit_count"])
            self.assertEqual(0, crosswalk_summary["target_gold_count"])
            self.assertFalse(crosswalk["source_text_included"])
            self.assertFalse(crosswalk["target_text_included"])

            drifted = copy.deepcopy(crosswalk)
            drifted["candidates"][0]["possible_target_unit_refs"] = [
                "invented-series:999"
            ]
            self.assertTrue(
                foundation._target_structural_crosswalk_issues(
                    drifted,
                    transfer_plan=loaded_inputs["transfer_plan"],
                    target_map=target_map,
                )
            )

    def test_german_assisted_review_opens_only_bounded_experiment_lanes(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.GERMAN_ASSISTED_SOURCE_REVIEW_SCHEMA,
            REPO_ROOT,
        )
        plan = json.loads(
            GERMAN_ASSISTED_REVIEW_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual([], list(validator.iter_errors(plan)))
        self.assertEqual("visual_only", plan["competence_boundary"]["operator_german_competence"])
        self.assertFalse(
            plan["competence_boundary"][
                "machine_agreement_supplies_language_competence"
            ]
        )
        self.assertEqual(30, plan["current_state"]["prepared_units"])
        self.assertEqual(
            ["tos-translation-source-review-v2-001"],
            plan["current_state"]["selected_unit_ids"],
        )
        self.assertEqual(0, plan["current_state"]["human_debt_units"])
        self.assertEqual(1, plan["current_state"]["runs"])
        self.assertEqual(
            1,
            plan["current_state"]["machine_triangulated_units"],
        )
        self.assertEqual(0, plan["current_state"]["accepted_german_units"])
        self.assertEqual(
            1,
            plan["current_state"][
                "prepared_critical_edition_witness_packets"
            ],
        )
        self.assertEqual(
            ["ai_only", "ai_human"],
            plan["current_state"]["translation_lanes_opened"],
        )
        self.assertEqual(
            1, plan["current_state"]["admitted_critical_edition_units"]
        )
        self.assertFalse(plan["current_state"]["promotion_authorized"])

        false_acceptance = copy.deepcopy(plan)
        false_acceptance["current_state"]["accepted_german_units"] = 1
        self.assertTrue(list(validator.iter_errors(false_acceptance)))

        false_competence = copy.deepcopy(plan)
        false_competence["competence_boundary"][
            "machine_agreement_supplies_language_competence"
        ] = True
        self.assertTrue(list(validator.iter_errors(false_competence)))

        false_human_only = copy.deepcopy(plan)
        false_human_only["current_state"]["translation_lanes_opened"].append(
            "human_only"
        )
        self.assertTrue(list(validator.iter_errors(false_human_only)))

    def test_german_source_triangulation_is_machine_only_and_text_free(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.GERMAN_SOURCE_TRIANGULATION_SCHEMA,
            REPO_ROOT,
        )
        packet = json.loads(
            GERMAN_SOURCE_TRIANGULATION_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual([], list(validator.iter_errors(packet)))
        self.assertEqual("machine_triangulated_candidate", packet["status"])
        self.assertEqual(
            12,
            packet["results"]["dta_exact_comparison"]["equal_paragraphs"],
        )
        self.assertEqual(
            261,
            packet["results"]["dta_exact_comparison"]["equal_tokens"],
        )
        self.assertEqual(
            260,
            packet["results"]["naumann_ocr_comparison"][
                "equal_reference_tokens"
            ],
        )
        self.assertEqual(
            3,
            packet["results"]["normalization_failure_control"][
                "naive_generic_whitespace_join_false_token_splits"
            ],
        )
        self.assertFalse(
            packet["method"]["model_used_for_text_decision"]
        )
        self.assertFalse(packet["method"]["source_text_emitted"])
        self.assertFalse(
            packet["inputs"]["ekgwb"]["source_text_tracked"]
        )
        self.assertTrue(
            packet["rights_and_transport_boundary"][
                "unencrypted_transport_is_authenticity_risk"
            ]
        )
        self.assertEqual([], packet["gate_effects"]["translation_lanes_opened"])
        self.assertEqual(0, packet["gate_effects"]["accepted_german_units"])
        self.assertFalse(packet["gate_effects"]["promotion_authorized"])

        false_acceptance = copy.deepcopy(packet)
        false_acceptance["gate_effects"]["accepted_german_units"] = 1
        self.assertTrue(list(validator.iter_errors(false_acceptance)))

        false_rights_clearance = copy.deepcopy(packet)
        false_rights_clearance["rights_and_transport_boundary"][
            "rights_clearance_claimed"
        ] = True
        self.assertTrue(list(validator.iter_errors(false_rights_clearance)))

        tracked_source_text = copy.deepcopy(packet)
        tracked_source_text["inputs"]["ekgwb"]["source_text_tracked"] = True
        self.assertTrue(list(validator.iter_errors(tracked_source_text)))

        raw_tokens = triangulation_builder._alpha_tokens("prüf¬ wort")
        source_aware_tokens = triangulation_builder._alpha_tokens(
            triangulation_builder.re.sub(r"¬\s*", "", "prüf¬ wort")
        )
        self.assertEqual(["prüf", "wort"], raw_tokens)
        self.assertEqual(["prüfwort"], source_aware_tokens)

    def test_bounded_translation_input_opens_only_local_calibration(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.BOUNDED_TRANSLATION_RESEARCH_INPUT_SCHEMA,
            REPO_ROOT,
        )
        packet = json.loads(
            BOUNDED_TRANSLATION_RESEARCH_INPUT_PATH.read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual([], list(validator.iter_errors(packet)))
        self.assertEqual(
            "eligible_for_local_machine_calibration",
            packet["status"],
        )
        self.assertEqual(
            "translation_method_calibration",
            packet["experiment_scope"]["purpose"],
        )
        self.assertFalse(
            packet["source_derivation"]["model_used_for_boundary_or_text"]
        )
        self.assertFalse(
            packet["source_derivation"][
                "source_text_emitted_to_tracked_packet"
            ]
        )
        self.assertTrue(packet["local_artifact"]["gitignored"])
        self.assertFalse(
            packet["local_artifact"][
                "source_text_copied_into_tracked_admission"
            ]
        )
        self.assertFalse(
            packet["experiment_scope"][
                "preexisting_authored_translation_surfaces_visible"
            ]
        )
        self.assertEqual(
            20,
            packet["local_artifact"]["normalized_alpha_tokens"],
        )
        self.assertEqual(
            0,
            packet["gate_effects"]["accepted_german_units"],
        )
        self.assertEqual(
            [],
            packet["gate_effects"]["accepted_translation_lanes_opened"],
        )
        self.assertEqual(
            0,
            packet["gate_effects"]["semantic_tasks_opened"],
        )
        self.assertFalse(
            packet["gate_effects"][
                "canon_or_graph_promotion_authorized"
            ]
        )

        false_acceptance = copy.deepcopy(packet)
        false_acceptance["gate_effects"]["accepted_german_units"] = 1
        self.assertTrue(list(validator.iter_errors(false_acceptance)))

        false_publication = copy.deepcopy(packet)
        false_publication["rights_and_visibility"][
            "public_site_upload_authorized"
        ] = True
        self.assertTrue(list(validator.iter_errors(false_publication)))

        tracked_text = copy.deepcopy(packet)
        tracked_text["local_artifact"][
            "source_text_copied_into_tracked_admission"
        ] = True
        self.assertTrue(list(validator.iter_errors(tracked_text)))

        contaminated_blind_run = copy.deepcopy(packet)
        contaminated_blind_run["experiment_scope"][
            "preexisting_authored_translation_surfaces_visible"
        ] = True
        self.assertTrue(list(validator.iter_errors(contaminated_blind_run)))

    def test_bounded_translation_input_builder_uses_synthetic_source_only(
        self,
    ) -> None:
        sentence = bounded_input_builder._first_sentence(
            "Erste synthetische Aussage. Zweite Aussage."
        )
        self.assertEqual("Erste synthetische Aussage.", sentence)
        self.assertEqual(
            "Mehrere Leerzeichen bleiben lesbar.",
            bounded_input_builder._first_sentence(
                "Mehrere   Leerzeichen\nbleiben lesbar. Danach."
            ),
        )
        with self.assertRaises(
            bounded_input_builder.BoundedInputBuildError
        ):
            bounded_input_builder._first_sentence(
                "Synthetischer Text ohne Abschluss"
            )

    def test_critical_edition_witness_freezes_metadata_without_admission(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.CRITICAL_EDITION_WITNESS_ADMISSION_SCHEMA,
            REPO_ROOT,
        )
        packet = json.loads(
            CRITICAL_EDITION_WITNESS_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual([], list(validator.iter_errors(packet)))
        self.assertEqual(
            "https://www.nietzschesource.org/eKGWB/Za-I-Vorrede-1",
            packet["target"]["critical_locator_url"],
        )
        self.assertEqual(
            "local_structure_compatible_exact_critical_passage_unverified",
            packet["target"]["alignment_state"],
        )
        self.assertEqual(
            "local_section_boundary_observed_critical_text_not_compared",
            packet["local_structural_context"]["state"],
        )
        self.assertFalse(
            packet["local_structural_context"]["comparison"][
                "exact_critical_text_compared"
            ]
        )
        self.assertFalse(
            packet["local_structural_context"]["comparison"][
                "exact_passage_alignment_claimed"
            ]
        )
        self.assertFalse(packet["content_boundary"]["source_text_stored"])
        self.assertFalse(
            packet["rights_gate"]["citation_witness_admission_authorized"]
        )
        self.assertFalse(
            packet["gate_effects"]["critical_edition_unit_admitted"]
        )
        self.assertEqual([], packet["gate_effects"]["translation_lanes_opened"])
        self.assertFalse(packet["gate_effects"]["human_task_created"])

        false_admission = copy.deepcopy(packet)
        false_admission["gate_effects"][
            "critical_edition_unit_admitted"
        ] = True
        self.assertTrue(list(validator.iter_errors(false_admission)))

        captured_text = copy.deepcopy(packet)
        captured_text["content_boundary"]["source_text_stored"] = True
        self.assertTrue(list(validator.iter_errors(captured_text)))

        source_review_plan = json.loads(
            (GOLD_ROOT / "translation-source-review-plan.v2.json").read_text(
                encoding="utf-8"
            )
        )
        visual_sample_plan = json.loads(
            (GOLD_ROOT / "ocr-visual-samples.json").read_text(encoding="utf-8")
        )
        target_unit = source_review_plan["units"][0]
        self.assertEqual(
            [],
            foundation._critical_edition_local_structural_context_issues(
                packet,
                target_unit=target_unit,
                source_review_plan=source_review_plan,
                ocr_sample_plan=visual_sample_plan,
            ),
        )

        drifted_member = copy.deepcopy(packet)
        drifted_member["local_structural_context"]["epub_witness"][
            "section_start_member"
        ]["path"] = "EPUB/page_56.html"
        self.assertIn(
            "critical-edition local section start drifted from the target source unit",
            foundation._critical_edition_local_structural_context_issues(
                drifted_member,
                target_unit=target_unit,
                source_review_plan=source_review_plan,
                ocr_sample_plan=visual_sample_plan,
            ),
        )

        drifted_page = copy.deepcopy(packet)
        drifted_page["local_structural_context"]["visual_witness"][
            "section_start_pdf_page"
        ] = 57
        alignment_issues = (
            foundation._critical_edition_local_structural_context_issues(
                drifted_page,
                target_unit=target_unit,
                source_review_plan=source_review_plan,
                ocr_sample_plan=visual_sample_plan,
            )
        )
        self.assertIn(
            "critical-edition local section start page drifted from the target source unit",
            alignment_issues,
        )
        self.assertIn(
            "critical-edition local section start anchor is absent or ambiguous in the visual sample plan",
            alignment_issues,
        )

    def test_citation_witness_decision_records_bounded_human_admission(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.CRITICAL_EDITION_CITATION_WITNESS_DECISION_SCHEMA,
            REPO_ROOT,
        )
        packet = json.loads(
            CRITICAL_EDITION_CITATION_DECISION_PATH.read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual([], list(validator.iter_errors(packet)))
        self.assertEqual("human_admitted_with_limits", packet["status"])
        self.assertEqual(
            "admit-with-limits", packet["human_decision"]["decision_status"]
        )
        self.assertEqual(
            "human:dionysus", packet["human_decision"]["reviewer_ref"]
        )
        self.assertTrue(
            packet["gate_effects"]["critical_edition_unit_admitted"]
        )
        self.assertEqual(
            ["ai_only", "ai_human"],
            packet["gate_effects"]["translation_lanes_opened"],
        )
        self.assertFalse(packet["gate_effects"]["accepted_german_unit_created"])
        self.assertFalse(packet["gate_effects"]["human_only_lane_opened"])
        self.assertFalse(
            packet["gate_effects"]["recognized_translation_revealed"]
        )
        self.assertFalse(
            packet["gate_effects"]["semantic_or_canon_promotion_authorized"]
        )
        self.assertFalse(
            packet["transport_and_fixity"][
                "publisher_authentication_established"
            ]
        )
        self.assertFalse(
            packet["transport_and_fixity"][
                "german_linguistic_correctness_established"
            ]
        )

        pending = copy.deepcopy(packet)
        pending["status"] = "awaiting_human_decision"
        pending["human_decision"] = {
            "decision_status": "pending",
            "reviewer_ref": None,
            "reviewed_at": None,
            "bibliographic_identity_accepted": None,
            "bounded_rights_scope_accepted": None,
            "citation_witness_admitted": None,
            "rationale": None,
        }
        pending["gate_effects"]["critical_edition_unit_admitted"] = False
        pending["gate_effects"]["translation_lanes_opened"] = []
        self.assertEqual([], list(validator.iter_errors(pending)))

        false_admission = copy.deepcopy(pending)
        false_admission["gate_effects"][
            "critical_edition_unit_admitted"
        ] = True
        self.assertTrue(list(validator.iter_errors(false_admission)))

        silent_human = copy.deepcopy(pending)
        silent_human["status"] = "human_admitted_with_limits"
        self.assertTrue(list(validator.iter_errors(silent_human)))

        unbound_supersession = copy.deepcopy(packet)
        unbound_supersession["status"] = "superseded"
        self.assertTrue(list(validator.iter_errors(unbound_supersession)))

        research_binding = packet["evidence"]["ordered_research_refresh"]
        research_path = REPO_ROOT / research_binding["ref"]
        self.assertIsNone(
            foundation._digest_bound_ref_issue(
                research_binding,
                expected_path=research_path,
                repo_root=REPO_ROOT,
                field="evidence.ordered_research_refresh",
            )
        )
        tampered_binding = copy.deepcopy(research_binding)
        tampered_binding["sha256"] = "0" * 64
        self.assertEqual(
            "evidence.ordered_research_refresh digest drifted",
            foundation._digest_bound_ref_issue(
                tampered_binding,
                expected_path=research_path,
                repo_root=REPO_ROOT,
                field="evidence.ordered_research_refresh",
            ),
        )

    def test_edition_reading_admission_separates_source_from_language_truth(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.EDITION_READING_ADMISSION_SCHEMA,
            REPO_ROOT,
        )
        packet = json.loads(
            EDITION_READING_ADMISSION_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual([], list(validator.iter_errors(packet)))
        self.assertEqual("edition_reading_attested", packet["status"])
        self.assertEqual(
            "edition_reading_attested",
            packet["reading_evidence"]["reading_posture"],
        )
        self.assertEqual(
            "TEI/text[1]/body[1]/div[1]/div[1]",
            packet["reading_evidence"]["source_selector"],
        )
        self.assertEqual(12, packet["reading_evidence"]["paragraph_count"])
        self.assertEqual(
            261,
            packet["reading_evidence"]["normalized_token_count"],
        )
        self.assertTrue(
            packet["reading_evidence"][
                "exact_after_source_aware_normalization"
            ]
        )
        self.assertFalse(
            packet["reading_evidence"]["tracked_packet_contains_source_text"]
        )
        self.assertEqual(
            1,
            packet["gate_effects"]["edition_reading_units_attested"],
        )
        self.assertEqual(0, packet["gate_effects"]["accepted_german_units"])
        self.assertFalse(
            packet["gate_effects"][
                "german_linguistic_correctness_established"
            ]
        )
        self.assertEqual(
            0,
            packet["gate_effects"]["accepted_translation_units"],
        )
        self.assertTrue(
            packet["gate_effects"][
                "observational_semantic_materialization_allowed"
            ]
        )
        self.assertEqual(0, packet["gate_effects"]["sign_candidates_created"])
        self.assertEqual(0, packet["gate_effects"]["semantic_claims_created"])
        self.assertEqual(0, packet["gate_effects"]["human_tasks_created"])
        self.assertFalse(packet["gate_effects"]["promotion_authorized"])

        for field, binding in packet["evidence"].items():
            expected_path = REPO_ROOT / binding["ref"]
            self.assertIsNone(
                foundation._digest_bound_ref_issue(
                    binding,
                    expected_path=expected_path,
                    repo_root=REPO_ROOT,
                    field=f"evidence.{field}",
                )
            )

        false_language_acceptance = copy.deepcopy(packet)
        false_language_acceptance["gate_effects"]["accepted_german_units"] = 1
        self.assertTrue(list(validator.iter_errors(false_language_acceptance)))

        tracked_source = copy.deepcopy(packet)
        tracked_source["reading_evidence"][
            "tracked_packet_contains_source_text"
        ] = True
        self.assertTrue(list(validator.iter_errors(tracked_source)))

        false_publication = copy.deepcopy(packet)
        false_publication["rights_and_visibility"][
            "payload_publication_authorized"
        ] = True
        self.assertTrue(list(validator.iter_errors(false_publication)))

        false_promotion = copy.deepcopy(packet)
        false_promotion["gate_effects"]["promotion_authorized"] = True
        self.assertTrue(list(validator.iter_errors(false_promotion)))

    def test_experimental_translation_candidate_reuses_run_without_promotion(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.EXPERIMENTAL_TRANSLATION_CANDIDATE_SCHEMA,
            REPO_ROOT,
        )
        packet = json.loads(
            EXPERIMENTAL_TRANSLATION_CANDIDATE_PATH.read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual([], list(validator.iter_errors(packet)))
        self.assertEqual(
            "frozen-ai-only-experimental-candidate", packet["status"]
        )
        self.assertEqual(
            "reject-before-human-review", packet["assessment"]["disposition"]
        )
        self.assertFalse(packet["assessment"]["human_review_performed"])
        self.assertFalse(packet["assessment"]["translation_accepted"])
        self.assertFalse(packet["content_boundary"]["source_text_tracked"])
        self.assertFalse(packet["content_boundary"]["candidate_text_tracked"])
        self.assertFalse(
            packet["content_boundary"]["recognized_comparator_revealed"]
        )
        self.assertEqual(
            0, packet["gate_effects"]["accepted_translation_packets"]
        )
        self.assertEqual(0, packet["gate_effects"]["semantic_tasks_opened"])

        prospective_candidate = copy.deepcopy(packet)
        prospective_candidate["packet_id"] = (
            "tos.experimental-translation-candidate.future.variant-d.v1"
        )
        prospective_candidate["historical_private_run"]["run_id"] = (
            "future-translation-run-v1-20260808"
        )
        prospective_candidate["historical_private_run"]["variant"] = "D"
        prospective_candidate["historical_private_run"]["model"][
            "model_id"
        ] = "local/future-model"
        prospective_candidate["assessment"]["russian_surface_posture"] = (
            "review-worthy"
        )
        prospective_candidate["assessment"]["observed_issues"] = []
        prospective_candidate["assessment"]["disposition"] = (
            "advance-to-human-russian-review"
        )
        prospective_candidate["provenance_event_ref"] = (
            "tos.event.translation.future-experimental-candidate.2026-08-08"
        )
        self.assertEqual(
            [], list(validator.iter_errors(prospective_candidate))
        )

        for field, expected_path in (
            (
                "citation_witness_decision",
                CRITICAL_EDITION_CITATION_DECISION_PATH,
            ),
            (
                "current_source_return_overlay",
                BOUNDED_TRANSLATION_RESEARCH_INPUT_PATH,
            ),
        ):
            self.assertIsNone(
                foundation._digest_bound_ref_issue(
                    packet["admission"][field],
                    expected_path=expected_path,
                    repo_root=REPO_ROOT,
                    field=f"admission.{field}",
                )
            )

        tracked_candidate = copy.deepcopy(packet)
        tracked_candidate["content_boundary"]["candidate_text_tracked"] = True
        self.assertTrue(list(validator.iter_errors(tracked_candidate)))

        simulated_human = copy.deepcopy(packet)
        simulated_human["assessment"]["human_review_performed"] = True
        self.assertTrue(list(validator.iter_errors(simulated_human)))

        false_promotion = copy.deepcopy(packet)
        false_promotion["gate_effects"]["accepted_translation_packets"] = 1
        self.assertTrue(list(validator.iter_errors(false_promotion)))

    def test_experimental_translation_episode_preserves_failure_and_rejection_without_promotion(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.EXPERIMENTAL_TRANSLATION_EPISODE_SCHEMA,
            REPO_ROOT,
        )
        failure = json.loads(
            EXPERIMENTAL_TRANSLATION_FAILURE_EPISODE_PATH.read_text(
                encoding="utf-8"
            )
        )
        rejection = json.loads(
            EXPERIMENTAL_TRANSLATION_REJECTION_EPISODE_PATH.read_text(
                encoding="utf-8"
            )
        )
        uncertain = json.loads(
            EXPERIMENTAL_TRANSLATION_UNCERTAIN_EPISODE_PATH.read_text(
                encoding="utf-8"
            )
        )

        self.assertEqual([], list(validator.iter_errors(failure)))
        self.assertEqual([], list(validator.iter_errors(rejection)))
        self.assertEqual([], list(validator.iter_errors(uncertain)))
        self.assertEqual("failed-and-retained", failure["status"])
        self.assertIsNone(failure["private_run"]["candidate_artifact"])
        self.assertEqual(
            "runtime-contract-incompatibility-before-generation",
            failure["outcome"]["failure_class"],
        )
        self.assertTrue(failure["outcome"]["corrected_after_no_candidate"])
        self.assertEqual(
            "frozen-ai-only-experimental-candidate", rejection["status"]
        )
        self.assertEqual(
            "reject-before-human-review", rejection["outcome"]["disposition"]
        )
        self.assertFalse(rejection["outcome"]["human_review_created"])
        self.assertEqual(
            0, rejection["gate_effects"]["accepted_translation_packets"]
        )
        self.assertEqual(
            542,
            rejection["measurements"]["generation"]["completion_eval_runs"],
        )
        self.assertEqual(
            "frozen-ai-only-experimental-candidate", uncertain["status"]
        )
        self.assertEqual(
            "retain-for-method-comparison",
            uncertain["outcome"]["disposition"],
        )
        self.assertEqual(
            "uncertain", uncertain["outcome"]["russian_surface_posture"]
        )
        self.assertEqual(0, uncertain["outcome"]["surface_finding_count"])
        self.assertEqual(3, len(uncertain["outcome"]["source_aware_risk_ids"]))
        self.assertFalse(uncertain["outcome"]["human_review_created"])
        self.assertEqual(
            0, uncertain["gate_effects"]["accepted_translation_packets"]
        )

        prospective = copy.deepcopy(rejection)
        prospective["episode_id"] = (
            "tos.experimental-translation-episode.future-model.surface-rejection.v1"
        )
        prospective["method_freeze"]["model"]["model_id"] = (
            "local/future-model"
        )
        prospective["method_freeze"]["profile_id"] = (
            "tos-future-experimental-translation-v1"
        )
        prospective["private_run"]["run_id"] = (
            "future-experimental-translation-v1-20260808"
        )
        prospective["provenance_event_ref"] = (
            "tos.event.translation.future-experimental-episode.2026-08-08"
        )
        self.assertEqual([], list(validator.iter_errors(prospective)))

        status_drift = copy.deepcopy(rejection)
        status_drift["status"] = "failed-and-retained"
        self.assertTrue(list(validator.iter_errors(status_drift)))

        candidate_on_failure = copy.deepcopy(failure)
        candidate_on_failure["private_run"]["candidate_artifact"] = (
            rejection["private_run"]["candidate_artifact"]
        )
        self.assertTrue(list(validator.iter_errors(candidate_on_failure)))

        tracked_private_path = copy.deepcopy(rejection)
        tracked_private_path["content_boundary"]["private_paths_tracked"] = True
        self.assertTrue(list(validator.iter_errors(tracked_private_path)))

        false_promotion = copy.deepcopy(rejection)
        false_promotion["gate_effects"]["accepted_translation_packets"] = 1
        self.assertTrue(list(validator.iter_errors(false_promotion)))

        missing_specialized_admission = copy.deepcopy(uncertain)
        del missing_specialized_admission["admission"][
            "specialized_mt_challenger_admission"
        ]
        self.assertTrue(list(validator.iter_errors(missing_specialized_admission)))

        missing_source_risk = copy.deepcopy(uncertain)
        missing_source_risk["outcome"]["source_aware_risk_ids"] = []
        self.assertTrue(list(validator.iter_errors(missing_source_risk)))

        false_uncertain_promotion = copy.deepcopy(uncertain)
        false_uncertain_promotion["outcome"]["disposition"] = (
            "advance-to-human-review"
        )
        self.assertTrue(list(validator.iter_errors(false_uncertain_promotion)))

        mixed_runtime_uncertainty = copy.deepcopy(rejection)
        mixed_runtime_uncertainty["outcome"] = uncertain["outcome"]
        self.assertTrue(list(validator.iter_errors(mixed_runtime_uncertainty)))

    def test_manual_gold_assurance_schedule_partitions_packet_without_promotion(
        self,
    ) -> None:
        assurance = json.loads(
            (GOLD_ROOT / "gold-assurance.v2.json").read_text(encoding="utf-8")
        )
        units = {unit["sample_id"]: unit for unit in assurance["units"]}
        self.assertEqual(
            [],
            foundation._human_work_schedule_issues(
                units, assurance["human_work_schedule"]
            ),
        )

        duplicate = copy.deepcopy(assurance)
        duplicate["human_work_schedule"]["unscheduled_unit_ids"].append(
            duplicate["human_work_schedule"]["selected_calibration_unit_ids"][0]
        )
        self.assertIn(
            "selected calibration and unscheduled units overlap",
            foundation._human_work_schedule_issues(
                units, duplicate["human_work_schedule"]
            ),
        )

    def test_manual_gold_assurance_allows_only_disclosed_delayed_solo_recheck(self) -> None:
        validator, _ = foundation._schema_validator(
            foundation.GOLD_ASSURANCE_SCHEMA,
            REPO_ROOT,
        )
        assurance = json.loads(
            (GOLD_ROOT / "gold-assurance.v2.json").read_text(encoding="utf-8")
        )
        unit = assurance["units"][0]
        unit["current_assurance"] = "solo_human_delayed_rechecked"
        unit["reference_use"] = "calibration_metrics_with_disclosure"
        unit["next_route"] = "independent_multi_human_review"
        unit["review_evidence"] = {
            "pass_1_receipt_ref": "local-review/pass-1.json",
            "pass_2_receipt_ref": "local-review/pass-2.json",
            "adjudication_receipt_ref": None,
            "content_sha256": "a" * 64,
            "observed_delay_hours": 24,
            "same_reviewer": True,
        }
        self.assertEqual([], list(validator.iter_errors(assurance)))
        self.assertIsNone(
            foundation._solo_recheck_delay_issue(
                unit,
                assurance["solo_recheck_policy"]["minimum_delay_hours"],
            )
        )

        below_floor = copy.deepcopy(unit)
        below_floor["review_evidence"]["observed_delay_hours"] = 23.9
        self.assertIn(
            "below the declared delay floor",
            foundation._solo_recheck_delay_issue(below_floor, 24),
        )

        wrong_identity = copy.deepcopy(assurance)
        wrong_identity["units"][0]["review_evidence"]["same_reviewer"] = False
        self.assertTrue(list(validator.iter_errors(wrong_identity)))

        false_multi_human = copy.deepcopy(assurance)
        false_multi_human["units"][0]["reference_use"] = "independent_multi_human_gold"
        self.assertTrue(list(validator.iter_errors(false_multi_human)))
        self.assertIn(
            "invalid for solo_human_delayed_rechecked",
            foundation._assurance_reference_use_issue(
                false_multi_human["units"][0]
            ),
        )

        overlapping_scope = copy.deepcopy(assurance["language_scopes"][0])
        overlapping_scope["blocked_claims"].append(
            overlapping_scope["allowed_claims"][0]
        )
        self.assertIn(
            "both allows and blocks",
            foundation._language_scope_overlap_issue(overlapping_scope),
        )

    def test_manual_gold_assurance_digest_binding_detects_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo_root = Path(temporary)
            owner_path = repo_root / "owner.json"
            owner_path.write_text('{"version":1}\n', encoding="utf-8")
            digest_bound_ref = {
                "ref": "owner.json",
                "sha256": hashlib.sha256(owner_path.read_bytes()).hexdigest(),
            }
            self.assertIsNone(
                foundation._digest_bound_ref_issue(
                    digest_bound_ref,
                    expected_path=owner_path,
                    repo_root=repo_root,
                    field="owner",
                )
            )
            owner_path.write_text('{"version":2}\n', encoding="utf-8")
            self.assertEqual(
                "owner digest drifted",
                foundation._digest_bound_ref_issue(
                    digest_bound_ref,
                    expected_path=owner_path,
                    repo_root=repo_root,
                    field="owner",
                ),
            )

    def test_semantic_and_llm_evaluation_plans_do_not_materialize_false_tasks(self) -> None:
        semantic_validator, _ = foundation._schema_validator(
            foundation.SOURCE_GATED_SEMANTIC_EVALUATION_PLAN_SCHEMA,
            REPO_ROOT,
        )
        llm_validator, _ = foundation._schema_validator(
            foundation.SOURCE_GATED_LLM_EVALUATION_PLAN_SCHEMA,
            REPO_ROOT,
        )
        for filename, expected_kind, validator in (
            (
                "semantic-samples.json",
                "semantic-annotation",
                semantic_validator,
            ),
            (
                "llm-tasks.json",
                "llm-assistance",
                llm_validator,
            ),
        ):
            plan = json.loads((GOLD_ROOT / filename).read_text(encoding="utf-8"))
            self.assertEqual([], list(validator.iter_errors(plan)))
            self.assertEqual(expected_kind, plan["plan_kind"])
            self.assertEqual("blocked-not-materialized", plan["status"])
            self.assertEqual([], plan["tasks"])
            self.assertEqual(0, plan["result"]["run_count"])
            self.assertIsNone(plan["result"]["winner"])
            self.assertFalse(plan["result"]["promotion_authorized"])
            if expected_kind == "llm-assistance":
                prepared = plan["prepared_task_contract"]
                self.assertEqual(
                    "sign-candidate-and-refusal",
                    prepared["selected_task_family"],
                )
                self.assertEqual(20, prepared["required_total_tasks"])
                self.assertEqual([], prepared["source_anchor_refs"])
                self.assertFalse(prepared["task_instances_materialized"])
                self.assertFalse(prepared["source_text_present"])
                self.assertFalse(prepared["human_work_scheduled"])
                self.assertTrue(
                    prepared["profile_refresh_required_on_plan_change"]
                )
                self.assertEqual(
                    "task-specific-accepted-anchor-set",
                    plan["source_evidence_gate"]["gate_kind"],
                )
                self.assertEqual(
                    "task-specific-unassisted-human-baseline",
                    plan["human_baseline_gate"]["gate_kind"],
                )
                self.assertFalse(
                    plan["human_baseline_gate"]["human_work_scheduled"]
                )
                historical = plan["historical_gate_snapshot"]
                self.assertEqual(
                    "superseded-universal-30-15-gate",
                    historical["snapshot_kind"],
                )
                self.assertFalse(historical["scheduling_authority"])
                self.assertFalse(historical["execution_authority"])
            else:
                self.assertEqual(
                    "task-specific-accepted-anchor-set",
                    plan["task_specific_source_gate"]["gate_kind"],
                )
                self.assertFalse(
                    plan["task_specific_source_gate"][
                        "universal_packet_completion_required"
                    ]
                )
                assurance = plan["assurance_policy"]
                self.assertFalse(assurance["routine_human_work_for_prepared_rows"])
                self.assertFalse(assurance["human_work_scheduled"])
                self.assertEqual(
                    "triggered-exception-not-routine",
                    assurance["second_human_review_posture"],
                )
                self.assertFalse(assurance["model_disagreement_is_human_perspective"])
                historical = plan["historical_gate_snapshot"]
                self.assertFalse(historical["scheduling_authority"])
                self.assertFalse(historical["execution_authority"])

            false_task = copy.deepcopy(plan)
            false_task_payload = {
                "task_id": "tos-task-synthetic",
                "task_family": plan["task_families"][0],
                "stratum": "random",
                "source_anchor_refs": ["tos.anchor.synthetic"],
                "accepted_source_sha256": "a" * 64,
                "local_content_ref": "local-content/synthetic.json",
                "local_content_sha256": "b" * 64,
                "eligible_for_variant_execution": True,
            }
            if expected_kind == "llm-assistance":
                false_task_payload.update(
                    {
                        "task_family": "sign-candidate-and-refusal",
                        "source_review_event_ref": "tos.event.synthetic-review",
                        "human_baseline_ref": None,
                        "human_baseline_sha256": None,
                    }
                )
            else:
                false_task_payload.update(
                    {
                        "epistemic_layer": "textual_observation",
                        "assurance_route": "source-visible-evidence",
                        "source_review_event_ref": "tos.event.synthetic-review",
                        "required_context_refs": ["tos.anchor.synthetic-context"],
                        "language_competence_evidence_refs": [],
                        "unassisted_human_baseline_ref": None,
                        "unassisted_human_baseline_sha256": None,
                    }
                )
            false_task["tasks"] = [false_task_payload]
            self.assertTrue(list(validator.iter_errors(false_task)))

            premature_ready = copy.deepcopy(plan)
            premature_ready["status"] = "ready"
            self.assertTrue(list(validator.iter_errors(premature_ready)))

            false_run = copy.deepcopy(plan)
            false_run["result"]["run_count"] = 1
            false_run["result"]["run_refs"] = ["synthetic-run"]
            self.assertTrue(list(validator.iter_errors(false_run)))

            if expected_kind == "llm-assistance":
                false_prepared = copy.deepcopy(plan)
                false_prepared["prepared_task_contract"][
                    "source_anchor_refs"
                ] = ["tos.anchor.synthetic"]
                self.assertTrue(list(validator.iter_errors(false_prepared)))

                scheduled_human = copy.deepcopy(plan)
                scheduled_human["prepared_task_contract"][
                    "human_work_scheduled"
                ] = True
                self.assertTrue(list(validator.iter_errors(scheduled_human)))

                scheduled_historical_debt = copy.deepcopy(plan)
                scheduled_historical_debt["historical_gate_snapshot"][
                    "scheduling_authority"
                ] = True
                self.assertTrue(
                    list(validator.iter_errors(scheduled_historical_debt))
                )
            else:
                scheduled_human = copy.deepcopy(plan)
                scheduled_human["assurance_policy"]["human_work_scheduled"] = True
                self.assertTrue(list(validator.iter_errors(scheduled_human)))

                scheduled_historical_debt = copy.deepcopy(plan)
                scheduled_historical_debt["historical_gate_snapshot"][
                    "scheduling_authority"
                ] = True
                self.assertTrue(
                    list(validator.iter_errors(scheduled_historical_debt))
                )

    def test_mysl_work_boundaries_are_text_free_contiguous_and_unreviewed(self) -> None:
        boundary_map = json.loads(
            (MYSL_WORK_BOUNDARY_ROOT / "work-boundary-map.json").read_text(
                encoding="utf-8"
            )
        )
        anchors = [
            json.loads(line)
            for line in (MYSL_WORK_BOUNDARY_ROOT / "anchors.jsonl")
            .read_text(encoding="utf-8")
            .splitlines()
            if line
        ]

        self.assertFalse(boundary_map["source_text_included"])
        self.assertEqual("unreviewed", boundary_map["review_status"])
        self.assertTrue(
            all(
                member["epistemic_status"] == "inferred"
                for member in boundary_map["members"]
            )
        )
        self.assertEqual(
            [
                (5, 237),
                (238, 406),
                (407, 524),
                (525, 555),
                (556, 630),
                (631, 692),
                (693, 769),
            ],
            [
                (member["start_page"], member["end_page"])
                for member in boundary_map["members"]
            ],
        )
        self.assertEqual(11, len(anchors))
        self.assertTrue(all(anchor["status"] == "proposed" for anchor in anchors))
        self.assertTrue(
            all(
                selector["type"] == "page_region"
                for anchor in anchors
                for selector in anchor["selectors"]
            )
        )
        self.assertIn(
            "semantic units, signs, concepts, or relations",
            boundary_map["does_not_establish"],
        )

    def test_current_foundation_validates_without_private_payloads(self) -> None:
        self.assertEqual(
            [],
            foundation.validate_foundation(REPO_ROOT, require_local_payloads=False),
        )

    def test_current_foundation_validates_with_local_payloads_when_available(self) -> None:
        issues = foundation.validate_foundation(REPO_ROOT, require_local_payloads=True)
        missing_local_content = [
            issue
            for issue in issues
            if issue[1].startswith("required local ") and issue[1].endswith(" is missing")
        ]
        other_issues = [issue for issue in issues if issue not in missing_local_content]

        self.assertEqual([], other_issues)
        if missing_local_content:
            self.skipTest(
                f"{len(missing_local_content)} private local content files are unavailable"
            )
        self.assertEqual([], issues)

    def test_frozen_pilot_has_declared_counts_and_no_false_human_gold(self) -> None:
        sample_plan = json.loads((GOLD_ROOT / "sample-plan.json").read_text(encoding="utf-8"))
        ocr_plan = json.loads(
            (GOLD_ROOT / "ocr-visual-samples.json").read_text(encoding="utf-8")
        )
        gold_status = json.loads((GOLD_ROOT / "gold-status.json").read_text(encoding="utf-8"))
        translation_plan = json.loads(
            (GOLD_ROOT / "translation-samples.json").read_text(encoding="utf-8")
        )
        translation_laboratory_plan = json.loads(
            (GOLD_ROOT / "translation-laboratory-plan.v1.json").read_text(encoding="utf-8")
        )
        translation_reference_register = json.loads(
            (GOLD_ROOT / "translation-reference-register.v1.json").read_text(
                encoding="utf-8"
            )
        )
        retrieval_plan = json.loads(
            (GOLD_ROOT / "retrieval-queries.json").read_text(encoding="utf-8")
        )
        visual_retrieval_plan = json.loads(
            (GOLD_ROOT / "visual-retrieval-plan.v1.json").read_text(
                encoding="utf-8"
            )
        )
        graph_plan = json.loads((GOLD_ROOT / "graph-queries.json").read_text(encoding="utf-8"))
        transfer_plan = json.loads(
            (GOLD_ROOT / "transfer-samples.json").read_text(encoding="utf-8")
        )
        graph_claims = [
            json.loads(line)
            for line in (GOLD_ROOT / "graph-claims.jsonl").read_text(encoding="utf-8").splitlines()
        ]

        self.assertTrue(sample_plan["frozen_before_variant_outputs"])
        self.assertEqual([12, 12, 12], [group["sample_count"] for group in sample_plan["source_groups"]])
        self.assertEqual(
            [5, 5, 5],
            [sum(sample["gold_candidate"] for sample in group["samples"]) for group in sample_plan["source_groups"]],
        )
        self.assertTrue(ocr_plan["frozen_before_variant_outputs"])
        self.assertEqual([12, 12, 12], [group["sample_count"] for group in ocr_plan["source_groups"]])
        self.assertEqual(
            [5, 5, 5],
            [sum(sample["gold_candidate"] for sample in group["samples"]) for group in ocr_plan["source_groups"]],
        )
        self.assertEqual(36, sum(len(group["samples"]) for group in ocr_plan["source_groups"]))
        scan_group = next(group for group in ocr_plan["source_groups"] if group["language"] == "de")
        self.assertEqual(
            "tos.item.friedrich-nietzsche.also-sprach-zarathustra.de-naumann-1893.internet-archive-image-container-pdf",
            scan_group["item_ref"],
        )
        self.assertEqual(
            "tos.file.sha256.61c947e5aff76a64d82600cc52dcb25ff1b5862530d3a99c96824da885c1e6cf",
            scan_group["file_ref"],
        )
        self.assertEqual(
            [1, 2, 44, 46, 48, 50, 54, 56, 60, 201, 381, 523],
            [sample["page"] for sample in scan_group["samples"]],
        )
        self.assertEqual(
            1,
            sum(
                sample["projection_change"] == "replacement_for_nonvisual_unit"
                for group in ocr_plan["source_groups"]
                for sample in group["samples"]
            ),
        )
        self.assertEqual(
            {
                "model_draft": "not_started",
                "human_pass_1": "not_started",
                "human_pass_2": "not_started",
                "human_gold_materialized": False,
                "human_gold_ref": None,
                "formal_quality_metrics_status": "blocked_until_human_gold",
                "human_correction_time_seconds": None,
            },
            {
                key: ocr_plan["gold_gate"][key]
                for key in (
                    "model_draft",
                    "human_pass_1",
                    "human_pass_2",
                    "human_gold_materialized",
                    "human_gold_ref",
                    "formal_quality_metrics_status",
                    "human_correction_time_seconds",
                )
            },
        )
        self.assertEqual(15, len(gold_status["units"]))
        self.assertTrue(all(unit["gold_status"] == "candidate" for unit in gold_status["units"]))
        self.assertTrue(
            all(
                unit[pass_name]["status"] == "not_started"
                for unit in gold_status["units"]
                for pass_name in ("human_pass_1", "human_pass_2")
            )
        )
        self.assertTrue(translation_plan["frozen_before_drafts"])
        self.assertEqual(30, len(translation_plan["fragments"]))
        self.assertEqual("sealed", translation_plan["lanes"]["recognized_comparator"])
        self.assertTrue(
            all(fragment["human_source_acceptance"] is False for fragment in translation_plan["fragments"])
        )
        self.assertEqual(
            0,
            translation_laboratory_plan["source_review_gate"]["current_human_accepted_units"],
        )
        self.assertEqual(
            "frozen-blocked-on-human-source-acceptance",
            translation_laboratory_plan["status"],
        )
        self.assertEqual(
            {
                "human_only": "blocked-on-source-acceptance",
                "ai_only": "blocked-on-source-acceptance",
                "ai_alternatives": "blocked-on-source-acceptance",
                "ai_human": "blocked-on-source-acceptance",
            },
            {
                key: value["state"]
                for key, value in translation_laboratory_plan["blind_lanes"].items()
            },
        )
        self.assertEqual(
            "sealed",
            translation_laboratory_plan["recognized_comparator"]["visibility"],
        )
        self.assertEqual(17, len(translation_laboratory_plan["workflow_order"]))
        self.assertEqual(16, len(translation_reference_register["entries"]))
        self.assertEqual(
            len(translation_reference_register["entries"]),
            translation_reference_register["coverage"]["entry_count"],
        )
        self.assertEqual(
            set(translation_reference_register["required_categories"]),
            {
                entry["category"]
                for entry in translation_reference_register["entries"]
            },
        )
        self.assertEqual(0, translation_reference_register["coverage"]["content_admitted_entries"])
        self.assertEqual(0, translation_reference_register["coverage"]["human_bibliographic_reviews"])
        self.assertEqual(0, translation_reference_register["coverage"]["human_rights_reviews"])
        self.assertTrue(
            all(
                entry["access"]["content_ingested_for_translation_lab"] is False
                and entry["admission"]["accepted_as_truth"] is False
                for entry in translation_reference_register["entries"]
            )
        )
        comparator_entry = next(
            entry
            for entry in translation_reference_register["entries"]
            if entry["reference_id"]
            == "tos-ref.ru.antonovsky-cultural-revolution-2007"
        )
        self.assertIn(
            translation_laboratory_plan["recognized_comparator"]["expression_ref"],
            comparator_entry["tos_refs"]["record_refs"],
        )
        self.assertIn(
            translation_laboratory_plan["recognized_comparator"]["item_ref"],
            comparator_entry["tos_refs"]["record_refs"],
        )
        stanford_entry = next(
            entry
            for entry in translation_reference_register["entries"]
            if entry["reference_id"]
            == "tos-ref.en.loeb-tinsley-stanford-2026-forthcoming"
        )
        self.assertEqual("2026-08-11", stanford_entry["dating"]["verified_at"])
        self.assertEqual("metadata-only", stanford_entry["access"]["access_state"])
        self.assertEqual(
            "not-acquired",
            stanford_entry["access"]["acquisition_state"],
        )
        self.assertFalse(stanford_entry["admission"]["accepted_as_truth"])
        self.assertIn(
            "no completed publisher-authenticated release has been established after the 2026-08-11 Stanford date began",
            stanford_entry["admission"]["blocking_reasons"],
        )
        self.assertTrue(retrieval_plan["frozen_before_variant_outputs"])
        self.assertEqual(20, len(retrieval_plan["queries"]))
        self.assertEqual("not_started", retrieval_plan["human_judgment_status"])
        self.assertTrue(
            all(query["relevance_status"] == "model-proposed-awaiting-human" for query in retrieval_plan["queries"])
        )
        self.assertTrue(
            visual_retrieval_plan["frozen_before_challenger_outputs"]
        )
        self.assertEqual(
            "frozen-awaiting-runtime-admission",
            visual_retrieval_plan["status"],
        )
        self.assertEqual(
            hashlib.sha256(
                (GOLD_ROOT / "retrieval-queries.json").read_bytes()
            ).hexdigest(),
            visual_retrieval_plan["query_projection"][
                "source_query_plan_sha256"
            ],
        )
        self.assertEqual(
            20,
            visual_retrieval_plan["query_projection"]["resolved_query_count"],
        )
        self.assertEqual(
            36,
            visual_retrieval_plan["page_image_corpus"]["render_count"],
        )
        self.assertEqual(
            ["A", "B"],
            [
                control["label"]
                for control in visual_retrieval_plan["fixed_controls"]
            ],
        )
        self.assertTrue(
            all(
                control["reuse_posture"]
                == "immutable-completed-control-no-rerun"
                for control in visual_retrieval_plan["fixed_controls"]
            )
        )
        self.assertEqual(
            "Qwen/Qwen3-VL-Embedding-2B",
            visual_retrieval_plan["challenger"]["model_id"],
        )
        self.assertEqual(
            "9f2f7e710d6d81056aa5c0a4f04764fec6bb7bda",
            visual_retrieval_plan["challenger"]["source_revision"],
        )
        self.assertFalse(
            visual_retrieval_plan["human_assurance"][
                "routine_review_scheduled"
            ]
        )
        self.assertEqual(0, visual_retrieval_plan["result"]["run_count"])
        self.assertIsNone(visual_retrieval_plan["result"]["winner"])
        self.assertFalse(
            visual_retrieval_plan["rights_posture"][
                "public_page_images_authorized"
            ]
        )
        self.assertEqual(
            [],
            foundation._visual_retrieval_plan_issues(
                visual_retrieval_plan,
                source_query_plan=retrieval_plan,
                source_sample_plan=sample_plan,
                visual_sample_plan=ocr_plan,
            ),
        )
        self.assertTrue(graph_plan["frozen_before_variant_outputs"])
        self.assertEqual(10, len(graph_plan["queries"]))
        self.assertEqual(13, len(graph_claims))
        self.assertEqual("not_started", graph_plan["human_judgment_status"])
        self.assertTrue(all(claim["review_status"] == "unreviewed" for claim in graph_claims))
        self.assertEqual("blocked-not-run", transfer_plan["status"])
        self.assertEqual(0, transfer_plan["result"]["run_count"])
        self.assertEqual(
            {"tos-sample-mysl-p238", "tos-sample-mysl-p407", "tos-sample-mysl-p631"},
            {unit["sample_id"] for unit in transfer_plan["scouting_units"]},
        )
        self.assertTrue(
            all(
                unit["eligible_for_semantic_transfer"] is False
                for unit in transfer_plan["scouting_units"]
            )
        )
        self.assertEqual(
            hashlib.sha256((GOLD_ROOT / "graph-claims.jsonl").read_bytes()).hexdigest(),
            graph_plan["claim_set_sha256"],
        )

    def test_visual_retrieval_crosswalk_fails_closed_on_unresolved_query(self) -> None:
        visual_retrieval_plan = json.loads(
            (GOLD_ROOT / "visual-retrieval-plan.v1.json").read_text(
                encoding="utf-8"
            )
        )
        retrieval_plan = json.loads(
            (GOLD_ROOT / "retrieval-queries.json").read_text(encoding="utf-8")
        )
        sample_plan = json.loads(
            (GOLD_ROOT / "sample-plan.json").read_text(encoding="utf-8")
        )
        ocr_plan = json.loads(
            (GOLD_ROOT / "ocr-visual-samples.json").read_text(encoding="utf-8")
        )
        expected_anchor = retrieval_plan["queries"][0][
            "expected_source_anchor_refs"
        ][0]
        source_sample_id = next(
            sample["sample_id"]
            for group in sample_plan["source_groups"]
            for sample in group["samples"]
            if sample["anchor_ref"] == expected_anchor
        )
        broken_ocr_plan = copy.deepcopy(ocr_plan)
        visual_sample = next(
            sample
            for group in broken_ocr_plan["source_groups"]
            for sample in group["samples"]
            if sample["source_sample_id"] == source_sample_id
        )
        visual_sample["source_sample_id"] = "tos-sample-broken-crosswalk"

        issues = foundation._visual_retrieval_plan_issues(
            visual_retrieval_plan,
            source_query_plan=retrieval_plan,
            source_sample_plan=sample_plan,
            visual_sample_plan=broken_ocr_plan,
        )

        self.assertIn(
            "visual retrieval resolved query count drifted",
            issues,
        )
        self.assertIn(
            "visual retrieval unresolved query IDs drifted",
            issues,
        )
        self.assertIn(
            "visual retrieval one-to-one query crosswalk drifted",
            issues,
        )

    def test_frozen_visual_retrieval_plan_rejects_false_outputs(self) -> None:
        plan = json.loads(
            (GOLD_ROOT / "visual-retrieval-plan.v1.json").read_text(
                encoding="utf-8"
            )
        )
        validator, _ = foundation._schema_validator(
            foundation.VISUAL_RETRIEVAL_PLAN_SCHEMA,
            REPO_ROOT,
        )

        false_run = copy.deepcopy(plan)
        false_run["result"]["run_count"] = 1
        false_run["result"]["run_refs"] = ["synthetic-run"]
        self.assertTrue(list(validator.iter_errors(false_run)))

        scheduled_human = copy.deepcopy(plan)
        scheduled_human["human_assurance"]["routine_review_scheduled"] = True
        self.assertTrue(list(validator.iter_errors(scheduled_human)))

        moving_model = copy.deepcopy(plan)
        moving_model["challenger"]["source_revision"] = "0" * 40
        self.assertTrue(list(validator.iter_errors(moving_model)))

    def test_restricted_gold_content_is_ignored_but_route_card_is_trackable(self) -> None:
        route_card = GOLD_ROOT / "local-content/README.md"
        restricted_example = GOLD_ROOT / "local-content/gold/example.json"
        retrieval_content = GOLD_ROOT / "local-content/retrieval/queries.v1.json"
        self.assertFalse(foundation._git_ignored(REPO_ROOT, route_card))
        self.assertTrue(foundation._git_ignored(REPO_ROOT, restricted_example))
        self.assertTrue(foundation._git_ignored(REPO_ROOT, retrieval_content))

    def test_missing_local_payload_is_optional_only_when_not_required(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo_root = Path(temporary)
            item_directory = repo_root / "item"
            item_directory.mkdir()
            entry = {
                "relative_path": "payload/source.txt",
                "byte_size": 3,
                "sha256": hashlib.sha256(b"abc").hexdigest(),
            }

            self.assertEqual(
                [],
                foundation.validate_payload_file(
                    repo_root,
                    item_directory,
                    entry,
                    require_local_payloads=False,
                ),
            )
            issues = foundation.validate_payload_file(
                repo_root,
                item_directory,
                entry,
                require_local_payloads=True,
            )
            self.assertTrue(any("required local payload is missing" in message for _, message in issues))

    def test_present_payload_must_match_size_and_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo_root = Path(temporary)
            item_directory = repo_root / "item"
            payload_directory = item_directory / "payload"
            payload_directory.mkdir(parents=True)
            (payload_directory / "source.txt").write_bytes(b"different")
            entry = {
                "relative_path": "payload/source.txt",
                "byte_size": 3,
                "sha256": hashlib.sha256(b"abc").hexdigest(),
            }

            issues = foundation.validate_payload_file(
                repo_root,
                item_directory,
                entry,
                require_local_payloads=True,
            )
            messages = [message for _, message in issues]
            self.assertTrue(any("byte size" in message for message in messages))
            self.assertTrue(any("sha256" in message for message in messages))


if __name__ == "__main__":
    unittest.main()
