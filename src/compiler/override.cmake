# macOS paths usually start with /Users/*. Unfortunately, clang-cl interprets
# paths starting with /U as macro undefines, so we need to put a -- before the
# input file path to force it to be treated as a path. CMake's compilation rules
# should be tweaked accordingly, but until that's done, and to support older
# CMake versions, overriding compilation rules works well enough. This file will
# be included by cmake after the default compilation rules have already been set
# up, so we can just modify them instead of duplicating them entirely.
string(REPLACE "-c <SOURCE>" "-c -- <SOURCE>" CMAKE_C_COMPILE_OBJECT "${CMAKE_C_COMPILE_OBJECT}")
string(REPLACE "-c <SOURCE>" "-c -- <SOURCE>" CMAKE_CXX_COMPILE_OBJECT "${CMAKE_CXX_COMPILE_OBJECT}")
# why not CMAKE_RC_FLAGS_INIT out? fixed in cmake e53a968e (replace " /D", but in old cmake, it's "/D")
string(REPLACE "/D" "-D" CMAKE_RC_FLAGS "${CMAKE_RC_FLAGS_INIT}")
string(REPLACE "/D" "-D" CMAKE_RC_FLAGS_DEBUG "${CMAKE_RC_FLAGS_DEBUG_INIT}")
if(NOT CMAKE_HOST_WIN32)
  set(CMAKE_NINJA_CMCLDEPS_RC 0) # cmcldeps is windows only
endif()