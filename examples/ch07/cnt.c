extern int ext_var;                  /* 别的模块定义 */
extern int ext_fn(int);
static int priv[8];
int pub_var = 1;
static int sfn(int x) { return x; }
__attribute__((visibility("hidden"))) int hid_fn(int x) { return x + 2; }
int pub_fn(int x) { return x + 1; }

int *p_priv = &priv[3];
int *p_pub = &pub_var;
int *p_ext = &ext_var;
const char *names[] = { "red", "green", "blue" };
int (*tbl[])(int) = { sfn, hid_fn, pub_fn, ext_fn };

int use(int x) { return ext_fn(x) + pub_fn(x) + ext_var + pub_var; }
