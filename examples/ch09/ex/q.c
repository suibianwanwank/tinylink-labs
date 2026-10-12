_Thread_local char tag[6] = "hello";
_Thread_local long seq = 1;
_Thread_local int hits;
_Thread_local _Alignas(32) char slab[40];

char *tag_p(void)  { return tag; }
long  get_seq(void) { return seq; }
int   get_hits(void) { return hits; }
char *slab_p(void) { return slab; }
