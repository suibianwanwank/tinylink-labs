// host.c：主程序提供 plugin_hook，插件在运行时按名字找它
#include <dlfcn.h>
#include <stdio.h>

int plugin_hook(int x) { return x + 100; }

int main(void) {
    void *h = dlopen("./plugin.so", RTLD_NOW);
    if (!h) { printf("dlopen: %s\n", dlerror()); return 1; }
    int (*run)(int) = (int (*)(int))dlsym(h, "plugin_run");
    printf("plugin_run(1) = %d\n", run(1));
    return 0;
}
