#include <pthread.h>
#include <stdio.h>
#include <stdint.h>

_Thread_local char tag[6] = "hello";
_Thread_local long seq = 1;
_Thread_local int hits;
_Thread_local _Alignas(32) char slab[40];

static char *tp(void) {
#if defined(__x86_64__)
    char *p; __asm__("movq %%fs:0, %0" : "=r"(p)); return p;
#else
    return __builtin_thread_pointer();   /* 读 TPIDR_EL0 的内建函数 */
#endif
}

static pthread_barrier_t bar;

static void *show(void *name) {
    if (((char *)name)[0] == 't') pthread_barrier_wait(&bar);
    char *t = tp();
    int local;
    hits++;
    printf("%-4s tp=%p &local=%p &tag=%p  tag%+ld seq%+ld hits%+ld slab%+ld hits=%d\n",
           (char *)name, (void *)t, (void *)&local, (void *)tag,
           (long)((intptr_t)tag - (intptr_t)t), (long)((intptr_t)&seq - (intptr_t)t),
           (long)((intptr_t)&hits - (intptr_t)t), (long)((intptr_t)slab - (intptr_t)t), hits);
    return 0;
}

int main(void) {
    pthread_t a, b;
    pthread_barrier_init(&bar, 0, 2);
    show("main");
    pthread_create(&a, 0, show, "t1");
    pthread_create(&b, 0, show, "t2");
    pthread_join(a, 0); pthread_join(b, 0);
    return 0;
}
