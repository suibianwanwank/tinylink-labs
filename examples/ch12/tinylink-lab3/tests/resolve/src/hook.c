extern void hook(void) __attribute__((weak));
int main(void) {
    if (hook)
        hook();
    return 0;
}
