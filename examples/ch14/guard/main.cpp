#include <cstdio>
int &config();
int use_inl();
int main() {
    std::printf("%d %d %d\n", config(), config(), use_inl());
}
