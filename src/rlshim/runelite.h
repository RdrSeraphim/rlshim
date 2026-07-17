#ifndef RLSHIM_RUNELITE_H
#define RLSHIM_RUNELITE_H

#include <string>

namespace runelite {
    bool is_valid_java_installed();
    bool establish_home();
    bool establish_jar();

    // Reads the display name of the saved character (~/.runelite/rlshim_char).
    // Returns an empty string if the file is missing, empty, or unreadable.
    // Any trailing newline is stripped.
    std::string read_saved_character();

    // Persists the given display name as the saved character, overwriting any
    // previous value. Returns false on failure.
    bool save_character(const std::string& display_name);

    // Absolute path to the RuneLite jar (~/.runelite/runelite.jar). Returns an
    // empty string if $HOME is unset. Callers should treat empty as an error.
    std::string jar_path();
}  // namespace runelite
#endif  // RLSHIM_RUNELITE_H
