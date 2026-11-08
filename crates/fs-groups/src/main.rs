//! `fs-groups`: one binary for everything (ADR 0063). Pattern: Facade over the crates that stay
//! libraries.
//!
//! ```text
//! fs-groups                          the window (feature `gui`, on by default)
//! fs-groups [--socket F] [--size WxH] [--snapshot F.png]
//! fs-groups serve                    the server (Cap'n Proto, groups.capnp)
//! ```

use std::process::ExitCode;

use fs_groups_server::ServeError;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("serve") => serve(&args[1..]),
        // `fs-groups --socket PATH serve`: the option may stand before the word.
        Some("--socket") if args.get(2).map(String::as_str) == Some("serve") => {
            let mut words = vec!["--socket".to_string(), args[1].clone()];
            words.extend(args[3..].iter().cloned());
            serve(&words)
        }
        None | Some("gui") => window(args.get(1..).unwrap_or_default()),
        Some(_) => window(&args),
    }
}

fn serve(words: &[String]) -> ExitCode {
    match fs_groups_server::serve(words.iter().map(Into::into)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(ServeError::Usage(m)) => {
            eprintln!("fs-groups: {m}");
            ExitCode::from(2)
        }
        Err(ServeError::Failed(m)) => {
            eprintln!("fs-groups: {m}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(feature = "gui")]
fn window(words: &[String]) -> ExitCode {
    match fs_groups_app::gui::run(words) {
        Ok(()) => ExitCode::SUCCESS,
        Err(m) => {
            eprintln!("fs-groups: {m}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(feature = "gui"))]
fn window(_words: &[String]) -> ExitCode {
    eprintln!(
        "fs-groups: this build has no window (built with --no-default-features); use `fs-groups serve`"
    );
    ExitCode::FAILURE
}
