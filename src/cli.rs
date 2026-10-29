use crate::auth::GameAccount;
use std::io::{self, Write};

pub fn prompt_for_url(
    step_name: &str,
    instructions: &str,
    url_to_open: &str,
    url_placeholder: &str,
) -> Option<String> {
    println!("\n================ {step_name} ================\n");
    println!("{instructions}\n");
    println!("{url_to_open}\n");
    print!("paste redirect URL here ({url_placeholder}): ");
    let _ = io::stdout().flush();

    let mut pasted_url = String::new();
    if io::stdin().read_line(&mut pasted_url).is_err() {
        return None;
    }

    let trimmed = pasted_url.trim_end_matches(&['\r', '\n'][..]).to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub fn prompt_for_character(accounts: &[GameAccount]) -> Option<GameAccount> {
    if accounts.is_empty() {
        return None;
    }
    if accounts.len() == 1 {
        return Some(accounts[0].clone());
    }

    loop {
        println!("\n================ choose your character ================\n");
        for (i, acc) in accounts.iter().enumerate() {
            let display_name = if acc.display_name.is_empty() {
                "<no name set>"
            } else {
                &acc.display_name
            };
            println!("[{}] {}", i + 1, display_name);
        }
        print!("\nchoose a character (1-{}): ", accounts.len());
        let _ = io::stdout().flush();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            return None;
        }

        if let Ok(choice) = input.trim().parse::<usize>()
            && choice >= 1
            && choice <= accounts.len()
        {
            return Some(accounts[choice - 1].clone());
        }

        println!("invalid choice, please try again.");
    }
}

pub fn warning_prompt(warning: &str) {
    println!("\n================ rlshim warning ================\n");
    println!("{warning}\n");
    println!("please fix this issue and restart the application.\n");
    println!("press [enter] to exit...");
    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
}
