pub(super) const GOLD_ROOT: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1";
pub(super) const TARGET_CANDIDATE_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/transfer-target-passage-candidates.v1.json";
pub(super) const OUTPUT_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/transfer-source-passage-candidates.v1.json";
pub(super) const ANCHOR_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/transfer-source-passage-anchors.v1.jsonl";
pub(super) const PROVENANCE_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/transfer-provenance.jsonl";
pub(super) const LOCAL_CONTENT_ROOT: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/local-content/transfer-source-passages/v1";
pub(super) const SCHEMA_PATH: &str =
    "ToS/contracts/transfer-source-passage-candidate-set.schema.json";
pub(super) const SOURCE_ANCHOR_SCHEMA_PATH: &str = "ToS/contracts/source-anchor.schema.json";
pub(super) const BOUNDARY_FORENSIC_RETURN: &str = "ToS/research-packets/foundation-laboratory-2026-07/TRANSFER_SOURCE_BOUNDARY_FORENSIC_RETURN.md";
pub(super) const JENSEITS_ITEM_DIR: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf";
pub(super) const JENSEITS_MAP: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/structure/numbered-unit-page-map.json";
pub(super) const GENE_ITEM_DIR: &str = "ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/de-naumann-1892-second/editions/leipzig-c-g-naumann-1892-second-edition/items/wikimedia-commons-unc-scan-pdf";
pub(super) const GENE_MAP: &str = "ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/de-naumann-1892-second/editions/leipzig-c-g-naumann-1892-second-edition/items/wikimedia-commons-unc-scan-pdf/structure/hierarchical-numbered-unit-page-map.json";
pub(super) const ANTI_ADDRESS_ITEM_DIR: &str = "ToS/source-witnesses/collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/wikimedia-commons-stanford-scan-djvu";
pub(super) const ANTI_NAV_ITEM_DIR: &str = "ToS/source-witnesses/collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/internet-archive-google-stanford-djvu-xml";
pub(super) const ANTI_MAP: &str = "ToS/source-witnesses/collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/wikimedia-commons-stanford-scan-djvu/structure/hierarchical-numbered-unit-page-map.json";
pub(super) const PDFTOTEXT_VERSION: &str = "26.01.0";
pub(super) const EVENT_ID: &str =
    "tos.event.segmentation.golden-kernel-transfer-source-passages-v1.2026-08-08";
pub(super) const SET_ID: &str =
    "tos.transfer-candidate-set.golden-kernel-transfer-source-passages-v1";
pub(super) const SCHEMA_REF: &str =
    "https://tree-of-sophia.local/ToS/contracts/transfer-source-passage-candidate-set.schema.json";
