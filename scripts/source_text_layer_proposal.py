"""Pure, bounded XHTML extraction and unreviewed private TextLayer construction.

Only the supplied member bytes are parsed. There is no file, ZIP, network,
schema, clock, grant or runtime access here. The command owner resolves the
original Item/member, authenticates the maker, validates pinned schemas and
rights, and atomically retains the returned confidential package. Construction
does not prove that supplied text came from supplied source metadata.

The initial profile is intentionally narrow: exact XHTML, no CR or entity
references, one exact element selector, transparent inline markup and explicit
br-to-LF. Unsupported input is refused, never silently repaired or normalized.
"""
from __future__ import annotations

import copy
import hashlib
import json
import re
import xml.etree.ElementTree as ET


MAX_MEMBER_BYTES = 8_388_608
MAX_TEXT_BYTES = 8_388_608
MAX_RECORD_BYTES = 1_048_576
MAX_ELEMENTS = 65_536
MAX_DEPTH = 64
MAX_MARKUP_TOKEN_BYTES = 16_384
MAX_ATTRIBUTES = 128
XHTML = "http://www.w3.org/1999/xhtml"
XML_ID = "{http://www.w3.org/XML/1998/namespace}id"
INLINE_ELEMENTS = (
    "a", "abbr", "b", "bdi", "bdo", "cite", "code", "dfn", "em", "i",
    "kbd", "mark", "q", "s", "samp", "small", "span", "strong", "sub",
    "sup", "time", "u", "var",
)
ROOT_ELEMENTS = frozenset((*INLINE_ELEMENTS, "p", "h1", "h2", "h3", "h4",
                           "h5", "h6", "li", "pre", "blockquote"))
LAYER_SCHEMA = "https://tree-of-sophia.local/ToS/contracts/source-text-layer.schema.json"
ANCHOR_SCHEMA = "https://tree-of-sophia.local/ToS/contracts/source-anchor-v2.schema.json"
AUTHORITY_BOUNDARY = (
    "a source text layer is one immutable, source-returnable representation with explicit "
    "derivation, uncertainty, review, competence, rights, and use scope; mechanical validation, "
    "model output, normalization, or agreement with another layer does not make it accepted "
    "source text, translation evidence, linguistic truth, semantic evidence, graph truth, "
    "canon authority, or publication permission"
)


class TextLayerProposalError(ValueError):
    """A supported, exact proposal cannot be constructed from these inputs."""


def _fail(message):
    # Do not put source text, XML snippets, selectors, paths or hashes in errors.
    raise TextLayerProposalError(message) from None


def _keys(value, fields, label):
    if type(value) is not dict or set(value) != set(fields):
        _fail(f"{label} has an incompatible field set")


def record_bytes(value):
    """The package's exact JSON serialization; distinct from assessment digest."""
    if type(value) is not dict:
        _fail("source package metadata must be a JSON object")
    try:
        encoded = (json.dumps(value, ensure_ascii=False, sort_keys=True,
                              separators=(",", ":"), allow_nan=False) + "\n").encode("utf-8")
    except (TypeError, ValueError, UnicodeError, RecursionError):
        _fail("source package metadata is not bounded finite UTF-8 JSON")
    if len(encoded) > MAX_RECORD_BYTES:
        _fail("source package metadata exceeds its byte budget")
    return encoded


DEFAULT_POLICY = {
    "schema_version": "tos_xhtml_text_extraction_policy_v1",
    "method": "tos.xhtml.character-data.v1",
    "input_media_type": "application/xhtml+xml",
    "encoding": "UTF-8-strict",
    "xml_version": "1.0",
    "namespace": XHTML,
    "carriage_returns": "reject-input-profile-limitation",
    "entity_and_character_references": "reject-all-ampersands",
    "doctype_and_processing_instructions": "reject-except-xml-declaration",
    "comments": "omit-bounded-markup-preserve-surrounding-character-data",
    "cdata": "literal-character-data-under-member-and-text-budgets",
    "markup_token_max_bytes": MAX_MARKUP_TOKEN_BYTES,
    "attributes_per_start_tag_max": MAX_ATTRIBUTES,
    "selector_schemes": ["tos.xhtml.element-ordinal.v1", "tos.xhtml.element-id.v1"],
    "ordinal_basis": "one-based-document-order-exact-namespace-and-local-name",
    "selected_root_elements": sorted(ROOT_ELEMENTS),
    "inline_elements": list(INLINE_ELEMENTS),
    "line_break_element": "br-to-one-LF",
    "text_nodes": "concatenate-in-document-order-without-selected-root-tail",
    "whitespace": "preserve-no-trim-collapse-or-inserted-block-separators",
    "unicode_normalization": "none",
    "attributes": "XML-1.0-parsed-identifiers-only-not-rendered",
    "unknown_selected_markup": "reject",
    "quality_assessment": "not-performed",
    "uncertainty": "none-recorded-is-not-reviewed-absence",
}
_POLICY_BYTES = record_bytes(DEFAULT_POLICY)


