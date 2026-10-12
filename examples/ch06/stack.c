#include <stdio.h>
#include <elf.h>
#include <sys/auxv.h>

extern char _start[];
extern const Elf64_Ehdr __ehdr_start;

static const char *name(unsigned long t) {
    switch (t) {
    case AT_NULL: return "AT_NULL";     case AT_PHDR: return "AT_PHDR";
    case AT_PHENT: return "AT_PHENT";   case AT_PHNUM: return "AT_PHNUM";
    case AT_PAGESZ: return "AT_PAGESZ"; case AT_BASE: return "AT_BASE";
    case AT_FLAGS: return "AT_FLAGS";   case AT_ENTRY: return "AT_ENTRY";
    case AT_UID: return "AT_UID";       case AT_EUID: return "AT_EUID";
    case AT_GID: return "AT_GID";       case AT_EGID: return "AT_EGID";
    case AT_PLATFORM: return "AT_PLATFORM"; case AT_HWCAP: return "AT_HWCAP";
    case AT_CLKTCK: return "AT_CLKTCK"; case AT_SECURE: return "AT_SECURE";
    case AT_RANDOM: return "AT_RANDOM"; case AT_HWCAP2: return "AT_HWCAP2";
    case AT_EXECFN: return "AT_EXECFN"; case AT_SYSINFO_EHDR: return "AT_SYSINFO_EHDR";
    case 27: return "AT_RSEQ_FEATURE_SIZE"; case 28: return "AT_RSEQ_ALIGN";
    case 51: return "AT_MINSIGSTKSZ";
    default: return "?";
    }
}

int main(int argc, char **argv, char **envp) {
    long *sp = (long *)argv - 1;              /* _start 收到的栈指针 */
    printf("%p  argc = %ld\n", (void *)sp, *sp);
    for (int i = 0; i <= argc; i++)
        printf("%p  argv[%d] = %p %s\n", (void *)&argv[i], i, (void *)argv[i], argv[i] ? argv[i] : "");
    char **e = envp;
    for (; *e; e++)
        printf("%p  envp    = %p %s\n", (void *)e, (void *)*e, *e);
    printf("%p  envp    = NULL\n", (void *)e);
    unsigned long *a = (unsigned long *)(e + 1);
    for (;; a += 2) {
        printf("%p  %-16s %#lx\n", (void *)a, name(a[0]), a[1]);
        if (a[0] == AT_NULL) break;
    }
    unsigned char *r = (unsigned char *)getauxval(AT_RANDOM);
    printf("AT_RANDOM bytes:");
    for (int i = 0; i < 16; i++) printf(" %02x", r[i]);
    printf("\n&_start = %p, phdr in image = %p\n", (void *)_start,
           (void *)((char *)&__ehdr_start + __ehdr_start.e_phoff));
    return 0;
}
