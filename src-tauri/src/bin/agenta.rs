use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    ExitCode::from(agenta_lib::interface::cli::run().await as u8)
}