def validate_extraction_profile(selector, policy):
    """Validate only a versioned pure rule, never source access or authority."""
    if record_bytes(policy) != _POLICY_BYTES:
        _fail("unsupported XHTML extraction policy")
    _keys(selector, {"type", "scheme", "value"}, "XHTML selector")
    scheme, value = selector["scheme"], selector["value"]
    if selector["type"] != "structural" or type(value) is not str or not 1 <= len(value) <= 256:
        _fail("unsupported bounded XHTML selector")
    if scheme == "tos.xhtml.element-ordinal.v1":
        match = re.fullmatch(r"([a-z][a-z0-9]{0,31}):([1-9][0-9]{0,4})", value)
        if (match is None or match[1] not in ROOT_ELEMENTS
                or int(match[2]) > MAX_ELEMENTS):
            _fail("unsupported XHTML element ordinal")
    elif scheme == "tos.xhtml.element-id.v1":
        if (value != value.strip() or any(ord(char) < 33 or char in "<>\"'&" for char in value)
                or any(0xD800 <= ord(char) <= 0xDFFF for char in value)):
            _fail("unsupported XHTML element identifier")
    else:
        _fail("unsupported XHTML selector scheme")


class _BoundedTree(ET.TreeBuilder):
    def __init__(self):
        super().__init__()
        self.depth = self.count = 0

    def start(self, tag, attrs):
        self.depth += 1
        self.count += 1
        if self.depth > MAX_DEPTH or self.count > MAX_ELEMENTS:
            _fail("XHTML structure exceeds the bounded extraction profile")
        return super().start(tag, attrs)

    def end(self, tag):
        result = super().end(tag)
        self.depth -= 1
        return result


def _guard_xml_tokens(raw):
    """Bound markup before XMLParser can allocate names or attribute maps.

    This byte scanner does not replace XML validation. ASCII quote/delimiter
    bytes cannot occur inside UTF-8 continuation bytes. A quoted ``>`` does
    not end a tag, and only unquoted ``=`` counts a start-tag attribute
    (including namespace declarations). Comments have the markup token cap;
    CDATA is literal text and retains the existing member/output byte caps.
    No token contents or attribute containers are copied by this preflight.
    """
    offset, length = 0, len(raw)
    while True:
        start = raw.find(b"<", offset)
        if start < 0:
            return
        if raw.startswith(b"<!--", start):
            end = raw.find(b"-->", start + 4, start + MAX_MARKUP_TOKEN_BYTES)
            if end < 0:
                _fail("XHTML comment is unterminated or exceeds its markup budget")
            offset = end + 3
            continue
        if raw.startswith(b"<![CDATA[", start):
            end = raw.find(b"]]>", start + 9)
            if end < 0:
                _fail("XHTML CDATA is unterminated")
            offset = end + 3
            continue
        if raw.startswith(b"<!", start):
            _fail("XHTML declaration is outside the supported markup profile")
        index, quote, attributes = start + 1, None, 0
        start_tag = index < length and raw[index] not in (ord("/"), ord("?"))
        while index < length:
            if index - start + 1 > MAX_MARKUP_TOKEN_BYTES:
                _fail("XHTML markup token exceeds its byte budget")
            byte = raw[index]
            if quote is not None:
                if byte == quote:
                    quote = None
            elif byte in (ord("'"), ord('"')):
                quote = byte
            elif byte == ord(">"):
                offset = index + 1
                break
            elif byte == ord("<"):
                _fail("XHTML markup token is malformed")
            elif start_tag and byte == ord("="):
                attributes += 1
                if attributes > MAX_ATTRIBUTES:
                    _fail("XHTML start tag exceeds its attribute budget")
            index += 1
        else:
            _fail("XHTML markup token is unterminated")


