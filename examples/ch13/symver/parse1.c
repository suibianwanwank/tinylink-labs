/* libparse 1：遇到非数字就停下，什么都没读到时返回 0 */
int parse_num(const char *s) {
    int v = 0;
    while (*s >= '0' && *s <= '9') v = v * 10 + (*s++ - '0');
    return v;
}
