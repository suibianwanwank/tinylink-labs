#include <sys/stat.h>
int main(void) { struct stat st; return stat("/", &st); }
