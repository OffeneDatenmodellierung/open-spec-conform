//! The `conform` binary.
//!
//! Four lines on purpose: everything worth testing lives in the library, where
//! a test can drive a whole invocation in-process and read exactly what a user
//! would have seen. See [`conform_cli`] for what this actually does.

fn main() -> std::process::ExitCode {
    let code = conform_cli::execute(
        std::env::args_os(),
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    );
    // `ExitCode` rather than `process::exit`, so the locked standard streams
    // above are flushed and dropped normally on the way out.
    std::process::ExitCode::from(u8::try_from(code).unwrap_or(u8::MAX))
}
