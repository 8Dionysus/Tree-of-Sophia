"""Standalone read-only access plane for Tree of Sophia."""

__all__ = ["ToSAccessCore"]
__version__ = "0.1.0"


def __getattr__(name):
    # Importing a compiler or codec must not initialize the serving adapters.
    if name == 'ToSAccessCore':
        from .core import ToSAccessCore
        return ToSAccessCore
    raise AttributeError(name)
