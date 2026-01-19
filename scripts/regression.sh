#!/usr/bin/env bash
set -euo pipefail

tmpfileA="/tmp/mptk_osemosys.mps"
tmpfileB="/tmp/mptk_osemosys_norm.mps"
cargo run comp examples/osemosys.mod examples/ose_atl.dat > $tmpfileA
uv run python scripts/normalize_mps.py $tmpfileA $tmpfileB
diff scratch/norm_ose_atl.grnd.mps $tmpfileB
