#include <sys/prctl.h>
#include <unistd.h>
#include <nlohmann/json.hpp>
#include <format>

#include "CLI11.hpp"

#include "rlshim/auth.h"
#include "rlshim/cli.h"
#include "rlshim/gui.h"
#include "rlshim/logger.h"
#include "rlshim/runelite.h"

using json = nlohmann::json;

#ifndef RLSHIM_VERSION
#define RLSHIM_VERSION "1.2.1"
#endif

int main(int argc, char* argv[]) {
    CLI::App app{std::format("rlshim v{} - https://github.com/RdrSeraphim/rlshim\n", RLSHIM_VERSION) +
        "A lightweight, native shim for launching RuneLite on Linux with Jagex Accounts.", "rlshim"};
    argv = app.ensure_utf8(argv);

    bool dont_use_gui{false};
    app.add_flag("-t,--no-gui", dont_use_gui, "disables gui, all applicable prompts sent to stdout");

    bool do_logout{false};
    app.add_flag("-l,--logout", do_logout, "log out of rlshim, clearing all saved credentials");

    bool use_saved_character{false};
    app.add_flag("-s,--use-saved-char", use_saved_character, "use the current saved character or, if none saved, save the character chosen at the selection menu");

    bool clear_saved_character{false};
    app.add_flag("-c,--clear-saved-char", clear_saved_character, "clears the currently saved character. if --use-saved-char is set, this clears the saved character and will save the next selected character");

    std::vector<std::string> rl_flags{};
    app.add_option("-f,--flags", rl_flags, "flags to pass to RuneLite")->delimiter(' ');

    CLI11_PARSE(app, argc, argv);

    if (do_logout) {
        auth::logout();
        return 0;
    }

    logger::info("running pre-flight checks...");

    if (!runelite::is_valid_java_installed()) {
        logger::error("java 11+ is not installed or not in PATH");
        dont_use_gui ? cli::warning_prompt("java 11+ is not installed or not in PATH")
            : gui::warning_prompt("java error", "java 11+ is not installed or not in PATH", 230);
        return 1;
    }
    logger::info("java installation found ✔");

    if (!runelite::establish_home()) {
        logger::error("failed to find or create ~/.runelite");
        dont_use_gui
            ? cli::warning_prompt(
                  "failed to find or create ~/.runelite, this is likely a permissions issue. ensure that the user "
                  "running this application has write access to their home directory.")
            : gui::warning_prompt("home error",
                                  "failed to find or create ~/.runelite, this is likely a permissions issue.\nensure "
                                  "that the user running this application has write access to their home directory.");

        return 1;
    }
    logger::info("~/.runelite found ✔");

    if (!auth::keyring::is_available()) {
        dont_use_gui
            ? cli::warning_prompt(
                  "rlshim could not find a valid keyring. credentials cannot be stored securely at this point. please "
                  "install libsecret and a keyring manager (e.g. gnome-keyring, kwallet, etc.) to proceed further.")
            : gui::warning_prompt(
                  "keyring error",
                  "rlshim could not find a valid keyring. credentials cannot\nbe stored securely at this point. please "
                  "install libsecret and a keyring\nmanager (e.g. gnome-keyring, kwallet, etc.) to proceed further.");

        return 1;
    } else {
        logger::info("keyring available ✔");
    }

    auth::auth_session session;
    auto creds_opt = auth::read_session();

    if (!creds_opt || creds_opt->empty() || creds_opt->at(0).tokens.refresh_token.empty()) {
        logger::info("no valid credentials found. starting interactive login...");
        auto new_session = auth::do_interactive_login(dont_use_gui);
        if (!new_session) {
            logger::error("login failed or aborted");
            return 1;
        }
        session = *new_session;
        auth::save_session(session);
        logger::info("successfully logged in and saved credentials");
    } else {
        session = creds_opt->at(0);
        std::string refresh_token = session.tokens.refresh_token;

        auto refreshed_session = auth::refresh_oauth_token(refresh_token);
        if (!refreshed_session) {
            logger::error("failed to refresh oauth token");
            return 1;
        }

        // Only the OAuth tokens are replaced here. The game session_id is
        // deliberately preserved from the stored session: it comes from a
        // separate, interactive consent flow (get_consent_id_token ->
        // get_session_id) that refreshing the OAuth token does NOT re-issue.
        // Overwriting it would force the user back through that browser flow on
        // every launch. It appears to be long-lived, so we keep reusing it.
        session.tokens = refreshed_session->tokens;
        auth::save_session(session);
    }

    auto accounts_opt = auth::get_game_accounts(session.session_id);
    if (accounts_opt && !accounts_opt->empty()) {
        session.accounts = *accounts_opt;
        auth::save_session(session);
    }

    if (session.accounts.empty()) {
        logger::error("no game accounts found, please ensure you have created a character.");
        return 1;
    }

    if (clear_saved_character) {
        runelite::save_character("");
    }

    std::optional<auth::game_account> selected_account;
    if (session.accounts.size() == 1) {
        selected_account = session.accounts[0];
    } else {
        if (use_saved_character && !clear_saved_character) {
            std::string saved_name = runelite::read_saved_character();
            if (!saved_name.empty()) {
                for (const auth::game_account& acc : session.accounts) {
                    if (acc.displayName == saved_name) {
                        selected_account = acc;
                        break;
                    }
                }
            }
        }
        if (!selected_account) {
            auto acc_opt =
                dont_use_gui ? cli::prompt_for_character(session.accounts) : gui::prompt_for_character(session.accounts);
            if (!acc_opt) {
                logger::error("character selection aborted or failed");
                return 1;
            }
            selected_account = *acc_opt;
            if (use_saved_character)
                runelite::save_character(selected_account->displayName);
        }
    }

    if (!runelite::establish_jar()) {
        logger::error("failed to ensure runelite.jar exists");
        dont_use_gui
            ? cli::warning_prompt(
                      "failed to ensure runelite.jar exists. ensure that you have write permissions to ~/.runelite and "
                      "that there is sufficient disk space.")
            : gui::warning_prompt("runelite error",
                      "failed to ensure runelite.jar exists\n\nensure that you have write "
                      "permissions to ~/.runelite and that there is sufficient disk space.");

        return 1;
    }
    std::string java = "java";
    std::string jar_path = runelite::jar_path();

    pid_t pid = fork();
    if (pid < 0) {
        logger::error("failed to fork process to launch RuneLite");
        return 1;
    } else if (pid == 0) {
        setsid();
        close(STDIN_FILENO);

        setenv("JX_SESSION_ID", session.session_id.c_str(), 1);
        setenv("JX_CHARACTER_ID", selected_account->accountId.c_str(), 1);
        setenv("JX_DISPLAY_NAME", selected_account->displayName.c_str(), 1);

        prctl(PR_SET_DUMPABLE, 0);

        logger::info("everything appears to be in order, starting runelite now...");
        std::string jar_flag_str = "-jar";

        std::vector<char*> exec_args;
        exec_args.push_back(const_cast<char*>(java.c_str()));
        exec_args.push_back(const_cast<char*>(jar_flag_str.c_str()));
        exec_args.push_back(const_cast<char*>(jar_path.c_str()));

        for (const std::string& rl_flag : rl_flags) {
            exec_args.push_back(const_cast<char*>(rl_flag.c_str()));
        }
        exec_args.push_back(nullptr);

        execvp(java.c_str(), exec_args.data());

        // If execvp returns, it failed
        logger::error("failed to launch RuneLite. Is java in your PATH?");
        _exit(1);
    }

    explicit_bzero(session.session_id.data(), session.session_id.size());
    explicit_bzero(session.tokens.refresh_token.data(), session.tokens.refresh_token.size());
    explicit_bzero(session.tokens.id_token.data(), session.tokens.id_token.size());

    return 0;
}
