use crate::controller::{ControllerCommands, ControllerInterface};
use crate::types::Premint;
use tokio::io::AsyncBufReadExt;
use tokio::{io, select};

/// Blocking loop forever to watch stdin for commands to the controller.
pub async fn watch_stdin(ctl: ControllerInterface) {
    let mut stdin = io::BufReader::new(io::stdin()).lines();

    loop {
        select! {
            Ok(Some(line)) = stdin.next_line() => {
               process_stdin_line(ctl.clone(), line).await;
            }
        }
    }
}

async fn process_stdin_line(ctl: ControllerInterface, line: String) {
    if line.is_empty() {
        return;
    }
    if line.starts_with("/ip4") {
        if let Err(err) = ctl
            .send_command(ControllerCommands::ConnectToPeer { address: line })
            .await
        {
            tracing::error!(
                error = err.to_string(),
                "Error sending connect to peer command"
            );
        };
    } else if line.starts_with("/peers") {
        // self.swarm_command_sender.send(SwarmCommand::ReturnNetworkState).await.unwrap();
        if let Err(err) = ctl
            .send_command(ControllerCommands::ReturnNetworkState)
            .await
        {
            tracing::error!(
                error = err.to_string(),
                "Error sending return network state command"
            );
        };
    } else if line.starts_with("/announce") {
        // self.swarm_command_sender.send(SwarmCommand::AnnounceSelf).await.unwrap();
        if let Err(err) = ctl.send_command(ControllerCommands::AnnounceSelf).await {
            tracing::error!(
                error = err.to_string(),
                "Error sending announce self command"
            );
        };
    } else {
        match Premint::from_json(line) {
            Ok(premint) => {
                if let Err(err) = ctl
                    .send_command(ControllerCommands::Broadcast { message: premint })
                    .await
                {
                    tracing::error!(error = err.to_string(), "Error sending broadcast command");
                };
            }

            Err(e) => {
                tracing::warn!("Error parsing premint: {:?}", e);
            }
        }
    }
}