def extract_xhtml_text(member_bytes, *, selector, policy):
    """Extract one selected element's parsed character data, without I/O.

    Strict UTF-8 and rejection of raw CR avoid XML's implicit newline rewrite.
    All ampersands are refused, including predefined/numeric references. XML
    1.0 declarations (optional UTF-8 encoding) are allowed; arbitrary PIs and
    DTDs are not. A lexical guard caps tags/attributes before parser allocation;
    bounded comments are omitted and CDATA retains literal character data.
    """
    validate_extraction_profile(selector, policy)
    if type(member_bytes) is not bytes or not 0 < len(member_bytes) <= MAX_MEMBER_BYTES:
        _fail("XHTML member exceeds its bounded byte profile")
    _guard_xml_tokens(member_bytes)
    try:
        document = member_bytes.decode("utf-8", errors="strict")
    except UnicodeError:
        _fail("XHTML member is not strict UTF-8")
    if "\r" in document or "&" in document:
        _fail("XHTML input uses unsupported carriage returns or references")
    if "<!DOCTYPE" in document or "<!ENTITY" in document or re.search(r"<\?(?!xml\s)", document):
        _fail("XHTML declarations or processing instructions are outside the profile")
    declarations = re.findall(r"<\?xml\s.*?\?>", document, flags=re.DOTALL)
    if declarations:
        declaration = declarations[0]
        if (len(declarations) != 1 or not document.lstrip("\ufeff").startswith(declaration)
                or re.search(r"\bversion\s*=\s*(['\"])1\.0\1", declaration) is None):
            _fail("XHTML requires a supported XML 1.0 declaration")
        encoding = re.search(r"\bencoding\s*=\s*(['\"])([^'\"]+)\1", declaration)
        if encoding and encoding[2].casefold() != "utf-8":
            _fail("XHTML declaration does not identify strict UTF-8")
    try:
        parser = ET.XMLParser(target=_BoundedTree())
        for offset in range(0, len(member_bytes), 65_536):
            parser.feed(member_bytes[offset:offset + 65_536])
        root = parser.close()
    except (ET.ParseError, UnicodeError, RecursionError):
        _fail("XHTML member is not supported well-formed XML")
    if root.tag != "{" + XHTML + "}html":
        _fail("XHTML member lacks the exact XHTML document namespace")
    scheme, value = selector["scheme"], selector["value"]
    if scheme == "tos.xhtml.element-ordinal.v1":
        local, ordinal = value.split(":")
        matches = list(root.iter("{" + XHTML + "}" + local))
        selected = matches[int(ordinal) - 1] if len(matches) >= int(ordinal) else None
    else:
        matches = [element for element in root.iter()
                   if element.get("id") == value or element.get(XML_ID) == value]
        selected = matches[0] if len(matches) == 1 else None
    if selected is None:
        _fail("XHTML selector does not resolve exactly one supported element")
    prefix = "{" + XHTML + "}"
    if not selected.tag.startswith(prefix) or selected.tag[len(prefix):] not in ROOT_ELEMENTS:
        _fail("selected XHTML root is outside the transparent text profile")
    parts = []

    def visit(element, *, top=False):
        local = element.tag[len(prefix):] if element.tag.startswith(prefix) else None
        if not top and local not in {*INLINE_ELEMENTS, "br"}:
            _fail("selected XHTML contains unsupported markup")
        if local == "br":
            if len(element) or element.text:
                _fail("XHTML line break contains unexpected character data")
            parts.append("\n")
            return
        if element.text is not None:
            parts.append(element.text)
        for child in element:
            visit(child)
            if child.tail is not None:
                parts.append(child.tail)

    visit(selected, top=True)
    text = "".join(parts)
    if not text or len(text.encode("utf-8")) > MAX_TEXT_BYTES:
        _fail("extracted XHTML text is empty or exceeds the output budget")
    return text


def _digest(value):
    if type(value) is not str or re.fullmatch(r"[a-f0-9]{64}", value) is None:
        _fail("source fixity must be an exact SHA-256")


