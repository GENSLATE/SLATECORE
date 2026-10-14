//! Limits on the arguments the webview may pass to a program the launcher starts.
//!
//! An app's arguments are a command line. The UI may store a few for an app (`set_app_override`)
//! and the main window may start an app with others (`launch_app`), but the tray menu window
//! only starts apps as configured, and nobody gets an unbounded list: the count and the length of
//! every argument are capped.

/// The most arguments one app may have.
pub const MAX_ARGS: usize = 16;
/// The longest argument, in characters.
pub const MAX_ARG_CHARS: usize = 1024;
/// The window that may pass arguments to `launch_app`.
pub const MAIN_WINDOW: &str = "main";

/// Why arguments were refused (the text is shown to the person).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ArgsError {
    #[error("only the launcher window can start an app with its own arguments")]
    NotAllowedFrom,
    #[error("an app can have at most {MAX_ARGS} arguments")]
    TooMany,
    #[error("an argument can be at most {MAX_ARG_CHARS} characters long")]
    TooLong,
    #[error("an argument cannot contain a NUL character")]
    Nul,
}

/// Checks a list of stored or passed arguments.
///
/// # Errors
///
/// [`ArgsError::TooMany`], [`ArgsError::TooLong`] or [`ArgsError::Nul`].
pub fn check_list(args: &[String]) -> Result<(), ArgsError> {
    if args.len() > MAX_ARGS {
        return Err(ArgsError::TooMany);
    }
    for arg in args {
        if arg.contains('\0') {
            return Err(ArgsError::Nul);
        }
        if arg.chars().count() > MAX_ARG_CHARS {
            return Err(ArgsError::TooLong);
        }
    }
    Ok(())
}

/// Checks the `args` of a `launch_app` call made by the webview labelled `caller`.
///
/// # Errors
///
/// [`ArgsError::NotAllowedFrom`] for any `Some(_)` from a window other than `main`, otherwise the
/// errors of [`check_list`].
pub fn check_launch(caller: &str, args: Option<&[String]>) -> Result<(), ArgsError> {
    let Some(args) = args else {
        return Ok(());
    };
    if caller != MAIN_WINDOW {
        return Err(ArgsError::NotAllowedFrom);
    }
    check_list(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(count: usize) -> Vec<String> {
        (0..count).map(|n| format!("--flag{n}")).collect()
    }

    #[test]
    fn the_main_window_may_pass_a_few_arguments() {
        assert_eq!(check_launch("main", Some(&args(3))), Ok(()));
        assert_eq!(check_launch("main", None), Ok(()));
        assert_eq!(check_launch("main", Some(&args(MAX_ARGS))), Ok(()));
    }

    #[test]
    fn the_tray_menu_and_any_other_window_may_not_pass_arguments() {
        for caller in ["tray-menu", "other", "", "Main", "main2"] {
            assert_eq!(
                check_launch(caller, Some(&args(1))),
                Err(ArgsError::NotAllowedFrom),
                "{caller:?}"
            );
            assert_eq!(
                check_launch(caller, Some(&[])),
                Err(ArgsError::NotAllowedFrom),
                "even an empty list replaces the configured arguments ({caller:?})"
            );
            assert_eq!(check_launch(caller, None), Ok(()), "{caller:?}");
        }
    }

    #[test]
    fn too_many_arguments_are_refused() {
        assert_eq!(check_list(&args(MAX_ARGS)), Ok(()));
        assert_eq!(check_list(&args(MAX_ARGS + 1)), Err(ArgsError::TooMany));
        assert_eq!(
            check_launch("main", Some(&args(MAX_ARGS + 1))),
            Err(ArgsError::TooMany)
        );
    }

    #[test]
    fn a_long_argument_is_refused_by_characters_not_bytes() {
        let at_limit = vec!["x".repeat(MAX_ARG_CHARS)];
        assert_eq!(check_list(&at_limit), Ok(()));
        let over = vec!["x".repeat(MAX_ARG_CHARS + 1)];
        assert_eq!(check_list(&over), Err(ArgsError::TooLong));
        // 1024 two-byte characters are 2048 bytes but still 1024 characters.
        let wide = vec!["\u{e9}".repeat(MAX_ARG_CHARS)];
        assert_eq!(check_list(&wide), Ok(()));
    }

    #[test]
    fn a_nul_character_is_refused() {
        assert_eq!(check_list(&["a\0b".to_owned()]), Err(ArgsError::Nul));
    }

    #[test]
    fn the_messages_are_for_people() {
        assert!(ArgsError::TooMany.to_string().contains("16"));
        assert!(ArgsError::TooLong.to_string().contains("1024"));
    }
}
