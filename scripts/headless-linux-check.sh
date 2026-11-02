#!/bin/sh
# Proves the SSH-server path: the CLI alone on a headless Linux host with no GUI and no systemd.
# Builds the CLI in rust:1, then runs a full user journey in a clean ubuntu:24.04 as a non-root
# user, with a fake `claude` on PATH. Needs Docker. Run from the repo root:
#   scripts/headless-linux-check.sh
set -eu

repo=$(cd "$(dirname "$0")/.." && pwd)
out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT

echo "== build the CLI in rust:1"
# ponytail: debug build, faster; named volumes keep the target dir and registry between runs
docker run --rm -v "$repo":/src:ro -v rck-target:/t -v rck-cargo:/usr/local/cargo/registry \
  -v "$out":/out rust:1 sh -c 'cd /src && CARGO_TARGET_DIR=/t cargo build -q -p rc-cli && cp /t/debug/rc-keeper /out/'

echo "== run in ubuntu:24.04"
docker run --rm -i -v "$out":/dl:ro ubuntu:24.04 sh -s <<'IN'
set -eu
cp /dl/rc-keeper /usr/local/bin/rc-keeper
printf '#!/bin/sh\necho "Remote Control active: https://claude.ai/code/session_headless"\nexec sleep 3600\n' > /usr/local/bin/claude
chmod +x /usr/local/bin/claude
useradd -m dev
su dev -s /bin/sh -c '
set -eu
fail() { echo "FAIL: $*"; rc-keeper status || true; exit 1; }
kids() { ps -eo args | grep -c "[s]leep 3600" || true; }
cd ~ && mkdir -p proj-a proj-b proj-c
rc-keeper version
rc-keeper add ~/proj-a --name A --permission-mode acceptEdits >/dev/null
rc-keeper add ~/proj-b --no-chrome >/dev/null
sleep 3
n=$(rc-keeper status --json | grep -c "\"state\": \"running\"" || true)
[ "$n" = 2 ] || fail "want 2 running after add (no start call), got $n"
rc-keeper status | grep -q session_headless || fail "no session URL in status"
rc-keeper stop proj-b >/dev/null; rc-keeper start proj-b >/dev/null; sleep 2
[ "$(kids)" = 2 ] || fail "want 2 claude children after stop/start, got $(kids)"
ls ~/.config/io.github.tupe12334.rckeeper/instances/proj-a.json >/dev/null || fail "no instance file"
if rc-keeper service install >/tmp/svc 2>&1; then fail "service install succeeded with no systemctl"; fi
grep -q "nohup" /tmp/svc || fail "no fallback hint: $(cat /tmp/svc)"
id=$(rc-keeper add ~/proj-c); rc-keeper remove "$id" >/dev/null; sleep 2
[ "$(kids)" = 2 ] || fail "add then remove left $(kids) children, want 2"
rc-keeper daemon stop >/dev/null; sleep 1
[ "$(kids)" = 0 ] || fail "daemon stop left $(kids) children"
echo "headless Linux check: OK"
'
IN