def _ref(value):
    if (type(value) is not str or not 1 <= len(value) <= 4096 or value.startswith(("/", "~"))
            or "\\" in value or ":" in value or any(ord(char) < 32 for char in value)
            or any(part in {"", ".", ".."} for part in value.split("/"))):
        _fail("source reference leaves its bounded local profile")


def _identity(value, kind, *, opaque=False):
    suffix = r"sid-[a-f0-9]{32}" if opaque else r"[a-z0-9]+(?:[.-][a-z0-9]+)*"
    if type(value) is not str or re.fullmatch(r"tos\." + re.escape(kind) + r"\." + suffix, value) is None:
        _fail("source identity has an incompatible kind or shape")


def build_text_layer_proposal(*, exact_text, source_scope, identities, refs,
                              member, selector, policy, maker, language,
                              rights_record_refs):
    """Construct an immutable proposal from caller-verified extraction inputs.

    The returned ``content`` is exact UTF-8 bytes. Serialize the three JSON
    documents with ``record_bytes`` so anchor/policy byte fixity stays exact.
    Output posture is fixed local-only/unreviewed; the caller must ensure it
    does not weaken inherited source restrictions. No rights are granted here.
    """
    validate_extraction_profile(selector, policy)
    _keys(source_scope, {"work_ref", "expression_ref", "edition_ref", "item_ref",
                         "file_ref", "file_sha256"}, "source scope")
    _keys(identities, {"layer_id", "anchor_id", "passage_id", "provenance_event_id"}, "delegated identities")
    _keys(refs, {"layer_ref", "anchor_ref", "content_ref", "policy_ref", "configuration_ref",
                 "configuration_sha256", "source_payload_ref"}, "source refs")
    _keys(member, {"member_path", "member_sha256"}, "XHTML member")
    _keys(maker, {"maker_type", "agent_ref", "method", "version"}, "extraction maker")
    record_bytes({"source_scope": source_scope, "identities": identities, "refs": refs,
                  "member": member, "maker": maker, "rights": rights_record_refs})
    for kind in ("work", "expression", "edition", "item", "file"):
        _identity(source_scope[kind + "_ref"], kind)
    _digest(source_scope["file_sha256"])
    for key, kind in (("layer_id", "text-layer"), ("anchor_id", "anchor"),
                      ("passage_id", "passage"), ("provenance_event_id", "event")):
        _identity(identities[key], kind, opaque=True)
    for key, value in refs.items():
        _digest(value) if key == "configuration_sha256" else _ref(value)
    if len(set(refs[key] for key in refs if key != "configuration_sha256")) != len(refs) - 1:
        _fail("source proposal refs must not alias another package artifact")
    _ref(member["member_path"])
    _digest(member["member_sha256"])
    if (maker["maker_type"] != "software"
            or any(type(maker[key]) is not str or not maker[key].strip()
                   for key in ("agent_ref", "method", "version"))):
        _fail("structural extraction requires an explicit software maker")
    if type(language) is not str or re.fullmatch(r"(?:und|[A-Za-z]{2,3}(?:-[A-Za-z0-9]{2,8})*)", language) is None:
        _fail("source representation language is outside its contract")
    if type(rights_record_refs) is not list or not 1 <= len(rights_record_refs) <= 64:
        _fail("private extraction requires bounded exact rights references")
    seen = set()
    for right in rights_record_refs:
        _keys(right, {"ref", "sha256"}, "rights reference")
        _ref(right["ref"])
        _digest(right["sha256"])
        if right["ref"] in seen:
            _fail("source rights references repeat an identity")
        seen.add(right["ref"])
    try:
        content = exact_text.encode("utf-8") if type(exact_text) is str else None
    except UnicodeError:
        _fail("extracted representation is not exact UTF-8")
    if not content or len(content) > MAX_TEXT_BYTES or "\r" in exact_text or "&" in exact_text:
        _fail("extracted representation is outside the supported text profile")
    # Defensive copies: a caller's later mutation cannot rewrite this result.
    owned_policy, owned_selector, owned_maker, rights = copy.deepcopy((policy, selector, maker, rights_record_refs))
    owned_maker.update(configuration_ref=refs["configuration_ref"], configuration_digest=refs["configuration_sha256"])
    anchor = {
        "$schema": ANCHOR_SCHEMA, "schema_version": "tos_source_anchor_v2",
        "anchor_id": identities["anchor_id"], "anchor_version": 1,
        "passage_id": identities["passage_id"],
        "target": {"item_id": source_scope["item_ref"], "file_id": source_scope["file_ref"],
                   "file_sha256": source_scope["file_sha256"], "media_type": "application/epub+zip"},
        "selector_payload": {"kind": "selector_expression", "expression": {
            "mode": "refinement_chain", "steps": [
                {"state": {"state_type": "digest_state", "representation_ref": refs["source_payload_ref"],
                           "representation_sha256": source_scope["file_sha256"], "media_type": "application/epub+zip"},
                 "selector": {"type": "container_member", **copy.deepcopy(member), "member_media_type": "application/xhtml+xml"}},
                {"state": {"state_type": "digest_state", "representation_ref": member["member_path"],
                           "representation_sha256": member["member_sha256"], "media_type": "application/xhtml+xml"},
                 "selector": owned_selector}]}},
        "publication_boundary": {"record_storage": "ignored_local", "source_content_visibility": "local_only",
                                 "source_text_in_record": False, "public_payload_expected": False},
        "selector_method": {key: value for key, value in owned_maker.items() if key != "agent_ref"},
        "resolution_status": "locator_only", "review_status": "unreviewed", "review_ref": None,
        "provenance_event_ref": identities["provenance_event_id"], "supersedes_anchor_ref": None,
    }
    content_digest = hashlib.sha256(content).hexdigest()
    layer = {
        "$schema": LAYER_SCHEMA, "schema_version": "tos_source_text_layer_v1",
        "layer_id": identities["layer_id"], "layer_version": 1, "supersedes_layer_ref": None,
        "layer_role": "machine_transcription",
        "source_binding": {**{key: source_scope[key] for key in ("work_ref", "expression_ref", "edition_ref", "item_ref")},
            "source_file_ref": source_scope["file_ref"], "source_file_sha256": source_scope["file_sha256"],
            "anchor_contract": "tos_source_anchor_v2", "anchors": [{"anchor_id": identities["anchor_id"],
                "anchor_record_ref": refs["anchor_ref"], "anchor_record_sha256": hashlib.sha256(record_bytes(anchor)).hexdigest()}]},
        "representation": {"content_file_id": "tos.file.sha256." + content_digest, "content_ref": refs["content_ref"],
            "content_sha256": content_digest, "media_type": "text/plain", "charset": "UTF-8", "language": language,
            "text_scope": {"start": 0, "end": len(exact_text), "position_unit": "unicode_code_point", "interval": "half_open"},
            "character_normalization": "none", "line_break_posture": "logical_reflow", "storage": "ignored_local",
            "content_visibility": "local_only", "tracked_content": False, "publication_authorized": False,
            "rights_record_refs": rights, "publication_authority_refs": []},
        "derivation": {"method": "structural_extraction", "input_layers": [], "maker": owned_maker,
            "preservation_goal": "source_near", "loss_posture": "preservation_intended", "silent_changes_allowed": False,
            "change_payload": {"kind": "none"}},
        "editorial_policy": {"policy_ref": refs["policy_ref"], "policy_sha256": hashlib.sha256(record_bytes(owned_policy)).hexdigest(),
            "transcription_goal": "machine_candidate", "historical_language_preserved": False,
            "printing_errors_silently_corrected": False, "typography_posture": "normalize_declared",
            "layout_posture": "logical_reflow", "unicode_normalization": "none",
            "uncertainty_representation": "explicit-never-silent", "method_declared": True},
        "uncertainty": {"status": "none", "annotations": []},
        "admission": {"mechanical_status": "materialized", "review_status": "unreviewed", "review_ref": None,
            "human_review_performed": False, "human_language_competence": "not_assessed", "language_competence_evidence_refs": [],
            "accepted_uses": [], "automatic_validation_complete": False, "model_output_is_ground_truth": False,
            "validator_proves_content_truth": False, "routine_human_task_created": False, "promotion_authorized": False},
        "provenance_event_ref": identities["provenance_event_id"], "authority_boundary": AUTHORITY_BOUNDARY,
    }
    record_bytes(layer)
    return {"layer": layer, "anchor": anchor, "policy": owned_policy, "content": content}
