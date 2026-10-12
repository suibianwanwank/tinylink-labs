// util.c
int scale(int x) { return 3 * x + 1; }

int twice(int x) { return 2 * x; }

unsigned checksum(const unsigned char *p, int n) {
    unsigned h = 2166136261u;
    for (int i = 0; i < n; i++)
        h = (h ^ p[i]) * 16777619u;
    return h;
}
