use colored::Colorize;

use mintpool_controller::controller::{ControllerCommands, ControllerInterface};
use mintpool_primitives::premint::Premint;
use tokio::io::AsyncBufReadExt;
use tokio::{io, select};

const PROMPT: &str = r#"🌍 Mintpool node accepting commands 🌍
Supported commands:
    /ip4/<some node_uri> - ex: /ip4/127.0.0.1/tcp/7779/p2p/12D3..e2Xo
    /peers - list all connected peers
    /node - shows node info
    /announce - announce self to the network to connect to all available peers
    <premint json> - send a premint to the network
"#;

const LINE_PROMPT: &str = "🌍 ENTER COMMAND 🌍";

/// Blocking loop forever to watch stdin for commands to the controller.
pub async fn watch_stdin(ctl: ControllerInterface) {
    let mut stdin = io::BufReader::new(io::stdin()).lines();
    println!("{}", PROMPT.blue());
    println!("{}", LINE_PROMPT.on_bright_purple().black().bold());

    loop {
        select! {
            Ok(Some(line)) = stdin.next_line() => {
               process_stdin_line(ctl.clone(), line).await;
                println!("{}", LINE_PROMPT.on_bright_purple().bold());
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
        let (snd, recv) = tokio::sync::oneshot::channel();
        match ctl
            .send_command(ControllerCommands::ReturnNetworkState { channel: snd })
            .await
        {
            Ok(()) => {
                if let Ok(state) = recv.await {
                    tracing::info!("Network state: {:?}", state);
                }
            }
            Err(err) => {
                tracing::error!(
                    error = err.to_string(),
                    "Error sending return network state command"
                );
            }
        };
    } else if line.starts_with("/node") {
        let (snd, recv) = tokio::sync::oneshot::channel();
        match ctl
            .send_command(ControllerCommands::ReturnNodeInfo { channel: snd })
            .await
        {
            Ok(()) => {
                if let Ok(info) = recv.await {
                    tracing::info!("Node info: {:?}", info);
                }
            }
            Err(err) => {
                tracing::error!(
                    error = err.to_string(),
                    "Error sending return node info command"
                );
            }
        };
    } else if line.starts_with("/announce") {
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
