"""Lossless, bounded delivery codec. This module makes no admission judgment."""
from __future__ import annotations

import copy
import json
import math
import re
from typing import Any

ROLES = ('name', 'caption', 'hover', 'statement', 'grounds', 'history', 'technical')
WIRE_BUDGET = 16_384
EXPANDED_BUDGET = 524_288
PACKET_BUDGET = 65_536
MAX_DEPTH = 64
MAX_MEMBERS = 30_000
_FORBIDDEN = {'__proto__', 'prototype', 'constructor'}


def _exact_ref(value: Any) -> bool:
    return (type(value) is dict and set(value) == {'id', 'version', 'digest'}
            and type(value['id']) is str and bool(value['id'])
            and type(value['version']) is int and 1 <= value['version'] <= 9_007_199_254_740_991
            and type(value['digest']) is str and re.fullmatch(r'sha256:[a-f0-9]{64}', value['digest']) is not None)


def bounded_cost(value: Any, budget: int) -> int:
    """Conservative JSON byte cost, checked before any clone or recursion."""
    stack = [(value, 0)]
    cost = members = 0
    while stack:
        node, depth = stack.pop()
        members += 1
        if depth > MAX_DEPTH or members > MAX_MEMBERS:
            raise ValueError('human form JSON exceeds structural bounds')
        if isinstance(node, str):
            if len(node) > budget - cost:
                raise ValueError('human form JSON exceeds byte budget')
            cost += len(json.dumps(node, ensure_ascii=False).encode('utf-8', errors='backslashreplace'))
        elif node is None or type(node) is bool:
            cost += 5
        elif type(node) is int:
            if node.bit_length() > 1023:
                raise ValueError('human form integer exceeds portable JSON range')
            cost += max(32, len(str(node)))
        elif type(node) is float and math.isfinite(node):
            cost += max(32, len(str(node)))
        elif type(node) is list:
            if len(stack) + members + len(node) > MAX_MEMBERS:
                raise ValueError('human form JSON exceeds member budget')
            cost += 2 + len(node)
            stack.extend((child, depth + 1) for child in node)
        elif type(node) is dict:
            if len(stack) + members + 2 * len(node) > MAX_MEMBERS:
                raise ValueError('human form JSON exceeds member budget')
            if any(type(key) is not str or key in _FORBIDDEN for key in node):
                raise ValueError('human form contains an unsafe object key')
            cost += 2 + 2 * len(node)
            stack.extend((child, depth + 1) for pair in node.items() for child in pair)
        else:
            raise ValueError('human form contains a non-JSON value')
        if cost > budget or len(stack) + members > MAX_MEMBERS:
            raise ValueError('human form JSON exceeds byte or member budget')
    return cost


def _same(left: Any, right: Any) -> bool:
    return json.dumps(left, sort_keys=True, ensure_ascii=False, allow_nan=False) == json.dumps(
        right, sort_keys=True, ensure_ascii=False, allow_nan=False)


def _common(values: list[dict]) -> dict:
    result = {}
    if not values:
        return result
    for key, first in values[0].items():
        if not all(key in value for value in values[1:]):
            continue
        items = [value[key] for value in values]
        if all(_same(first, value) for value in items[1:]):
            result[key] = copy.deepcopy(first)
        elif all(type(value) is dict for value in items):
            nested = _common(items)
            if nested:
                result[key] = nested
    return result


def _subtract(value: dict, base: dict) -> dict:
    result = {}
    for key, member in value.items():
        if key not in base:
            result[key] = copy.deepcopy(member)
        elif not _same(member, base[key]):
            result[key] = _subtract(member, base[key])
    return result


def _merge(base: dict, delta: dict) -> dict:
    result = copy.deepcopy(base)
    for key, member in delta.items():
        if key not in base:
            result[key] = copy.deepcopy(member)
        elif type(base[key]) is dict and type(member) is dict and base[key] and member:
            result[key] = _merge(base[key], member)
        else:
            raise ValueError('human form delta overlaps a base leaf')
    return result


def _selection(value: Any, version: str, packet_key: str) -> dict:
    if type(value) is not dict or value.get('schema_version') != version:
        raise ValueError('invalid human form selection version')
    roles = value.get('roles')
    if type(roles) is not dict or set(roles) != set(ROLES):
        raise ValueError('invalid human form selection roles')
    for role in ROLES:
        selected = roles[role]
        if type(selected) is not dict or set(selected) != {'state', 'reason', 'form', packet_key}:
            raise ValueError('invalid human form role envelope')
        packet = selected[packet_key]
        if (selected['state'] == 'ready' and type(packet) is not dict) or (selected['state'] != 'ready' and packet is not None):
            raise ValueError('human form role state contradicts packet')
    return roles


