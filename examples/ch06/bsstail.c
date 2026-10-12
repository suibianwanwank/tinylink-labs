#include <stdio.h>
#include <elf.h>
#include <unistd.h>
#include <sys/auxv.h>

int counter = 1;          /* .data */
int buffer[1024];         /* .bss  */

int main(void) {
    Elf64_Phdr *ph = (Elf64_Phdr *)getauxval(AT_PHDR);
    unsigned long n = getauxval(AT_PHNUM);
    for (unsigned long i = 0; i < n; i++) {
        if (ph[i].p_type != PT_LOAD || !(ph[i].p_flags & PF_W)) continue;
        unsigned long fend = ph[i].p_vaddr + ph[i].p_filesz;   /* 文件内容在内存里的终点 */
        unsigned long off  = ph[i].p_offset + ph[i].p_filesz;  /* 对应的文件偏移 */
        printf("RW: vaddr %#lx filesz %#lx memsz %#lx -> file part ends at %#lx\n",
               (unsigned long)ph[i].p_vaddr, (unsigned long)ph[i].p_filesz,
               (unsigned long)ph[i].p_memsz, fend);
        unsigned char disk[16];
        FILE *f = fopen("/proc/self/exe", "rb");
        fseek(f, off, SEEK_SET); fread(disk, 1, 16, f); fclose(f);
        printf("file  @%#lx:", off);
        for (int k = 0; k < 16; k++) printf(" %02x", disk[k]);
        printf("\nmemory@%#lx:", fend);
        for (int k = 0; k < 16; k++) printf(" %02x", ((unsigned char *)fend)[k]);
        printf("\n");
    }
    printf("&buffer = %p, sbrk(0) = %p\n", (void *)buffer, sbrk(0));
    FILE *m = fopen("/proc/self/maps", "r"); char line[256];
    while (fgets(line, sizeof line, m)) fputs(line, stdout);
    return 0;
}
