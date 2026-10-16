"""Exceptions raised while selecting the external native Core consumer."""


class DataAccessUnavailable(RuntimeError):
    """The selected native software or data owner is unavailable."""


class AddressedUpdateError(RuntimeError):
    """The selected native owner refused an addressed snapshot update."""
