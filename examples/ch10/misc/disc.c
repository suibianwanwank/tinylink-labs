int counter;
__attribute__((noinline, section(".init.text"))) void setup(void) { counter++; }
void _entry(void) { setup(); }
