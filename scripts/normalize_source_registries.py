"""Retain exact research originals and build an immutable normalized snapshot."""
import argparse
from pathlib import Path
from source_registry_common import ROOT, PACKET, run

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input-root', type=Path)
    parser.add_argument('--packet-root', type=Path, default=ROOT / PACKET)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    result = run(args.packet_root, args.input_root, args.check)
    print(result['snapshot_id'], result['counts'])

if __name__ == '__main__':
    main()
