extern int counter;
int bump(int);

int main(void) {
    bump(1);
    return counter;
}
