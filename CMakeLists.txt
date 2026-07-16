cmake_minimum_required(VERSION 3.15...4.1)
set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
if(NOT CMAKE_C_COMPILER AND NOT CMAKE_CXX_COMPILER)
    find_program(CLANG_C_COMPILER clang)
    find_program(CLANG_CXX_COMPILER clang++)
    if(CLANG_C_COMPILER AND CLANG_CXX_COMPILER)
        set(CMAKE_C_COMPILER ${CLANG_C_COMPILER})
        set(CMAKE_CXX_COMPILER ${CLANG_CXX_COMPILER})
    endif()
endif()

project(rlshim VERSION 1.2.0 LANGUAGES CXX)

if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE "Release" CACHE STRING "Choose the type of build, options are: Debug Release" FORCE)
endif()
message(STATUS "Build type: ${CMAKE_BUILD_TYPE}")

option(RLSHIM_BUILD_APP "Build the rlshim application (requires libsecret, curl, glfw, etc.)" ON)
option(RLSHIM_BUILD_TESTS "Build unit tests for rlshim's pure logic" OFF)

include(FetchContent)

if(RLSHIM_BUILD_APP)

find_package(PkgConfig REQUIRED)
pkg_check_modules(LIBSECRET REQUIRED IMPORTED_TARGET libsecret-1)

find_package(CURL REQUIRED)

set(CURL_IMP_VERSION "1.5.6")
set(CURL_IMP_DIR "${CMAKE_BINARY_DIR}/_deps/curl-impersonate")
set(CURL_IMP_ARCHIVE "${CURL_IMP_DIR}/libcurl-impersonate.tar.gz")

# Determine Architecture
if(CMAKE_SYSTEM_PROCESSOR MATCHES "^(aarch64|arm64)$")
    set(RLSHIM_ARCH "aarch64")
else()
    set(RLSHIM_ARCH "x86_64")
endif()

# Determine Libc
execute_process(COMMAND ${CMAKE_CXX_COMPILER} -dumpmachine OUTPUT_VARIABLE COMPILER_TARGET OUTPUT_STRIP_TRAILING_WHITESPACE)
if(COMPILER_TARGET MATCHES "musl")
    set(RLSHIM_LIBC "musl")
else()
    set(RLSHIM_LIBC "gnu")
endif()

set(CURL_IMP_URL "https://github.com/lexiforest/curl-impersonate/releases/download/v${CURL_IMP_VERSION}/libcurl-impersonate-v${CURL_IMP_VERSION}.${RLSHIM_ARCH}-linux-${RLSHIM_LIBC}.tar.gz")

if(NOT EXISTS "${CURL_IMP_DIR}/extracted")
    message(STATUS "Downloading libcurl-impersonate-chrome v${CURL_IMP_VERSION} for ${RLSHIM_ARCH}-linux-${RLSHIM_LIBC}...")
    file(DOWNLOAD
        "${CURL_IMP_URL}"
        "${CURL_IMP_ARCHIVE}"
        SHOW_PROGRESS
    )
    file(ARCHIVE_EXTRACT INPUT "${CURL_IMP_ARCHIVE}" DESTINATION "${CURL_IMP_DIR}/extracted")
endif()

set(CURL_IMP_ROOT "${CURL_IMP_DIR}/extracted")

# Static library — BoringSSL, nghttp2, brotli, zstd are all baked in
add_library(curl_impersonate_lib STATIC IMPORTED)
set_target_properties(curl_impersonate_lib PROPERTIES
    IMPORTED_LOCATION "${CURL_IMP_ROOT}/libcurl-impersonate.a"
    INTERFACE_INCLUDE_DIRECTORIES "${CURL_INCLUDE_DIRS}"
)
message(STATUS "curl-impersonate library: ${CURL_IMP_ROOT}/libcurl-impersonate.a")
message(STATUS "curl headers (system): ${CURL_INCLUDE_DIRS}")

FetchContent_Declare(json GIT_REPOSITORY https://github.com/nlohmann/json.git
                         GIT_TAG v3.12.0)
FetchContent_MakeAvailable(json)

FetchContent_Declare(glfw GIT_REPOSITORY https://github.com/glfw/glfw.git
                          GIT_TAG 3.3.8)
