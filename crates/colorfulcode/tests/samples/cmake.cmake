cmake_minimum_required(VERSION 3.20)
project(colorful VERSION 1.2.0 LANGUAGES C CXX)

#[[ Bracket comments
    can span lines. ]]
option(BUILD_TESTS "Build the test suite" ON)
set(CMAKE_CXX_STANDARD 20)
set(SOURCES src/main.cpp src/lexer.cpp)

if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release CACHE STRING "Build type" FORCE)
endif()

# Helper that adds warnings to a target.
function(add_warnings target)
  if(MSVC)
    target_compile_options(${target} PRIVATE /W4)
  else()
    target_compile_options(${target} PRIVATE -Wall -Wextra)
  endif()
endfunction()

add_executable(colorful ${SOURCES})
add_warnings(colorful)
target_compile_definitions(colorful PRIVATE
  VERSION="${PROJECT_VERSION}"
  $<$<CONFIG:Debug>:DEBUG_BUILD=1>)
target_include_directories(colorful PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/include)

find_package(Threads REQUIRED)
target_link_libraries(colorful PRIVATE Threads::Threads)

foreach(name IN LISTS SOURCES)
  message(STATUS "source: ${name} (home: $ENV{HOME})")
endforeach()

file(WRITE ${CMAKE_BINARY_DIR}/info.txt [=[raw "text" ${not_expanded}]=])
if(BUILD_TESTS AND EXISTS "${CMAKE_SOURCE_DIR}/tests")
  enable_testing()
  add_subdirectory(tests)
endif()
