// main.c
int add(int a, int b);

int counter = 42;
int zeros[64];
const int table[4] = {1, 2, 3, 4};
const char *msg = "hello";

int main(void) {
    return add(counter, table[2]) * 2;
}
