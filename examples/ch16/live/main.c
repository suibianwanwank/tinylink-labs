// main.c：只用了 handlers[0]
typedef void (*handler)(void);
extern handler handlers[];
int main(void) { handlers[0](); return 0; }
