"""Error identities used by the native exact-source SDK adapters."""

class SourceReadError(ValueError):
    """The supplied ABI value is structurally invalid."""


class SourceReadBudgetExceeded(SourceReadError):
    """The selected exact record would exceed a delivery budget."""
