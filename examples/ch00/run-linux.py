#!/usr/bin/env python3
"""Run chapter 00 in independent directories on native x86-64 Linux."""
import os, platform, subprocess, tempfile
from pathlib import Path
assert platform.system() == 'Linux' and platform.machine() == 'x86_64'
root=Path(tempfile.mkdtemp(prefix='linker-ch00.'))
print('WORKDIR='+str(root),flush=True)
def case(name, files, commands):
 d=root/name;d.mkdir()
 for filename,source in files.items():(d/filename).write_text(source)
 for cmd in commands:
  print('\n$ '+cmd,flush=True)
  p=subprocess.run(['bash','-c','ulimit -c 0; '+cmd],cwd=d,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
  print(p.stdout,end='',flush=True);print('status='+str(p.returncode),flush=True)
case('good', {'add.c': '// add.c\nint add(int a, int b) { return a + b; }\n', 'call_add.c': '// call_add.c\nint add(int a, int b);\n\nint main(void) {\n    return add(1, 2);\n}\n'}, ['cc call_add.c add.c -o app; ./app; echo exit=$?', 'cc -### call_add.c add.c -o app'])
case('mismatch', {'main.c': '// main.c\n#include <stdio.h>\n\nint y = 5;\nint x = 7;\n\nvoid set_x(void);   /* 定义在 other.c 里 */\n\nint main(void) {\n    set_x();\n    printf("x = %d, y = %d\\n", x, y);\n    return 0;\n}\n', 'other.c': '// other.c\nextern double x;\n\nvoid set_x(void) {\n    x = 1.0;\n}\n'}, ['musl-gcc -O2 -Wall -Wextra -c main.c other.c; musl-gcc -static main.o other.o -o prog; ./prog', 'readelf -lW prog; nm prog | grep -E " _start$| _start_c$| main$"; readelf -SW prog | grep -E "init_array|fini_array|data|bss"; nm main.o other.o; readelf -sW other.o | grep " x$"', 'musl-gcc -O2 -flto -static main.c other.c -o prog_lto'])
case('phases', {'bad.c': '// bad.c\nint add(int a, int b);\n\nint main(void) {\n    return add(1);\n}\n', 'add.c': '// add.c\nint add(int a, int b) { return a + b; }\n', 'main.c': '// main.c\nint add(int a, int b);\n\nint main(void) {\n    return add(1, 2);\n}\n'}, ['musl-gcc -c bad.c', 'musl-gcc -c main.c; musl-gcc -static main.o -o app', 'ld.lld -static main.o -o /dev/null', 'musl-gcc -shared -fPIC add.c -o libadd.so; musl-gcc main.c -L. -ladd -o calc_dyn; readelf -lW calc_dyn; readelf -dW calc_dyn | grep NEEDED', './calc_dyn', 'LD_LIBRARY_PATH=. ./calc_dyn; echo exit=$?'])
case('pointer', {'table.c': '// table.c\nint table[2] = {1, 2};\n', 'use.c': '// use.c\n#include <stdio.h>\n\nextern int *table;\n\nint main(void) {\n    printf("table[1] = %d\\n", table[1]);\n    return 0;\n}\n'}, ['musl-gcc -O2 -Wall -Wextra -static table.c use.c -o arr; ./arr', 'musl-gcc -O2 -c use.c; objdump -dr --no-show-raw-insn use.o'])
case('A', {'main.c': '#include <stdio.h>\nint square(int n) { return n * n; }\nint main(void) { printf("%d\\n", squre(3)); return 0; }\n'}, ['musl-gcc -static main.c -o prog'])
case('B', {'a.c': '// a.c\nint count;\nvoid bump(void) { count++; }\n', 'main.c': '// main.c\n#include <stdio.h>\nint count;\nvoid bump(void);\nint main(void) { bump(); printf("%d\\n", count); return 0; }\n'}, ['musl-gcc -c a.c main.c; musl-gcc -static a.o main.o -o prog', 'musl-gcc -fcommon -static a.c main.c -o common; ./common'])
case('C', {'util.c': '// util.c\nstatic int helper(int n) { return n + 1; }\nint twice(int n) { return helper(n) * 2; }\n', 'main.c': '// main.c\nint helper(int n);\nint main(void) { return helper(41); }\n'}, ['musl-gcc -c util.c main.c; musl-gcc -static util.o main.o -o prog', 'nm util.o'])
case('D', {'scale.h': '// scale.h\nint scale(int v);\n', 'scale.c': '// scale.c\n#include "scale.h"\nlong scale(long v) { return v * 10; }\n', 'main.c': '// main.c\n#include "scale.h"\nint main(void) { return scale(4); }\n'}, ['musl-gcc -static scale.c main.c -o prog'])
case('E', {'greet.c': '// greet.c\n#include <stdio.h>\nvoid greet(void) { puts("hello from libgreet"); }\n', 'main.c': '// main.c\nvoid greet(void);\nint main(void) { greet(); return 0; }\n'}, ['musl-gcc -shared -fPIC greet.c -o libgreet.so; musl-gcc main.c -L. -lgreet -o prog; ./prog'])
case('F', {'limit.c': '// limit.c\nlong long limit = 5000000000LL;\n', 'main.c': '// main.c\n#include <stdio.h>\nextern int limit;\nint main(void) { printf("limit = %d\\n", limit); return 0; }\n'}, ['musl-gcc -Wall -Wextra -static limit.c main.c -o prog; ./prog'])
case('duplicate', {'main.c': '// main.c\n#include <stdio.h>\n\nint y = 5;\nint x = 7;\n\nvoid set_x(void);   /* 定义在 other.c 里 */\n\nint main(void) {\n    set_x();\n    printf("x = %d, y = %d\\n", x, y);\n    return 0;\n}\n', 'other.c': 'double x = 1.0;\nvoid set_x(void) {}\n'}, ['musl-gcc -O2 -Wall -Wextra -c main.c other.c; musl-gcc -static main.o other.o -o prog', 'ld.lld -static main.o other.o -o /dev/null; readelf -sW main.o other.o | grep " x$"'])
case('fixed', {'main.c': '// main.c\n#include <stdio.h>\n\nint y = 5;\nint x = 7;\n\nvoid set_x(void);   /* 定义在 other.c 里 */\n\nint main(void) {\n    set_x();\n    printf("x = %d, y = %d\\n", x, y);\n    return 0;\n}\n', 'other.c': 'extern int x;\nvoid set_x(void) { x = 1; }\n'}, ['musl-gcc -O2 -Wall -Wextra -static main.c other.c -o prog; ./prog'])

case('order-variants', {'main.c':'#include <stdio.h>\nint y=5;int x=7;void set_x(void);int main(void){set_x();printf("x = %d, y = %d\\n",x,y);}', 'other.c':'extern double x;void set_x(void){x=1.0;}'}, ['musl-gcc -O0 -static main.c other.c -o o0; ./o0','musl-gcc -O2 -fno-toplevel-reorder -static main.c other.c -o order; ./order','clang -O2 -c main.c other.c; musl-gcc -static main.o other.o -o cl; ./cl'])
case("D-no-header",{'scale.c': 'long scale(long v){return v*10;}', 'main.c': 'int scale(int);int main(void){return scale(4);}'},['musl-gcc -Wall -Wextra -static scale.c main.c -o prog; ./prog; echo exit=$?'])
