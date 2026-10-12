#!/bin/sh
set -e
readelf --dyn-syms -W /lib/x86_64-linux-gnu/libc.so.6 | grep -E ' (memcpy|memmove)@'