set(GLFW_BUILD_DOCS OFF CACHE BOOL "" FORCE)
set(GLFW_BUILD_TESTS OFF CACHE BOOL "" FORCE)
set(GLFW_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
set(GLFW_INSTALL OFF CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(glfw)

if(POLICY CMP0169)
    cmake_policy(SET CMP0169 OLD)
endif()
FetchContent_Declare(imgui GIT_REPOSITORY https://github.com/ocornut/imgui.git
                           GIT_TAG v1.92.8)
FetchContent_Populate(imgui)
set(IMGUI_DIR ${imgui_SOURCE_DIR})

set(IMGUI_SOURCES
    ${IMGUI_DIR}/imgui.cpp
    ${IMGUI_DIR}/imgui_draw.cpp
    ${IMGUI_DIR}/imgui_tables.cpp
    ${IMGUI_DIR}/imgui_widgets.cpp
    ${IMGUI_DIR}/backends/imgui_impl_glfw.cpp
    ${IMGUI_DIR}/backends/imgui_impl_opengl3.cpp
)
# Suppress compiler warnings specifically for ImGui sources
set_source_files_properties(${IMGUI_SOURCES} PROPERTIES COMPILE_FLAGS "-w")

find_package(OpenGL REQUIRED)

add_executable(rlshim
    src/main.cpp
    src/rlshim/auth.cpp
    src/rlshim/runelite.cpp
    src/rlshim/cli.cpp
    src/rlshim/gui.cpp
    src/rlshim/parse.cpp
    ${IMGUI_SOURCES}
)

target_include_directories(rlshim PRIVATE 
    ${CMAKE_SOURCE_DIR}/src 
    ${IMGUI_DIR} 
    ${IMGUI_DIR}/backends
)

target_compile_options(rlshim PRIVATE -Wall -Wextra -Wpedantic -Wno-missing-field-initializers)
target_compile_definitions(rlshim PRIVATE RLSHIM_VERSION=\"${PROJECT_VERSION}\")

find_package(OpenSSL REQUIRED)

target_link_libraries(rlshim PRIVATE
    curl_impersonate_lib
    PkgConfig::LIBSECRET
    nlohmann_json::nlohmann_json
    OpenSSL::Crypto
    glfw
    OpenGL::GL
    pthread
)

if(CMAKE_BUILD_TYPE STREQUAL "Release")
    include(CheckIPOSupported)
    check_ipo_supported(RESULT ipo_supported OUTPUT output LANGUAGES CXX)
    if(ipo_supported)
        message(STATUS "Enabling Link Time Optimization (LTO)")
        set_property(TARGET rlshim PROPERTY INTERPROCEDURAL_OPTIMIZATION TRUE)
    else()
        message(STATUS "LTO not supported: ${output}")
    endif()
endif()

install(TARGETS rlshim DESTINATION bin)
install(FILES "rlshim.desktop" DESTINATION share/applications)
install(DIRECTORY "data" DESTINATION share/rlshim)
install(FILES "data/icon.png" DESTINATION share/pixmaps RENAME "rlshim.png")

if(NOT TARGET uninstall)
    configure_file(
        "${CMAKE_CURRENT_SOURCE_DIR}/cmake_uninstall.cmake.in"
        "${CMAKE_CURRENT_BINARY_DIR}/cmake_uninstall.cmake"
        IMMEDIATE @ONLY)

    add_custom_target(uninstall
        COMMAND ${CMAKE_COMMAND} -P ${CMAKE_CURRENT_BINARY_DIR}/cmake_uninstall.cmake)
endif()

# Generate AUR PKGBUILD templates
configure_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/packaging/aur/PKGBUILD.source.in"
    "${CMAKE_CURRENT_BINARY_DIR}/aur/rlshim/PKGBUILD"
    @ONLY)

configure_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/packaging/aur/PKGBUILD.bin.in"
    "${CMAKE_CURRENT_BINARY_DIR}/aur/rlshim-bin/PKGBUILD"
    @ONLY)

configure_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/packaging/aur/PKGBUILD.git.in"
    "${CMAKE_CURRENT_BINARY_DIR}/aur/rlshim-git/PKGBUILD"
    @ONLY)


set(CPACK_PACKAGE_NAME "rlshim")
set(CPACK_PACKAGE_VERSION "${PROJECT_VERSION}")
set(CPACK_PACKAGE_FILE_NAME "${CPACK_PACKAGE_NAME}-${CPACK_PACKAGE_VERSION}-${RLSHIM_ARCH}-${RLSHIM_LIBC}")
set(CPACK_PACKAGE_CONTACT "Seraphim Pardee <me@srp.life>")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "A lightweight, native Linux launcher for RuneLite.")
set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_CURRENT_SOURCE_DIR}/LICENSE")

# RuneLite requires a JRE (11+). It is properly RuneLite's dependency, but since
# rlshim cannot function without it, we declare it here so users don't have to
# hunt it down themselves. The alternations/virtual provides let any conforming
# JRE satisfy it rather than pinning a specific vendor.
set(CPACK_DEBIAN_PACKAGE_DEPENDS "libsecret-1-0, default-jre | java11-runtime")
set(CPACK_RPM_PACKAGE_REQUIRES "libsecret, java >= 1:11")
set(CPACK_RPM_PACKAGE_LICENSE "BSD-2-Clause")

set(CPACK_COMPONENTS_GROUPING "ALL_COMPONENTS_IN_ONE")

include(CPack)

endif()  # RLSHIM_BUILD_APP

if(RLSHIM_BUILD_TESTS)
    enable_testing()

    # Fetch single headers directly (pinned) rather than adding their CMake
    # projects. doctest's old cmake_minimum_required breaks on CMake >= 4, and
    # this keeps the test build free of the app's heavy dependency tree.
    function(rlshim_fetch_header url dest)
        if(NOT EXISTS "${dest}")
            file(DOWNLOAD "${url}" "${dest}" TLS_VERIFY ON STATUS _dl_status)
            list(GET _dl_status 0 _dl_code)
            if(NOT _dl_code EQUAL 0)
                message(FATAL_ERROR "Failed to download ${url}: ${_dl_status}")
            endif()
        endif()
    endfunction()

    set(DOCTEST_VERSION "2.4.11")
    set(JSON_VERSION "3.12.0")
    rlshim_fetch_header(
        "https://raw.githubusercontent.com/doctest/doctest/v${DOCTEST_VERSION}/doctest/doctest.h"
        "${CMAKE_BINARY_DIR}/_deps/test-include/doctest/doctest.h")
    rlshim_fetch_header(
        "https://github.com/nlohmann/json/releases/download/v${JSON_VERSION}/json.hpp"
        "${CMAKE_BINARY_DIR}/_deps/test-include/nlohmann/json.hpp")

    # Only the pure logic is linked in — no curl/libsecret/imgui needed.
    add_executable(rlshim_tests
        tests/test_parse.cpp
        src/rlshim/parse.cpp
    )
    target_include_directories(rlshim_tests PRIVATE
        ${CMAKE_SOURCE_DIR}/src
        ${CMAKE_BINARY_DIR}/_deps/test-include
    )

    add_test(NAME rlshim_tests COMMAND rlshim_tests)
endif()
