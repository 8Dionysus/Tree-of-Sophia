"""Standalone read-only access plane for Tree of Sophia."""

__all__ = ["ToSAccessCore", "NativeCore", "NativeAccessCore"]
__version__ = "0.1.0"


def __getattr__(name):
    # Importing a compiler or codec must not initialize the serving adapters.
    if name == 'ToSAccessCore':
        from .core import ToSAccessCore
        return ToSAccessCore
    if name == 'NativeCore':
        from .native_core import NativeCore
        return NativeCore
    if name == 'NativeAccessCore':
        from .native_access_core import NativeAccessCore
        return NativeAccessCore
    raise AttributeError(name)
