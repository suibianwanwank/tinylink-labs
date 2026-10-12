// tmpl.cpp：模板实例化出大量字节相同、名字不同的函数
#include <algorithm>
#include <map>
#include <vector>

template <class T> static int work(int n) {
    std::vector<T> v;
    for (int i = 0; i < n; i++) v.push_back(reinterpret_cast<T>(static_cast<long>((i * 7919) % n)));
    std::sort(v.begin(), v.end());
    std::map<T, int> m;
    for (auto p : v) m[p]++;
    return static_cast<int>(m.size());
}

int main(int argc, char **) {
    return work<int *>(argc * 100) + work<long *>(argc * 100) + work<char *>(argc * 100) +
           work<double *>(argc * 100) + work<short *>(argc * 100);
}
