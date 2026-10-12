#include <pthread.h>
#include <stdio.h>

static _Thread_local int counter = 42;

static void *worker(void *unused) {
    (void)unused;
    printf("worker starts: %d\n", counter);
    counter += 10;
    printf("worker changes: %d\n", counter);
    return NULL;
}

int main(void) {
    pthread_t thread;
    ++counter;
    printf("main before: %d\n", counter);
    if (pthread_create(&thread, NULL, worker, NULL) != 0) return 1;
    if (pthread_join(thread, NULL) != 0) return 1;
    printf("main after: %d\n", counter);
    return 0;
}
