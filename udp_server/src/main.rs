use lib_udp_server::{BellMessage, GameState, Point};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Program started");

    let game_state = GameState::new_with_capacity(1000);
    let game_state = Arc::new(RwLock::new(game_state));

    let rest_period_in_secs = 3;

    let socket = UdpSocket::bind("127.0.0.1:8080").await?;
    let socket = Arc::new(socket);

    let socket_clone = socket.clone();
    let game_state_clone = game_state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(rest_period_in_secs)).await;
            let is_empty = {
                let game_state = game_state_clone.read().await;
                game_state.is_empty()
            };

            let game_state = game_state_clone.clone();
            let socket = socket_clone.clone();
            if !is_empty {
                tokio::spawn(async move {
                    // we probably don't need to store the positions at all times
                    // TODO: make a messaging system to update the positions
                    // instead of using a lock system
                    let mut game_state = game_state.write().await;
                    let messages = game_state.retrieve_messages();
                    // TODO: abstract this into its own function to allow for server side
                    // modifications (e.g. collision)
                    let out_going_messages = messages
                        .iter()
                        .map(|message| match message {
                            BellMessage::PositionChangeMessage(point) => {
                                let audiences = game_state.get_addrs_for_id(point.id);
                                audiences
                                    .iter()
                                    .map(|addr| {
                                        (*addr, BellMessage::PositionChangeMessage(point.clone()))
                                    })
                                    .collect::<Vec<(&std::net::SocketAddr, BellMessage)>>()
                            }
                            BellMessage::PlayerInsertionMessage(point) => {
                                let audiences = game_state.get_addrs_for_id(point.id);
                                audiences
                                    .iter()
                                    .map(|addr| {
                                        (*addr, BellMessage::PlayerInsertionMessage(point.clone()))
                                    })
                                    .collect::<Vec<(&std::net::SocketAddr, BellMessage)>>()
                            }
                            _ => vec![],
                        })
                        .flatten()
                        .collect::<Vec<(&std::net::SocketAddr, BellMessage)>>();

                    for (addr, message) in out_going_messages {
                        let data = serde_json::to_vec(&message).unwrap();
                        _ = socket.send_to(&data, *addr).await;
                    }
                });
            }
        }
    });

    let game_state_clone = game_state.clone();
    let mut buf = vec![0; 1024];

    // using RwLock for now
    while let Ok((size, src)) = socket.recv_from(&mut buf).await {
        {
            let sample_data = BellMessage::PositionChangeMessage(Point {
                x: 0.,
                y: 0.,
                id: 0,
            });
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
                if let BellMessage::PlayerInsertionMessage(ref point) = data {
                    game_state.insert_player(point.id, point.x, point.y, src);
                }
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
