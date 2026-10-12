// where.c：printf 在运行时来自哪里
#include <dlfcn.h>
#include <stdio.h>
#include <mach-o/dyld.h>
int main(void) {
    Dl_info info;
    dladdr((void *)&printf, &info);
    printf("main   = %p\n", (void *)&main);
    printf("printf = %p\n", (void *)&printf);
    printf("image  = %s, base = %p\n", info.dli_fname, info.dli_fbase);
    printf("in shared cache: %d\n",
           _dyld_shared_cache_contains_path("/usr/lib/system/libsystem_c.dylib"));
    return 0;
}
