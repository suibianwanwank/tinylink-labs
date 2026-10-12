#include <pthread.h>
#include <stdio.h>
#ifdef OLD_PTHREAD
__asm__(".symver pthread_create, pthread_create@GLIBC_2.2.5");
#endif
static void *work(void *arg) { return arg; }
int main(void) {
    pthread_t t; void *r;
    pthread_create(&t, 0, work, (void *)42);
    pthread_join(t, &r);
    printf("thread returned %ld\n", (long)r);
    return 0;
}
