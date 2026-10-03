"""Maintained reference module, with explicit installed native association."""
import sys
from pathlib import Path


def main(arguments=None):
    args = list(sys.argv[1:] if arguments is None else arguments)
    if args and (args[0] == '--native-prefix' or args[0].startswith('--native-prefix=')):
        from .native_dispatch import run
        try:
            if args[0] == '--native-prefix':
                if len(args) < 2:
                    raise ValueError('--native-prefix requires an absolute software prefix')
                prefix, args = args[1], args[2:]
            else:
                prefix, args = args[0].split('=', 1)[1], args[1:]
            run(Path(prefix), args)
        except (OSError, ValueError, KeyError, TypeError) as error:
            raise SystemExit(f'invalid_native_installation: {error}') from error
    else:
        from .cli import main as reference_main
        return reference_main(args)


def native_main(arguments=None):
    """Wheel console caller for explicitly associated installed native code."""
    args = list(sys.argv[1:] if arguments is None else arguments)
    if not args or not (args[0] == '--native-prefix' or args[0].startswith('--native-prefix=')):
        raise SystemExit('tos-native requires --native-prefix /absolute/installed-prefix before operation arguments')
    return main(args)


if __name__ == '__main__':
    main()