pub(super) const PDF_VISIBLE_MARKER_RETURNS: &str = r#"[{"key":["jenseits",52,"32"],"image_object_number":252,"image_object_generation":0,"image_width_pixels":2500,"image_height_pixels":3900,"pixel_bbox":[1168,2485,77,47],"record_order":39,"line_bbox":[54.96,311.874983,249.60096,317.82565]},{"key":["genealogie",40,"11"],"image_object_number":194,"image_object_generation":0,"image_width_pixels":2636,"image_height_pixels":4283,"pixel_bbox":[1289,2469,72,32],"record_order":58,"line_bbox":[241.40662,375.444496,322.070076,383.544]}]"#;
pub(super) const ANTI_JP2_MARKER_RETURNS: &str = r#"[{"key":[240,"8"],"leaf_number":239,"member_path":"nietzscheswerke00nietgoog_jp2/nietzscheswerke00nietgoog_0239.jp2","pixel_bbox":[1839,2011,56,57],"record_order":8,"line_bbox":[916.0,2163.0,2979.0,2251.0]},{"key":[241,"9"],"leaf_number":240,"member_path":"nietzscheswerke00nietgoog_jp2/nietzscheswerke00nietgoog_0240.jp2","pixel_bbox":[1979,2253,61,62],"record_order":10,"line_bbox":[1071.0,2393.0,3129.0,2478.0]},{"key":[290,"44"],"leaf_number":289,"member_path":"nietzscheswerke00nietgoog_jp2/nietzscheswerke00nietgoog_0289.jp2","pixel_bbox":[1834,3399,97,52],"record_order":20,"line_bbox":[949.0,3532.0,2995.0,3620.0]}]"#;
pub(super) const INPUTS: &[(&str, &str)] = &[
    (
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/transfer-target-passage-candidates.v1.json",
        "tracked-target-passage-candidate-frame",
    ),
    (
        "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/structure/numbered-unit-page-map.json",
        "tracked-Jenseits-source-numbered-unit-map",
    ),
    (
        "ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/de-naumann-1892-second/editions/leipzig-c-g-naumann-1892-second-edition/items/wikimedia-commons-unc-scan-pdf/structure/hierarchical-numbered-unit-page-map.json",
        "tracked-Genealogie-source-numbered-unit-map",
    ),
    (
        "ToS/source-witnesses/collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/wikimedia-commons-stanford-scan-djvu/structure/hierarchical-numbered-unit-page-map.json",
        "tracked-Antichrist-source-numbered-unit-map",
    ),
    (
        "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/item.manifest.json",
        "Jenseits-source-item-manifest",
    ),
    (
        "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/resource-inventory.json",
        "Jenseits-source-inventory",
    ),
    (
        "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/rights.json",
        "Jenseits-source-rights",
    ),
    (
        "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/forensic-report.md",
        "Jenseits-baseline-forensic-report",
    ),
    (
        "ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/de-naumann-1892-second/editions/leipzig-c-g-naumann-1892-second-edition/items/wikimedia-commons-unc-scan-pdf/item.manifest.json",
        "Genealogie-source-item-manifest",
    ),
    (
        "ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/de-naumann-1892-second/editions/leipzig-c-g-naumann-1892-second-edition/items/wikimedia-commons-unc-scan-pdf/resource-inventory.json",
        "Genealogie-source-inventory",
    ),
    (
        "ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/de-naumann-1892-second/editions/leipzig-c-g-naumann-1892-second-edition/items/wikimedia-commons-unc-scan-pdf/rights.json",
        "Genealogie-source-rights",
    ),
    (
        "ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/de-naumann-1892-second/editions/leipzig-c-g-naumann-1892-second-edition/items/wikimedia-commons-unc-scan-pdf/forensic-report.md",
        "Genealogie-baseline-forensic-report",
    ),
    (
        "ToS/research-packets/foundation-laboratory-2026-07/TRANSFER_SOURCE_BOUNDARY_FORENSIC_RETURN.md",
        "bounded-model-visible-PDF-marker-return",
    ),
    (
        "ToS/source-witnesses/collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/wikimedia-commons-stanford-scan-djvu/item.manifest.json",
        "Antichrist-address-item-manifest",
    ),
    (
        "ToS/source-witnesses/collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/wikimedia-commons-stanford-scan-djvu/resource-inventory.json",
        "Antichrist-address-inventory",
    ),
    (
        "ToS/source-witnesses/collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/wikimedia-commons-stanford-scan-djvu/rights.json",
        "Antichrist-address-rights",
    ),
    (
        "ToS/source-witnesses/collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/internet-archive-google-stanford-djvu-xml/item.manifest.json",
        "Antichrist-navigation-item-manifest",
    ),
    (
        "ToS/source-witnesses/collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/internet-archive-google-stanford-djvu-xml/resource-inventory.json",
        "Antichrist-navigation-inventory",
    ),
    (
        "ToS/source-witnesses/collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/internet-archive-google-stanford-djvu-xml/rights.json",
        "Antichrist-navigation-rights",
    ),
    (
        "ToS/source-witnesses/collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/internet-archive-google-stanford-djvu-xml/forensic-report.md",
        "Antichrist-JP2-marker-review",
    ),
    (
        "ToS/source-witnesses/collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/internet-archive-google-stanford-djvu-xml/source-metadata-snapshot.jp2-scandata.2026-08-08.json",
        "Antichrist-JP2-scandata-provider-snapshot",
    ),
    (
        "ToS/contracts/transfer-source-passage-candidate-set.schema.json",
        "source-passage-candidate-set-contract",
    ),
    (
        "ToS/contracts/source-anchor.schema.json",
        "source-anchor-contract",
    ),
];
