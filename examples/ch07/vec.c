int counter;

__attribute__((noinline)) int helper(int x) { return x * 2; }

int bump(int x) {
    counter += helper(x);
    return counter;
}
