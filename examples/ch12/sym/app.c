// app.c
int plugin_hook(int x) { return x + 100; }   // 程序里没人调用
int helper(int x) { return x * 2; }

int main(void) { return helper(21); }
