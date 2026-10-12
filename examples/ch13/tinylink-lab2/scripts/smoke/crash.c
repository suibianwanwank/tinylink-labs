/* scripts/smoke-test.sh 用它检查崩溃能否正常返回：读空指针，期望退出码 139（128 + SIGSEGV）。 */
int main(void) {
    volatile int *p = 0;
    return *p;
}
