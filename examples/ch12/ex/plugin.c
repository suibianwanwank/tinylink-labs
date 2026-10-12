// plugin.c：编成 plugin.so
int plugin_hook(int x);
int plugin_run(int x) { return plugin_hook(x) * 2; }