def encode_human_form_selection(selection: dict, *, enforce_budget: bool = True) -> dict:
    """Encode a logical v1 selection; allocation may temporarily exceed wire cost."""
    bounded_cost(selection, EXPANDED_BUDGET)
    roles = _selection(selection, 'tos_human_form_selection_v1', 'packet')
    if 'packet_base' in selection or 'shared_limits' in selection:
        raise ValueError('reserved human form wire field in v1 selection')
    packets, limits = {}, []
    for role in ROLES:
        packet = roles[role]['packet']
        if packet is None:
            continue
        bounded_cost(packet, PACKET_BUDGET)
        if not _exact_ref(roles[role]['form']) or not _exact_ref(packet.get('form')) or not _same(roles[role]['form'], packet['form']):
            raise ValueError('human form role and packet exact refs differ')
        packet = copy.deepcopy(packet)
        del packet['form']
        admission = packet.get('admission')
        if type(admission) is dict:
            if 'limit_refs' in admission:
                raise ValueError('reserved admission.limit_refs in source packet')
            if 'limits' in admission:
                if type(admission['limits']) is not list or any(type(limit) is not str for limit in admission['limits']):
                    raise ValueError('invalid admission limits')
                refs = []
                for limit in admission.pop('limits'):
                    if limit not in limits:
                        limits.append(limit)
                    refs.append(limits.index(limit))
                admission['limit_refs'] = refs
        packets[role] = packet
    base = _common(list(packets.values()))
    if len(limits) > 512:
        raise ValueError('excessive shared human form limits')
    result = copy.deepcopy(selection)
    result.update(schema_version='tos_human_form_selection_v2', packet_base=base, shared_limits=limits)
    for role in ROLES:
        selected = result['roles'][role]
        del selected['packet']
        selected['packet_delta'] = _subtract(packets[role], base) if role in packets else None
    bounded_cost(result, WIRE_BUDGET if enforce_budget else EXPANDED_BUDGET)
    return result


def decode_human_form_selection(selection: dict) -> dict:
    """Bound wire first, reconstruct independent complete packets, then bound output."""
    bounded_cost(selection, WIRE_BUDGET)
    roles = _selection(selection, 'tos_human_form_selection_v2', 'packet_delta')
    base, limits = selection.get('packet_base'), selection.get('shared_limits')
    if type(base) is not dict or type(limits) is not list or len(limits) > 512 or any(type(limit) is not str for limit in limits):
        raise ValueError('invalid shared human form data')
    if 'form' in base:
        raise ValueError('inline form ref in shared human form base')
    if len(set(limits)) != len(limits):
        raise ValueError('duplicate shared human form limit')
    if not any(roles[role]['state'] == 'ready' for role in ROLES) and (base or limits):
        raise ValueError('unused shared human form data')
    result = copy.deepcopy(selection)
    result['schema_version'] = 'tos_human_form_selection_v1'
    del result['packet_base'], result['shared_limits']
    used = set()
    for role in ROLES:
        selected = result['roles'][role]
        delta = selected.pop('packet_delta')
        packet = None
        if delta is not None:
            if 'form' in delta or not _exact_ref(selected['form']):
                raise ValueError('invalid or shadowed shared human form ref')
            packet = _merge(base, delta)
            packet['form'] = copy.deepcopy(selected['form'])
            admission = packet.get('admission')
            if type(admission) is dict and 'limits' in admission:
                raise ValueError('inline limits in shared human form packet')
            if type(admission) is dict and 'limit_refs' in admission:
                refs = admission.pop('limit_refs')
                if type(refs) is not list or any(type(index) is not int or not 0 <= index < len(limits) for index in refs):
                    raise ValueError('invalid shared human form limit index')
                used.update(refs)
                admission['limits'] = [limits[index] for index in refs]
            bounded_cost(packet, PACKET_BUDGET)
        selected['packet'] = packet
    if used != set(range(len(limits))):
        raise ValueError('unused shared human form limit')
    bounded_cost(result, EXPANDED_BUDGET)
    return result
