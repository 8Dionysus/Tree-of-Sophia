"""Pure semantic checks shared by discovery admission and acquisition preflight."""

from __future__ import annotations

from typing import Any


def material_discovery_semantic_issues(payload: object) -> list[str]:
    """Return the foundation route's record-local discovery issues.

    Callers still validate the material-discovery schema and any cross-record
    provenance bindings. This function closes the semantic rules that can be
    checked from one discovery record alone.
    """

    if not isinstance(payload, dict):
        return ["discovery record is not an object"]

    issues: list[str] = []
    raw_channels = payload.get("channels", [])
    channels = raw_channels if isinstance(raw_channels, list) else []
    if not isinstance(raw_channels, list):
        issues.append("discovery channels are not an array")
    channel_rows = [channel for channel in channels if isinstance(channel, dict)]
    channel_ids = [channel.get("channel_id") for channel in channel_rows]
    valid_channel_ids = [value for value in channel_ids if isinstance(value, str)]
    if len(valid_channel_ids) != len(set(valid_channel_ids)):
        issues.append("discovery channel IDs are not unique")
    sequences = [channel.get("sequence") for channel in channel_rows]
    valid_sequences = [
        value
        for value in sequences
        if isinstance(value, int) and not isinstance(value, bool)
    ]
    if len(valid_sequences) != len(set(valid_sequences)):
        issues.append("discovery channel sequence values are not unique")
    general_web_sequences = [
        channel.get("sequence")
        for channel in channel_rows
        if channel.get("channel_type") == "general-web-search"
        and isinstance(channel.get("sequence"), int)
        and not isinstance(channel.get("sequence"), bool)
    ]
    if general_web_sequences and valid_sequences and max(valid_sequences) != max(general_web_sequences):
        issues.append("general web search is not the final discovery channel")

    result_ids: set[str] = set()
    expected_selected: set[str] = set()
    expected_rejected: set[str] = set()
    for channel in channel_rows:
        raw_results = channel.get("results", [])
        results = raw_results if isinstance(raw_results, list) else []
        ranks = [
            result.get("rank")
            for result in results
            if isinstance(result, dict)
        ]
        if ranks != list(range(1, len(ranks) + 1)):
            issues.append(
                f"discovery result order for {channel.get('channel_id')} is not contiguous from rank 1"
            )
        for result in results:
            if not isinstance(result, dict):
                continue
            result_id = result.get("result_id")
            if not isinstance(result_id, str):
                continue
            if result_id in result_ids:
                issues.append(f"duplicate discovery result_id: {result_id}")
            result_ids.add(result_id)
            if result.get("decision") == "select":
                expected_selected.add(result_id)
            elif result.get("decision") == "reject":
                expected_rejected.add(result_id)

    selected_values = payload.get("selected_result_ids", [])
    rejected_values = payload.get("rejected_result_ids", [])
    selected = {
        value for value in selected_values if isinstance(value, str)
    } if isinstance(selected_values, list) else set()
    rejected = {
        value for value in rejected_values if isinstance(value, str)
    } if isinstance(rejected_values, list) else set()
    if selected != expected_selected:
        issues.append("selected_result_ids do not match results whose decision is select")
    if rejected != expected_rejected:
        issues.append("rejected_result_ids do not match results whose decision is reject")
    if selected & rejected:
        issues.append("discovery result is both selected and rejected")
    unresolved_results = (selected | rejected) - result_ids
    if unresolved_results:
        issues.append(
            f"discovery decision references unknown results: {sorted(unresolved_results)}"
        )

    raw_comparison = payload.get("channel_comparison", [])
    comparisons = raw_comparison if isinstance(raw_comparison, list) else []
    comparison_ids = {
        item.get("channel_id")
        for item in comparisons
        if isinstance(item, dict) and isinstance(item.get("channel_id"), str)
    }
    if comparison_ids != set(valid_channel_ids):
        issues.append("discovery channel comparison does not cover the exact channel set")
    return issues
