#!/usr/bin/env python3
"""Verify separate-debug-file lookup on native x86-64 Linux."""
from pathlib import Path
import os
import platform
import re
import shutil
import struct
import subprocess
import tempfile
import zlib


def main():
    if (platform.system(), platform.machine()) != ("Linux", "x86_64"):
        raise SystemExit("requires native x86-64 Linux")
    source = Path(__file__).resolve().parent
    env = dict(os.environ, LC_ALL="C", DEBUGINFOD_URLS="")
    with tempfile.TemporaryDirectory(prefix="linker-debug-companion-") as directory:
        work = Path(directory)
        for name in ("main.c", "util.c"):
            shutil.copyfile(source / name, work / name)

        def run(*args):
            result = subprocess.run(args, cwd=work, env=env, check=True,
                                    text=True, stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, timeout=20)
            return result.stdout

        run("gcc", "-O0", "-g", "-Wl,--build-id=sha1", "main.c", "util.c", "-o", "prog")
        assert run("./prog").strip() == "42"
        (work / "dbg").mkdir()
        run("objcopy", "--only-keep-debug", "prog", "dbg/prog.debug")
        run("objcopy", "--strip-debug", "--strip-unneeded", "prog", "dbg/prog")
        run("objcopy", "--add-gnu-debuglink=dbg/prog.debug", "dbg/prog")
        run("objcopy", "--dump-section", ".gnu_debuglink=debuglink.bin", "dbg/prog", "inspect-copy")

        record = (work / "debuglink.bin").read_bytes()
        nul = record.index(0)
        assert record[:nul] == b"prog.debug"
        crc_offset = (nul + 1 + 3) & ~3
        assert len(record) == crc_offset + 4
        assert record[nul:crc_offset] == bytes(crc_offset - nul)
        stored_crc = struct.unpack_from("<I", record, crc_offset)[0]
        assert stored_crc == zlib.crc32((work / "dbg/prog.debug").read_bytes())

        def build_id(path):
            return re.search(r"Build ID: ([0-9a-f]+)", run("readelf", "-n", path)).group(1)

        identifier = build_id("prog")
        assert identifier == build_id("dbg/prog") == build_id("dbg/prog.debug")
        root = work / "debug-root"
        root.mkdir()

        def lookup():
            return run("gdb", "-nx", "-q", "-batch", "-iex", "set debuginfod enabled off",
                       "-iex", f"set debug-file-directory {root}", "-iex", "set verbose on",
                       "-ex", "info line add", "dbg/prog")

        by_name = lookup()
        assert "prog.debug" in by_name and 'of "util.c"' in by_name
        print("debuglink: CRC matches and GDB resolves util.c")

        destination = root / ".build-id" / identifier[:2] / (identifier[2:] + ".debug")
        destination.parent.mkdir(parents=True)
        shutil.move(work / "dbg/prog.debug", destination)
        assert not (work / "dbg/prog.debug").exists()
        by_id = lookup()
        assert str(destination) in by_id and 'of "util.c"' in by_id
        print("build-id: original companion moved; GDB resolves util.c through the ID tree")
        print(f"CRC={stored_crc:#010x}; build ID={identifier}")


if __name__ == "__main__":
    main()
