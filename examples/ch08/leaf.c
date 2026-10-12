// leaf.c
int ext(int);
int twice(int x) { return ext(x) + ext(x + 1); }
