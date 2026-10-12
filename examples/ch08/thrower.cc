extern "C" int may_fail(int);
struct Guard { ~Guard(); };
int run(int x) {
  Guard g;
  try { return may_fail(x); }
  catch (int e) { return -e; }
}
void boom() { throw 42; }
