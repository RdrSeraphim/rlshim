#ifndef RLSHIM_LOGGER_H
#define RLSHIM_LOGGER_H
#include <format>
#include <print>

namespace logger {
    template <typename... Args>
    void info(std::format_string<Args...> fmt, Args&&... args) {
        std::print("\033[1;34m[info] \033[0m");
        std::println(fmt, std::forward<Args>(args)...);
    }

    template <typename... Args>
    void debug(std::format_string<Args...> fmt, Args&&... args) {
        std::print("\033[1;32m[debug] \033[0m");
        std::println(fmt, std::forward<Args>(args)...);
    }

    template <typename... Args>
    void error(std::format_string<Args...> fmt, Args&&... args) {
        std::print("\033[1;31m[error] \033[0m");
        std::println(fmt, std::forward<Args>(args)...);
    }
}  // namespace logger

#endif  // RLSHIM_LOGGER_H
