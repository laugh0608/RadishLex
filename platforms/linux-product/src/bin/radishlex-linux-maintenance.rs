#[cfg(target_os = "linux")]
fn main() {
    use radishlex_linux_product_install::{
        run_linux_maintenance, LinuxMaintenanceCommand, TransactionOutcome,
    };

    let arguments = match std::env::args_os()
        .skip(1)
        .map(|value| value.into_string())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(arguments) => arguments,
        Err(_) => {
            eprintln!("maintenance_error=ArgumentInvalid");
            std::process::exit(1);
        }
    };
    let result = LinuxMaintenanceCommand::parse(&arguments).and_then(run_linux_maintenance);
    match result {
        Ok(outcome) => {
            let value = match outcome {
                TransactionOutcome::Completed => "completed",
                TransactionOutcome::AbortedPreserved => "aborted_preserved",
                TransactionOutcome::RolledBack => "rolled_back",
            };
            println!("maintenance_outcome={value}");
        }
        Err(error) => {
            eprintln!("maintenance_error={:?}", error.code());
            if let Some(failure) = error.failure_code() {
                eprintln!("maintenance_failure={failure:?}");
            }
            std::process::exit(1);
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("maintenance_error=EnvironmentUnsupported");
    std::process::exit(1);
}
