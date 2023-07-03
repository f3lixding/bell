use lib_udp_server::{BellMessage, GameState, Point, PositionChangeMessage};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Program started");

    let game_state = GameState::new_with_capacity(1000);
    let game_state = Arc::new(RwLock::new(game_state));

    let rest_period_in_secs = 3;

    let game_state_clone = game_state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(rest_period_in_secs)).await;
            let is_empty = {
                let game_state = game_state_clone.read().await;
                game_state.is_empty()
            };

            if !is_empty {
                let mut game_state = game_state_clone.write().await;
                let messages = game_state.retrieve_messages();
                tokio::spawn(async move {
                    println!("{} valid message received and should be processed here", messages.len());
                });
            }
        }
    });

    let game_state_clone = game_state.clone();
    let socket = UdpSocket::bind("127.0.0.1:8080").await?;
    let mut buf = vec![0; 1024];

    // using RwLock for now
    while let Ok((size, src)) = socket.recv_from(&mut buf).await {
        {
            let sample_data = BellMessage::from(PositionChangeMessage { x: 0.0, y: 0.0 });
            let mut data = serde_json::to_vec(&sample_data).unwrap();
            data.push(b'\n');
            socket.send_to(&data, src).await?;
        }
        let mut data = vec![0; size];
        data.copy_from_slice(&buf[..size]);
        let data = serde_json::from_slice::<BellMessage>(&data);
        if data.is_ok() {
            let data = data.unwrap();
            let is_full = {
                let game_state = game_state_clone.read().await;
                game_state.is_full()
            };

            if !is_full {
                let mut game_state = game_state_clone.write().await;
                game_state.queue_message(data);
            } else {
                println!("Game is full"); // TODO: need to send a message to the client that the game is full
            }
        } else {
            println!("Message received isn't BellMessage");
        }
    }

    Ok(())
}
