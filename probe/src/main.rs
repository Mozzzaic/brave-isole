mod checks;
fn main() -> std::process::ExitCode {
    match checks::run(std::env::args().skip(1).collect(), &[]) {
        Ok(code) => code,
        Err(_) => {
            eprintln!("invalid or unavailable synthetic probe");
            std::process::ExitCode::FAILURE
        }
    }
}
