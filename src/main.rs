mod auth;
mod cli;
mod curl_ffi;
mod gui;
mod logger;
mod parse;
mod runelite;

use clap::Parser;
use std::ffi::CString;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "rlshim",
    version = env!("CARGO_PKG_VERSION"),
    about = concat!(
        "rlshim v",
        env!("CARGO_PKG_VERSION"),
        " - https://github.com/RdrSeraphim/rlshim\nA lightweight, native shim for launching RuneLite on Linux with Jagex Accounts."
    )
)]
struct Args {
    /// disables gui, all applicable prompts sent to stdout
    #[arg(short = 't', long = "no-gui")]
    dont_use_gui: bool,

    /// log out of rlshim, clearing all saved credentials
    #[arg(short = 'l', long = "logout")]
    do_logout: bool,

    /// use the current saved character or, if none saved, save the character chosen at the selection menu
    #[arg(short = 's', long = "use-saved-char")]
    use_saved_character: bool,

    /// clears the currently saved character. if --use-saved-char is set, this clears the saved character and will save the next selected character
    #[arg(short = 'c', long = "clear-saved-char")]
    clear_saved_character: bool,

    /// flags to pass to RuneLite
    #[arg(short = 'f', long = "flags", value_delimiter = ' ', num_args = 1..)]
    rl_flags: Vec<String>,
}

