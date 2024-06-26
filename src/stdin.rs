use crate::controller::{ControllerCommands, ControllerInterface, DBQuery};
use crate::types::PremintTypes;
use colored::Colorize;
use tokio::io::AsyncBufReadExt;
use tokio::{io, select};

const PROMPT: &str = r#"🌍 Mintpool node accepting commands 🌍
Supported commands:
    /connect <multiaddr> - ex: /connect /dnsaddr/mintpool-1.zora.co
    /ip4/<some node_uri> - ex: /ip4/127.0.0.1/tcp/7779/p2p/12D3..e2Xo
    /peers - list all connected peers
    /sync - sync from a random peer
    /node - shows node info
    /announce - announce self to the network to connect to all available peers
    /list - list all premints in the database
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
    if line.starts_with("/connect ") {
        if let Err(err) = ctl
            .send_command(ControllerCommands::ConnectToPeer {
                address: line[9..].parse().unwrap(),
            })
            .await
        {
            tracing::error!(
                error = err.to_string(),
                "Error sending connect to peer command"
            );
        };
    } else if line.starts_with("/sync") {
        if let Err(err) = ctl.send_command(ControllerCommands::Sync).await {
            tracing::error!(error = err.to_string(), "Error sending sync command");
        };
    } else if line.starts_with("/ip4") {
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
    } else if line.starts_with("/list") {
        let (snd, recv) = tokio::sync::oneshot::channel();
        match ctl
            .send_command(ControllerCommands::Query(DBQuery::ListAll(snd)))
            .await
        {
            Ok(()) => {
                if let Ok(Ok(premints)) = recv.await {
                    println!("Available premints:");
                    premints.iter().for_each(|p| println!("{:?}", p));
                } else {
                    tracing::error!("Error getting list all premints response");
                }
            }
            Err(err) => {
                tracing::error!(error = err.to_string(), "Error sending list all command");
            }
        };
    } else {
        match PremintTypes::from_json(line) {
            Ok(premint) => {
                let (snd, recv) = tokio::sync::oneshot::channel();
                if let Err(err) = ctl
                    .send_command(ControllerCommands::Broadcast {
                        message: premint,
                        channel: snd,
                    })
                    .await
                {
                    tracing::error!(error = err.to_string(), "Error sending broadcast command");
                };
                match recv.await {
                    Ok(Ok(())) => {
                        tracing::info!("Premint broadcasted successfully");
                    }
                    Ok(Err(e)) => {
                        tracing::warn!("Error broadcasting premint: {:?}", e);
                    }
                    Err(e) => {
                        tracing::error!("Error broadcasting premint: {:?}", e);
                    }
                }
            }

            Err(e) => {
                tracing::warn!("Error parsing premint: {:?}", e);
            }
        }
    }
}
