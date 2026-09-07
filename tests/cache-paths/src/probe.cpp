#include <windows.h>
#include <vector>
extern "C" int cpp_probe(void) { return std::vector<int>(1, sizeof(DWORD))[0]; }
