// hello.c：不依赖 CRT 的 Windows 程序，只调用 kernel32 的 ExitProcess
__declspec(dllimport) void __stdcall ExitProcess(unsigned int code);
void __stdcall Sleep(unsigned int ms);   // 故意不写 dllimport

static int value = 40;
int *p_value = &value;                   // 数据里的绝对地址：需要基址重定位

int main(void) {
    Sleep(0);
    ExitProcess(*p_value + 2);
    return 0;
}
