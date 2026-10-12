#!/bin/sh
# Native x86-64 Linux: libtool version arithmetic and parallel SONAME generations.
set -e
. "$(dirname "$0")/../env.sh"
stage soname
run_abi sh -c 'cd soname && sh steps.sh'

