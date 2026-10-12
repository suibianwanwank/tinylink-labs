#!/usr/bin/env bash
set -eu
[ "$(uname -s)" = Linux ] && [ "$(uname -m)" = x86_64 ]
work=$(mktemp -d /tmp/linker-ch03-rebuild.XXXXXX)
cd "$work"
echo "WORKDIR=$work"
cc --version | head -1
ld --version | head -1
expect_failure() {
  if "$@" > failure.log 2>&1; then
    echo "UNEXPECTED SUCCESS: $*"; exit 1
  fi
  cat failure.log
}

cat > add.h <<'EOF'
#ifndef ADD_H
#define ADD_H
int add(int a, int b);
#endif
EOF
cat > main.c <<'EOF'
#include <stdio.h>
#include "add.h"
int main(void) {
    printf("%d\n", add(1, 2));
    return 0;
}
EOF
cat > add.c <<'EOF'
#include "add.h"
int add(int a, int b) { return a + b; }
EOF
cc -O0 -Wall -Wextra -c main.c add.c  # -c: compile without linking
cc main.o add.o -o app              # include normal C startup and library support
./app
# 3

nm -A main.o add.o | grep ' add$'
# main.o:                 U add
# add.o:0000000000000000 T add
# Addresses and spacing can differ; U and T are the important observations.
expect_failure cc main.o -o missing-add   # Expected link failure: undefined reference to add

cc main.o add.o -Wl,--trace-symbol=add -o traced
# -Wl, passes the following option to the linker.
# The trace identifies main.o as a reference and add.o as a definition.

cat > alternate.c <<'EOF'
#include "add.h"
int add(int a, int b) { return a + b + 100; }
EOF
cc -O0 -Wall -Wextra -c alternate.c
expect_failure cc main.o add.o alternate.o -o duplicate
# Expected link failure: multiple definition of add

cat > fallback.c <<'EOF'
#include "add.h"
__attribute__((weak)) int add(int a, int b) { return a + b + 100; }
EOF
cc -O0 -Wall -Wextra -c fallback.c
nm fallback.o | grep ' add$'  # W: weak function definition
cc main.o fallback.o -o fallback-app
./fallback-app               # 103
cc main.o fallback.o add.o -o override-app
./override-app               # 3
cc main.o add.o fallback.o -o reverse-app
./reverse-app                # 3

cat > state-a.c <<'EOF'
int counter;
int main(void) { return counter; }
EOF
cat > state-b.c <<'EOF'
int counter;
EOF
cc -O0 -fno-common -c state-a.c state-b.c
nm -A state-a.o state-b.o | grep ' counter$'  # B in both inputs
expect_failure cc state-a.o state-b.o -o duplicate-state   # Expected duplicate-definition failure
cc -O0 -fcommon -c state-a.c state-b.c
nm -A state-a.o state-b.o | grep ' counter$'  # C in both inputs
cc state-a.o state-b.o -o shared-state
./shared-state                             # Exits successfully, with status 0
nm -S shared-state | grep ' counter$'       # One B symbol, size 4 on this target

mkdir -p archive
cd archive
cat > main.c <<'EOF'
#include <stdio.h>
int add(int, int);
int main(void) { printf("%d\n", add(1, 2)); return 0; }
EOF
cat > add.c <<'EOF'
int scale(int);
int add(int a, int b) { return scale(a + b); }
EOF
cat > scale.c <<'EOF'
int scale(int x) { return x * 2; }
EOF
cat > unused.c <<'EOF'
int unused_helper(void) { return 99; }
EOF
cc -O0 -Wall -Wextra -c main.c add.c scale.c unused.c
ar rcs libmath.a add.o unused.o  # r: add/replace; c: create; s: write symbol index
ar rcs libscale.a scale.o
ar t libmath.a
# add.o
# unused.o
cc -fuse-ld=bfd main.o libmath.a libscale.a -o app
./app                          # 6
if nm app | grep ' unused_helper$'; then exit 1; fi # No match; grep exits with status 1

expect_failure cc -fuse-ld=bfd main.o libscale.a libmath.a -o wrong-order
# Expected link failure: undefined reference to scale
cc -fuse-ld=bfd main.o libmath.a libscale.a -Wl,-Map=app.map -o app
head -12 app.map
# The opening archive report explains:
# libmath.a(add.o) was needed by main.o (add).
# libscale.a(scale.o) was needed by libmath.a(add.o) (scale).

# Still inside archive/. Grouping works for this example too.
cc -fuse-ld=bfd main.o \
  -Wl,--start-group libscale.a libmath.a -Wl,--end-group -o grouped
./grouped  # 6
# If LLD is installed, this reversed order also succeeds without a group:
cc -fuse-ld=lld main.o libscale.a libmath.a -Wl,--warn-backrefs -o with-lld
./with-lld # 6; the linker warns about the backward reference to scale
cd ..

mkdir -p types
cd types
cat > owner.c <<'EOF'
int value = 7;
EOF
cat > user.c <<'EOF'
extern double value;
int main(void) { return value != 0.0; }
EOF
cc -O0 -Wall -Wextra -c owner.c user.c
readelf -sW owner.o user.o | grep ' value$'
cc owner.o user.o -o type-mismatch
# Inspect the link result only. Do not run this invalid C program.
cat > value.h <<'EOF'
extern int value;
EOF
expect_failure cc -include value.h -c user.c -o checked.o
# Expected compile failure: conflicting types for value
cd ..

mkdir -p inline-demo
cd inline-demo
cat > version.h <<'EOF'
inline int version() { return 7; }
EOF
cat > one.cpp <<'EOF'
#include "version.h"
int from_one() { return version(); }
EOF
cat > two.cpp <<'EOF'
#include <cstdio>
#include "version.h"
int from_one();
int main() { std::printf("%d %d\n", from_one(), version()); }
EOF
c++ -O0 -c one.cpp two.cpp
nm -C one.o two.o | grep version
readelf --section-groups one.o two.o
c++ one.o two.o -o versions
./versions  # 7 7
cd ..

# Back in ~/linker-study/ch03; inspect the original inputs.
nm -A main.o add.o alternate.o fallback.o | grep ' add$'
cc main.o add.o -Wl,--trace-symbol=add -Wl,-Map=resolved.map -o resolved

[ "$(./app)" = 3 ]
[ "$(./fallback-app)" = 103 ]
[ "$(./override-app)" = 3 ]
[ "$(./reverse-app)" = 3 ]
[ "$(archive/app)" = 6 ]
[ "$(archive/grouped)" = 6 ]
[ "$(archive/with-lld)" = 6 ]
[ "$(inline-demo/versions)" = "7 7" ]
cc add.o main.o -o reversed
[ "$(./reversed)" = 3 ]
ar rcs libreal.a add.o
cc main.o fallback.o libreal.a -o chosen
[ "$(./chosen)" = 103 ]
cd archive
ar rcs libmath.a scale.o
cc -fuse-ld=bfd main.o libmath.a -o combined
[ "$(./combined)" = 6 ]
if nm combined | grep ' unused_helper$'; then exit 1; fi
printf '\nPASS: ch03 rewritten native Linux examples and exercise checks\n'