fn main() -> ExitCode {
    let args = Args::parse();

    if args.do_logout {
        auth::logout();
        return ExitCode::SUCCESS;
    }

    log_info!("running pre-flight checks...");

    if !runelite::is_valid_java_installed() {
        log_error!("java 11+ is not installed or not in PATH");
        if args.dont_use_gui {
            cli::warning_prompt("java 11+ is not installed or not in PATH");
        } else {
            gui::warning_prompt(
                "java error",
                "java 11+ is not installed or not in PATH",
                230,
            );
        }
        return ExitCode::FAILURE;
    }
    log_info!("java installation found ✔");

    if !runelite::establish_home() {
        log_error!("failed to find or create ~/.runelite");
        if args.dont_use_gui {
            cli::warning_prompt(
                "failed to find or create ~/.runelite, this is likely a permissions issue. ensure that the user running this application has write access to their home directory.",
            );
        } else {
            gui::warning_prompt(
                "home error",
                "failed to find or create ~/.runelite, this is likely a permissions issue.\nensure that the user running this application has write access to their home directory.",
                260,
            );
        }
        return ExitCode::FAILURE;
    }
    log_info!("~/.runelite found ✔");

    if !auth::keyring::is_available() {
        if args.dont_use_gui {
            cli::warning_prompt(
                "rlshim could not find a valid keyring. credentials cannot be stored securely at this point. please install libsecret and a keyring manager (e.g. gnome-keyring, kwallet, etc.) to proceed further.",
            );
        } else {
            gui::warning_prompt(
                "keyring error",
                "rlshim could not find a valid keyring. credentials cannot\nbe stored securely at this point. please install libsecret and a keyring\nmanager (e.g. gnome-keyring, kwallet, etc.) to proceed further.",
                260,
            );
        }
        return ExitCode::FAILURE;
    } else {
        log_info!("keyring available ✔");
    }

    let mut session: auth::AuthSession;
    let creds_opt = auth::read_session();

    let needs_interactive_login = match &creds_opt {
        None => true,
        Some(list) => list.is_empty() || list[0].tokens.refresh_token.is_empty(),
    };

    if needs_interactive_login {
        log_info!("no valid credentials found. starting interactive login...");
        let Some(new_session) = auth::do_interactive_login(args.dont_use_gui) else {
            log_error!("login failed or aborted");
            return ExitCode::FAILURE;
        };
        session = new_session;
        auth::save_session(&session);
        log_info!("successfully logged in and saved credentials");
    } else {
        session = creds_opt.unwrap().into_iter().next().unwrap();
        let refresh_token = session.tokens.refresh_token.clone();

        let Some(refreshed_session) = auth::refresh_oauth_token(&refresh_token) else {
            log_error!("failed to refresh oauth token");
            return ExitCode::FAILURE;
        };

        // Preserve `session.session_id` across OAuth token refreshes
        session.tokens = refreshed_session.tokens;
        auth::save_session(&session);
    }

    if let Some(accounts) = auth::get_game_accounts(&session.session_id)
        && !accounts.is_empty()
    {
        session.accounts = accounts;
        auth::save_session(&session);
    }

    if session.accounts.is_empty() {
        log_error!("no game accounts found, please ensure you have created a character.");
        return ExitCode::FAILURE;
    }

    if args.clear_saved_character {
        runelite::save_character("");
    }

    let mut selected_account: Option<auth::GameAccount> = None;
    if session.accounts.len() == 1 {
        selected_account = Some(session.accounts[0].clone());
    } else {
        if args.use_saved_character && !args.clear_saved_character {
            let saved_name = runelite::read_saved_character();
            if !saved_name.is_empty() {
                selected_account = session
                    .accounts
                    .iter()
                    .find(|acc| acc.display_name == saved_name)
                    .cloned();
            }
        }

        if selected_account.is_none() {
            let acc_opt = if args.dont_use_gui {
                cli::prompt_for_character(&session.accounts)
            } else {
                gui::prompt_for_character(&session.accounts)
            };
            let Some(acc) = acc_opt else {
                log_error!("character selection aborted or failed");
                return ExitCode::FAILURE;
            };
            if args.use_saved_character {
                runelite::save_character(&acc.display_name);
            }
            selected_account = Some(acc);
        }
    }

    let selected_account = selected_account.unwrap();

    if !runelite::establish_jar() {
        log_error!("failed to ensure runelite.jar exists");
        if args.dont_use_gui {
            cli::warning_prompt(
                "failed to ensure runelite.jar exists. ensure that you have write permissions to ~/.runelite and that there is sufficient disk space.",
            );
        } else {
            gui::warning_prompt(
                "runelite error",
                "failed to ensure runelite.jar exists\n\nensure that you have write permissions to ~/.runelite and that there is sufficient disk space.",
                260,
            );
        }
        return ExitCode::FAILURE;
    }

    let jar_path = runelite::jar_path();

    unsafe {
        let pid = libc::fork();
        if pid < 0 {
            log_error!("failed to fork process to launch RuneLite");
            return ExitCode::FAILURE;
        } else if pid == 0 {
            libc::setsid();
            libc::close(libc::STDIN_FILENO);

            // Safe in single-threaded forked child before execvp
            if let Ok(v) = CString::new(session.session_id.as_str()) {
                libc::setenv(c"JX_SESSION_ID".as_ptr(), v.as_ptr(), 1);
            }
            if let Ok(v) = CString::new(selected_account.account_id.as_str()) {
                libc::setenv(c"JX_CHARACTER_ID".as_ptr(), v.as_ptr(), 1);
            }
            if let Ok(v) = CString::new(selected_account.display_name.as_str()) {
                libc::setenv(c"JX_DISPLAY_NAME".as_ptr(), v.as_ptr(), 1);
            }

            libc::prctl(libc::PR_SET_DUMPABLE, 0);

            log_info!("everything appears to be in order, starting runelite now...");

            let c_java = CString::new("java").unwrap();
            let c_jar_flag = CString::new("-jar").unwrap();
            let c_jar_path = CString::new(jar_path).unwrap();

            let mut c_args: Vec<CString> = vec![c_java.clone(), c_jar_flag, c_jar_path];
            for flag in &args.rl_flags {
                if let Ok(c_flag) = CString::new(flag.as_str()) {
                    c_args.push(c_flag);
                }
            }

            let mut ptrs: Vec<*const libc::c_char> = c_args.iter().map(|s| s.as_ptr()).collect();
            ptrs.push(std::ptr::null());

            libc::execvp(c_java.as_ptr(), ptrs.as_ptr());

            log_error!("failed to launch RuneLite. Is java in your PATH?");
            libc::_exit(1);
        }
    }

    session.zeroize_sensitive();
    ExitCode::SUCCESS
}
