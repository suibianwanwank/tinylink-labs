#!/bin/sh
# Linux 原生实验；所有步骤失败均汇总。
W=$(mktemp -d "${TMPDIR:-/tmp}/linker-ch12.XXXXXX"); export W
printf 'experiment directory: %s\n' "$W"
failed=""
D=$(cd "$(dirname "$0")" && pwd)
for s in puzzle/run.sh mech/run.sh mech/debug.sh thinlto/build.sh thinlto/time.sh thinlto/puzzle-thin.sh \
         sym/run.sh lib/run.sh icf/run.sh order/run.sh ex/run.sh \
         icfhash/fetch-tinylink.sh icfhash/check-tmpl.sh icfhash/check-lua.sh icfhash/tests/run.sh; do
  echo "######## $s"
  sh "$D/$s" || failed="$failed $s"
done
[ -z "$failed" ] || { echo "FAILED:$failed"; exit 1; }
