#!/bin/sh
# Run one guarded normal start (optionally wash-only) and retain a private daily log.
set -eu
umask 077
repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_dir"
mkdir -p .local/scheduled-start
exec >> ".local/scheduled-start/$(date +%F).log" 2>&1
printf '\n[%s] Scheduled laundry attempt: %s\n' "$(date -Iseconds)" "${1:-wash-and-dry}"
exec 9>.local/scheduled-start/lock
if ! flock -n 9; then
    printf 'Another scheduled attempt is running; skipped.\n'
    exit 0
fi
case "${1:-}" in
    '') set -- start ;;
    --dry-run) set -- start --dry-run ;;
    --wash-only) set -- start --wash-only ;;
    *) printf 'Usage: daily-start.sh [--dry-run|--wash-only]\n'; exit 2 ;;
esac
# Refresh credentials before each daily attempt; failure prevents the start.
target/release/laundry-automation refresh-token
if target/release/laundry-automation "$@"; then
    printf '[%s] Request completed successfully\n' "$(date -Iseconds)"
else
    result=$?
    printf '[%s] Request failed with exit code %s; no automatic retry\n' "$(date -Iseconds)" "$result"
    exit "$result"
fi
