#include <errno.h>
#include <stdio.h>
#include <unistd.h>
extern _Thread_local int counter;
int main(void) { counter++; if (close(-1) < 0) printf("counter=%d errno=%d\n", counter, errno); return 0; }
