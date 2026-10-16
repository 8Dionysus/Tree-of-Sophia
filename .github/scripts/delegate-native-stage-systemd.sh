#!/usr/bin/env bash
# Ordinary ephemeral Linux CI delegation. Native product verifies all OS custody.
set -euo pipefail
fail() { printf '%s\n' "native CI unsupported/rejected: $*" >&2; exit 125; }
if [[ ${1-} == --delegated ]]; then
  shift
  [[ $# -ge 6 ]] || fail 'internal arguments'
  native=$1; scratch=$2; persistent=$3; cutoff=$4; shutdown=$5; shift 5
  current() { node --input-type=module -e 'if(process.hrtime.bigint()>=BigInt(process.argv[1]))process.exit(125)' "$cutoff"; }
  current
  [[ $EUID -ne 0 && -f /sys/fs/cgroup/cgroup.controllers ]] || fail 'nonroot cgroup v2 required'
  membership=$(cat /proc/self/cgroup)
  [[ $membership == 0::/* && $membership != *$'\n'* ]] || fail 'single unified membership required'
  common=/sys/fs/cgroup${membership#0::}
  [[ -d $common && ! -L $common ]] || fail 'delegated unit cgroup missing'
  [[ $(cat "$common/memory.max") == 3221225472 && $(cat "$common/memory.swap.max") == 0 ]] || fail 'actual aggregate caps'
  current
  mkdir "$common/setup" "$common/consumer"
  # Move this actual bootstrap process before enabling child memory control.
  printf '%s\n' "$$" > "$common/setup/cgroup.procs"
  [[ -z $(cat "$common/cgroup.procs") ]] || fail 'common parent must have no processes'
  printf '%s\n' '+memory' > "$common/cgroup.subtree_control"
  printf '%s\n' 536870912 > "$common/setup/memory.max"
  printf '%s\n' 0 > "$common/setup/memory.swap.max"
  printf '%s\n' 2684354560 > "$common/consumer/memory.max"
  printf '%s\n' 0 > "$common/consumer/memory.swap.max"
  [[ -w $common/consumer/cgroup.kill ]] || fail 'consumer kill delegation required'
  current
  exec "$native" private-stage-run --unshare-exe /usr/bin/unshare \
    --consumer-cgroup "$common/consumer" --scratch-parent "$scratch" \
    --quota-bytes 536870912 --inodes 65536 --working-ram-bytes 2684354560 \
    --work-deadline-ns "$cutoff" --maximum-shutdown-ms "$shutdown" \
    --persistent-store "$persistent" -- "$@"
fi
[[ $# -ge 7 ]] || fail 'usage: script NATIVE SCRATCH PERSISTENT ORIGINAL_NS SHUTDOWN_MS -- CONSUMER ARGS'
native=$1; scratch=$2; persistent=$3; cutoff=$4; shutdown=$5; shift 5
[[ $1 == -- ]] || fail 'missing consumer separator'; shift
[[ $EUID -ne 0 && -n ${CI:-} && -f /sys/fs/cgroup/cgroup.controllers ]] || fail 'ephemeral nonroot CI cgroup-v2 environment required'
[[ $native == /* && $scratch == /* && $persistent == /* && $cutoff =~ ^[0-9]+$ && $shutdown =~ ^[0-9]+$ ]] || fail 'absolute paths and explicit original budgets required'
shutdown=$((10#$shutdown))
[[ $shutdown -gt 0 && $shutdown -le 5000 ]] || fail 'finite CI shutdown allowance exceeded'
[[ -x $native && -x /usr/bin/unshare && -x /usr/bin/systemd-run ]] || fail 'selected executables missing'
script=$(realpath -- "$0")
# CLOCK_MONOTONIC from Node is the same original caller clock. Do not reset it.
remaining=$(node --input-type=module -e 'const cutoff=BigInt(process.argv[1]);const now=process.hrtime.bigint();if(cutoff<=now)process.exit(125);console.log((cutoff-now)/1000000n)' "$cutoff")
[[ $remaining =~ ^[0-9]+$ && $remaining -gt $shutdown && $remaining -le 50000 ]] || fail 'original deadline exhausted'
work_ms=$((remaining-shutdown))
unit="tos-native-ci-${UID}-$(cat /proc/sys/kernel/random/uuid)"
# Unit identity is unique to this invocation; cleanup never signals a sampled PID.
cleanup() {
  result=$?
  trap - EXIT INT TERM
  # Successful --wait already reported service termination.
  if [[ $result == 0 ]]; then exit 0; fi
  # Separate finite outer fallback, not proof of the native original cutoff.
  if ! /usr/bin/timeout --signal=KILL "$((shutdown/1000)).$(printf '%03d' "$((shutdown%1000))")s" sudo -n /usr/bin/systemctl stop "$unit"; then
    printf '%s\n' "external containment required for transient unit $unit" >&2
    result=125
  fi
  exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
# systemd owns stop/kill/reaping; Bash never signals a sampled child PID.
# RuntimeMaxSec is a relative fallback from activation; it does NOT prove
# original absolute deadline containment or cancellation cleanup. SOURCE DRAFT.
sudo -n /usr/bin/systemd-run --quiet --wait --pipe --expand-environment=no --unit="$unit" \
  --property="User=$(id -un)" --property="Group=$(id -gn)" \
  --property=Type=exec --property="TimeoutStartSec=${work_ms}ms" --property=Delegate=yes --property=MemoryMax=3221225472 --property=MemorySwapMax=0 \
  --property="RuntimeMaxSec=${work_ms}ms" --property="TimeoutStopSec=${shutdown}ms" \
  --property=KillMode=control-group --property=SendSIGKILL=yes \
  --property="WorkingDirectory=$PWD" \
  /usr/bin/bash "$script" --delegated "$native" "$scratch" "$persistent" "$cutoff" "$shutdown" "$@" <&0 &
# Builtin wait is interruptible; cleanup addresses the exact unit, never this PID.
wait "$!"
