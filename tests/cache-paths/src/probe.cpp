#include <windows.h>
#include <stdio.h>

// Exercise C++ and the SDK/CRT include paths without requiring a particular
// Microsoft STL version's minimum Clang version.
template <typename T> constexpr int type_size() { return sizeof(T); }
extern "C" int cpp_probe(void) { return type_size<DWORD>(); }
