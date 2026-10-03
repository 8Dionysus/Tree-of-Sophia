"""Shared error identities for reference and native exact-source callers."""

class SourceReadError(ValueError):
    """The supplied ABI value is structurally invalid."""


class SourceReadBudgetExceeded(SourceReadError):
    """The selected exact record would exceed a delivery budget."""
