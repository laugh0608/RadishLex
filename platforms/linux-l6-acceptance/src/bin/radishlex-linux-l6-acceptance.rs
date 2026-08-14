#[cfg(target_os = "linux")]
fn main() {
    use radishlex_linux_l6_acceptance::{
        run_checkpoint_controller, run_checkpoint_worker, write_system_checkpoint_evidence,
        ControllerCommand, LinuxCheckpointProcessBackend, WorkerCommand,
    };

    let arguments = match std::env::args_os()
        .skip(1)
        .map(|value| value.into_string())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(arguments) => arguments,
        Err(_) => {
            eprintln!("l6_acceptance_error=ArgumentInvalid");
            std::process::exit(1);
        }
    };
    let result = match arguments.first().map(String::as_str) {
        Some("crash") => ControllerCommand::parse(&arguments)
            .map_err(|_| "CommandInvalid")
            .and_then(|command| {
                let mut backend = LinuxCheckpointProcessBackend;
                let evidence = run_checkpoint_controller(&command, &mut backend)
                    .map_err(|_| "ControllerFailed")?;
                write_system_checkpoint_evidence(&evidence).map_err(|_| "EvidenceFailed")?;
                Ok("checkpoint_recorded")
            }),
        Some("__checkpoint_worker") => WorkerCommand::parse(&arguments)
            .map_err(|_| "CommandInvalid")
            .and_then(|command| {
                run_checkpoint_worker(command).map_err(|_| "WorkerFailed")?;
                Ok("unreachable")
            }),
        _ => Err("CommandInvalid"),
    };
    match result {
        Ok(outcome) => println!("l6_acceptance_outcome={outcome}"),
        Err(error) => {
            eprintln!("l6_acceptance_error={error}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("l6_acceptance_error=EnvironmentUnsupported");
    std::process::exit(1);
}
