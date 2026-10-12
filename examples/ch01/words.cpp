// words.cpp
#include <map>
#include <string>
#include <vector>

int count_words(const std::vector<std::string> &ws) {
    std::map<std::string, int> freq;
    for (const auto &w : ws) freq[w]++;
    return static_cast<int>(freq.size());
}
