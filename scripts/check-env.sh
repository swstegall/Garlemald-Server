#!/usr/bin/env bash
# Verify that the local machine can build garlemald-server. Exits 0 on
# success, non-zero with a list of missing tools otherwise.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=_lib.sh
source "$SCRIPT_DIR/_lib.sh"

say "Checking build environment"
if check_env; then
    ok "environment ready"
    exit 0
else
    err "fix the issues above and re-run"
    exit 1
fi
