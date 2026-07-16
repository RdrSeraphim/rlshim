#include <sys/prctl.h>
#include <unistd.h>
#include <cstdlib>
#include <nlohmann/json.hpp>

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
    logger::info("rlshim v{} - https://github.com/RdrSeraphim/rlshim", RLSHIM_VERSION);

    bool use_gui = true;
    bool do_logout = false;
    std::vector<char*> rl_args;

    for (int i = 1; i < argc; ++i) {
        std::string arg = argv[i];
        if (arg == "--no-gui") {
            use_gui = false;
        } else if (arg == "--logout") {
            do_logout = true;
        } else {
            rl_args.push_back(argv[i]);
        }
    }

    if (do_logout) {
        auth::logout();
        return 0;
    }

    logger::info("running pre-flight checks...");

    if (!runelite::is_valid_java_installed()) {
        logger::error("java 11+ is not installed or not in PATH");
        use_gui ? gui::warning_prompt("java error", "java 11+ is not installed or not in PATH", 230)
                : cli::warning_prompt("java 11+ is not installed or not in PATH");
        return 1;
    }
    logger::info("java installation found ✔");

    if (!runelite::establish_home()) {
        logger::error("failed to find or create ~/.runelite");
        use_gui
            ? gui::warning_prompt("home error",
                                  "failed to find or create ~/.runelite, this is likely a permissions issue.\nensure "
                                  "that the user running this application has write access to their home directory.")
            : cli::warning_prompt(
                  "failed to find or create ~/.runelite, this is likely a permissions issue. ensure that the user "
                  "running this application has write access to their home directory.");
        return 1;
    }
    logger::info("~/.runelite found ✔");

    if (!auth::keyring::is_available()) {
        use_gui
            ? gui::warning_prompt(
                  "keyring error",
                  "rlshim could not find a valid keyring. credentials cannot\nbe stored securely at this point. please "
                  "install libsecret and a keyring\nmanager (e.g. gnome-keyring, kwallet, etc.) to proceed further.")
            : cli::warning_prompt(
                  "rlshim could not find a valid keyring. credentials cannot be stored securely at this point. please "
                  "install libsecret and a keyring manager (e.g. gnome-keyring, kwallet, etc.) to proceed further.");
        return 1;
    } else {
        logger::info("keyring available ✔");
    }

    auth::auth_session session;
    auto creds_opt = auth::read_session();

    if (!creds_opt || creds_opt->empty() || creds_opt->at(0).tokens.refresh_token.empty()) {
        logger::info("no valid credentials found. starting interactive login...");
        auto new_session = auth::do_interactive_login(use_gui);
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

    auth::game_account selected_account;
    if (session.accounts.size() == 1) {
        selected_account = session.accounts[0];
    } else {
        auto acc_opt =
            use_gui ? gui::prompt_for_character(session.accounts) : cli::prompt_for_character(session.accounts);
        if (!acc_opt) {
            logger::error("character selection aborted or failed");
            return 1;
        }
        selected_account = *acc_opt;
    }

    if (!runelite::establish_jar()) {
        logger::error("failed to ensure runelite.jar exists");
        use_gui ? gui::warning_prompt("runelite error",
                                      "failed to ensure runelite.jar exists\n\nensure that you have write "
                                      "permissions to ~/.runelite and that there is sufficient disk space.")
                : cli::warning_prompt(
                      "failed to ensure runelite.jar exists. ensure that you have write permissions to ~/.runelite and "
                      "that there is sufficient disk space.");
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
        setenv("JX_CHARACTER_ID", selected_account.accountId.c_str(), 1);
        setenv("JX_DISPLAY_NAME", selected_account.displayName.c_str(), 1);

        prctl(PR_SET_DUMPABLE, 0);

        logger::info("everything appears to be in order, starting runelite now...");
        std::string jar_flag_str = "-jar";

        std::vector<char*> exec_args;
        exec_args.push_back(const_cast<char*>(java.c_str()));
        exec_args.push_back(const_cast<char*>(jar_flag_str.c_str()));
        exec_args.push_back(const_cast<char*>(jar_path.c_str()));

        for (char* arg : rl_args) {
            exec_args.push_back(arg);
        }
        exec_args.push_back(nullptr);

        execvp(java.c_str(), exec_args.data());

        // If execvp returns, it failed
        logger::error("failed to launch RuneLite. Is java in your PATH?");
        _exit(1);
    }

    // 5. Parent: wipe sensitive data from own memory
    explicit_bzero(session.session_id.data(), session.session_id.size());
    explicit_bzero(session.tokens.refresh_token.data(), session.tokens.refresh_token.size());
    explicit_bzero(session.tokens.id_token.data(), session.tokens.id_token.size());

    return 0;
}
