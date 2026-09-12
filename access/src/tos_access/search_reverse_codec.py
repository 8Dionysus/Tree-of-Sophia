"""Bounded, canonical, lossless per-document term sets (search storage v3).

Term IDs are dictionary addresses, not document/source ordering. This codec is
deliberately separate from the signed, at-most-256 forward posting codec.
"""
from array import array
import hashlib
import hmac

MAX_TERM_ID = 2**63 - 1
MAX_TERMS = 200_000
MAX_PAYLOAD_BYTES = 9 * MAX_TERMS
_DOMAIN = b"tos-search-reverse-uvarint-v1\0"


class ReverseCodecError(ValueError):
    """Invalid, over-limit, noncanonical or damaged reverse frame."""


def _identity(doc_id, kind):
    if type(doc_id) is not int or not 1 <= doc_id <= 2**53 - 1 or kind not in ("node", "relation"):
        raise ReverseCodecError("invalid reverse document identity")


def reverse_digest(doc_id, kind, count, payload):
    _identity(doc_id, kind)
    if type(count) is not int or not 1 <= count <= MAX_TERMS:
        raise ReverseCodecError("invalid reverse term count")
    return hashlib.sha256(_DOMAIN + doc_id.to_bytes(8, "big") + kind.encode("ascii")
                          + b"\0" + count.to_bytes(4, "big") + payload).digest()


def encode_reverse(doc_id, kind, terms):
    """Consume already sorted, strictly unique IDs; return count, bytes, seal."""
    _identity(doc_id, kind)
    payload = bytearray()
    previous = count = 0
    for term in terms:
        count += 1
        if count > MAX_TERMS or type(term) is not int or not previous < term <= MAX_TERM_ID:
            raise ReverseCodecError("invalid or oversized reverse term sequence")
        value = term - previous
        previous = term
        while value >= 128:
            payload.append((value & 127) | 128)
            value >>= 7
        payload.append(value)
    payload = bytes(payload)
    return count, payload, reverse_digest(doc_id, kind, count, payload)


def decode_reverse(doc_id, kind, count, payload, digest):
    """Validate the complete frame before returning bounded numeric addresses."""
    if (type(count) is not int or not 1 <= count <= MAX_TERMS
            or not isinstance(payload, bytes) or not count <= len(payload) <= 9 * count
            or not isinstance(digest, bytes) or len(digest) != 32):
        raise ReverseCodecError("invalid reverse frame bounds")
    if not hmac.compare_digest(digest, reverse_digest(doc_id, kind, count, payload)):
        raise ReverseCodecError("reverse frame digest mismatch")
    terms = array("Q")
    value = shift = previous = 0
    for byte in payload:
        value |= (byte & 127) << shift
        if byte & 128:
            shift += 7
            if shift > 56:
                raise ReverseCodecError("reverse varint overflow")
            continue
        if not value or (shift and byte == 0) or previous + value > MAX_TERM_ID:
            raise ReverseCodecError("noncanonical or overflowing reverse varint")
        previous += value
        terms.append(previous)
        if len(terms) > count:
            raise ReverseCodecError("reverse frame has excess terms")
        value = shift = 0
    if shift or len(terms) != count:
        raise ReverseCodecError("truncated reverse frame or count mismatch")
    return terms
