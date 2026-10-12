// main.c
int scale(int x);

int main(int argc, char **argv) {
    unsigned s = 0;
    for (int i = 0; i < 400000000; i++)
        s += scale(i ^ argc);
    return s >> 24;
}
