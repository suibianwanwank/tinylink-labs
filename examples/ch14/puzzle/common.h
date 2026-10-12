// common.h：两个 .cpp 都包含它
inline int twice(int x) { return 2 * x; }

template <typename T>
T add(T a, T b) { return a + b; }

inline int next_id() {
    static int n = 0;   // 函数内的静态局部变量
    return ++n;
}
