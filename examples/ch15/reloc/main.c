// main.c
int *addr_of_elem(void);
int read_ext(void);
void call_it(void);
extern int *p_elem;
int main(void) { call_it(); return (int)(addr_of_elem() - p_elem) + read_ext(); }
