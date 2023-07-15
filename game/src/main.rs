use bevy::prelude::*;
use lib_udp_server::{BellMessage, Point};
use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::thread;

use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};

mod packet_sender;

#[derive(Component)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
    Still,
}

#[derive(Component)]
struct PlayerId(u32);

#[derive(Resource, Clone, Default)]
struct SpritePosition {
    pub x: Arc<RwLock<f32>>,
    pub y: Arc<RwLock<f32>>,
    pub has_extern_changes: Arc<RwLock<bool>>,
}

// TODO: Use this instead of a single SpritePosition
#[derive(Resource, Clone, Default)]
struct SpritePositions {
    pub position_collections: Arc<RwLock<HashMap<u32, SpritePosition>>>,
    pub injection_order: Arc<RwLock<Option<u32>>>,
    pub self_id: Arc<RwLock<u32>>,
}

#[derive(Resource)]
struct MessageSender {
    tx: Mutex<std::sync::mpsc::Sender<UpdateMessage>>,
}

#[derive(Clone)]
enum UpdateMessage {
    PositionChange,
    PositionChangeExtern(lib_udp_server::Point),
    PlayerInsertion(lib_udp_server::Point),
}

fn main() {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let sprite_collections = {
        let map = HashMap::new();
        // TODO: replace this with a proper initialization routine

        let position_collections = Arc::new(RwLock::new(map));
        let injection_order = Arc::new(RwLock::new(None));
        SpritePositions {
            position_collections,
            injection_order,
            self_id: Arc::new(RwLock::new(0)),
        }
    };

    let (tx, rx) = channel::<UpdateMessage>();

    let sprite_collections_clone = sprite_collections.clone();
    let socket_clone = socket.try_clone().unwrap();
    // Internal listening thread
    thread::spawn(move || {
        while let Ok(message) = rx.recv() {
            let self_id = sprite_collections_clone.self_id.read().unwrap();
            match message {
                UpdateMessage::PositionChange => {
                    let target_sprite = sprite_collections_clone
                        .position_collections
                        .read()
                        .unwrap();
                    let target_sprite = target_sprite.get(&self_id).unwrap();
                    let SpritePosition { ref x, ref y, .. } = target_sprite;
                    let (x, y) = {
                        let x = x.read().unwrap();
                        let y = y.read().unwrap();
                        (*x, *y)
                    };
                    let message = BellMessage::PositionChangeMessage(Point { x, y, id: *self_id });
                    let message = serde_json::to_vec(&message).unwrap();
                    socket_clone.send_to(&message, "127.0.0.1:8080").unwrap();
                }
                UpdateMessage::PositionChangeExtern(point) => {
                    let mut target_sprite_collections = sprite_collections_clone
                        .position_collections
                        .write()
                        .unwrap();
                    let target_sprite = target_sprite_collections.get(&(point.id as u32));
                    match target_sprite {
                        Some(target_sprite) => {
                            let SpritePosition {
                                ref x,
                                ref y,
                                ref has_extern_changes,
                            } = target_sprite;
                            {
                                let mut x = x.write().unwrap();
                                let mut y = y.write().unwrap();
                                let mut has_extern_changes = has_extern_changes.write().unwrap();
                                *x = point.x;
                                *y = point.y;
                                *has_extern_changes = true;
                            }
                        }
                        None => {
                            let sprite_position = SpritePosition {
                                x: Arc::new(RwLock::new(point.x)),
                                y: Arc::new(RwLock::new(point.y)),
                                has_extern_changes: Arc::new(RwLock::new(true)),
                            };
                            target_sprite_collections.insert(point.id as u32, sprite_position);
                        }
                    }
                }
                UpdateMessage::PlayerInsertion(point) => {
                    println!("Inserting player {}", point.id);
                    let mut player_registry = sprite_collections_clone
                        .position_collections
                        .write()
                        .unwrap();
                    println!("Registry obtained");
                    let sprite_position = SpritePosition {
                        x: Arc::new(RwLock::new(point.x)),
                        y: Arc::new(RwLock::new(point.y)),
                        has_extern_changes: Arc::new(RwLock::new(false)),
                    };

                    player_registry.insert(point.id as u32, sprite_position);
                    sprite_collections_clone
                        .injection_order
                        .write()
                        .unwrap()
                        .replace(point.id as u32);
                    println!("Injection order updated");
                }
            }
        }
    });

    // fetch id as assigned by the server
    let registration_message = BellMessage::PlayerRegistrationMessage(Point {
        x: 0.,
        y: 0.,
        id: 0,
    });
    let registration_message = serde_json::to_vec(&registration_message).unwrap();
    socket
        .send_to(&registration_message, "127.0.0.1:8080")
        .unwrap();

    let mut buf = vec![0; 1024];
    let id: u32;
    if let Ok((size, _src)) = socket.recv_from(&mut buf) {
        if size > 4 {
            panic!("Received registration data but failed to parse it");
        }
        id = u32::from_be_bytes(buf[0..4].try_into().unwrap());
        println!("Id received: {}", id);
    } else {
        panic!("Received registration data but failed to parse it");
    }

    *sprite_collections.self_id.write().unwrap() = id;
    sprite_collections
        .position_collections
        .write()
        .unwrap()
        .insert(
            id,
            SpritePosition {
                x: Arc::new(RwLock::new(0.)),
                y: Arc::new(RwLock::new(0.)),
                has_extern_changes: Arc::new(RwLock::new(false)),
            },
        );

    // External listening thread
    let tx_clone = tx.clone();
    let socket_clone = socket.try_clone().unwrap();
    thread::spawn(move || {
        let mut buf = vec![0; 1024];
        while let Ok((size, _src)) = socket_clone.recv_from(&mut buf) {
            let data = &buf[..size];
            let message = serde_json::from_slice::<BellMessage>(&data);
            if let Ok(message) = message {
                match message {
                    BellMessage::PositionChangeMessage(point) => {
                        let send_result = tx_clone.send(UpdateMessage::PositionChangeExtern(point));
                        if let Err(e) = send_result {
                            println!("Error sending message: {:?}", e);
                        }
                    }
                    BellMessage::PlayerInsertionMessage(point) => {
                        println!("Player insertion message received");
                        tx_clone
                            .send(UpdateMessage::PlayerInsertion(point))
                            .unwrap();
                    }
                    _ => {}
                }
            }
        }
    });

    let message_sender = {
        let sender_lock = Mutex::new(tx);
        MessageSender { tx: sender_lock }
    };

    App::new()
        .add_plugins(DefaultPlugins)
        // .add_plugin(LogDiagnosticsPlugin::default())
        // .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .insert_resource(sprite_collections)
        .insert_resource(message_sender)
        .add_system(sprite_movement)
        .add_system(maybe_insert_player)
        .add_startup_system(setup)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    sprite_collections: ResMut<SpritePositions>,
) {
    let id = sprite_collections.self_id.read().unwrap();
    commands.spawn(Camera2dBundle::default());
    commands.spawn((
        SpriteBundle {
            texture: asset_server.load("icon.png"),
            transform: Transform::from_xyz(0., 0., 0.),
            ..default()
        },
        Direction::Still,
        PlayerId(*id),
    ));
}

fn maybe_insert_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    sprite_positions: ResMut<SpritePositions>,
) {
    let mut insertion_order = sprite_positions.injection_order.write().unwrap();
    if insertion_order.is_some() {
        println!("Injection order found");
        let insert_id = insertion_order.take();
        if let Some(insert_id) = insert_id {
            let target_position = sprite_positions.position_collections.read().unwrap();
            println!("Read copy of target position map obtained");
            let target_position = target_position.get(&insert_id);
            if target_position.is_none() {
                println!("Target position not found for injection order");
                return;
            }
            let target_position = target_position.unwrap();
            println!("Target position found");
            commands.spawn((
                SpriteBundle {
                    texture: asset_server.load("icon.png"),
                    transform: Transform::from_xyz(
                        *target_position.x.read().unwrap(),
                        *target_position.y.read().unwrap(),
                        0.,
                    ),
                    ..default()
                },
                Direction::Still,
                PlayerId(insert_id),
            ));

            println!("Sprite bundle spawned");
        }
    }
    // if let Some(target_id) = sprite_positions.1.read().unwrap() {

    // }
}

/// The sprite is animated by changing its translation depending on the time that has passed since
/// the last frame.
fn sprite_movement(
    time: Res<Time>,
    mut sprite_position: Query<(&mut Direction, &mut Transform, &PlayerId)>,
    mut position_record: ResMut<SpritePositions>,
    keyboard_input: Res<Input<KeyCode>>,
    message_sender: Res<MessageSender>,
) {
    let self_id = { *position_record.self_id.read().unwrap() };

    for (mut logo, mut transform, player_id) in &mut sprite_position {
        let has_extern_changes = {
            let guard = position_record
                .position_collections
                .read()
                .unwrap()
                .get(&player_id.0)
                .unwrap()
                .has_extern_changes
                .read()
                .unwrap()
                .clone();
            guard
        };

        if has_extern_changes || self_id != player_id.0 {
            let position_record = position_record.position_collections.read().unwrap();
            let position_record = position_record.get(&player_id.0).unwrap();
            transform.translation.x = position_record.x.read().unwrap().clone();
            transform.translation.y = position_record.y.read().unwrap().clone();
            let mut has_extern_changes = position_record.has_extern_changes.write().unwrap();
            *has_extern_changes = false;
            *logo = Direction::Still; // reset the direction here otherwise the sprite will
                                      // keep moving
            continue;
        }

        match *logo {
            Direction::Up if !has_extern_changes => {
                transform.translation.y += 150. * time.delta_seconds()
            }
            Direction::Down if !has_extern_changes => {
                transform.translation.y -= 150. * time.delta_seconds()
            }
            Direction::Left if !has_extern_changes => {
                transform.translation.x -= 150. * time.delta_seconds()
            }
            Direction::Right if !has_extern_changes => {
                transform.translation.x += 150. * time.delta_seconds()
            }
            Direction::Still if !has_extern_changes => {}
            _ => {
                // we have external commands to carry out from the update_server
                let position_record = position_record.position_collections.read().unwrap();
                let position_record = position_record.get(&player_id.0).unwrap();
                transform.translation.x = position_record.x.read().unwrap().clone();
                transform.translation.y = position_record.y.read().unwrap().clone();
                let mut has_extern_changes = position_record.has_extern_changes.write().unwrap();
                *has_extern_changes = false;
                *logo = Direction::Still; // reset the direction here otherwise the sprite will
                                          // keep moving
                continue;
            }
        }

        if keyboard_input.pressed(KeyCode::Up) {
            *logo = Direction::Up;
            update_server(
                transform.translation.x,
                transform.translation.y,
                &mut position_record,
                &message_sender,
                &player_id,
            );
        }
        if keyboard_input.pressed(KeyCode::Down) {
            *logo = Direction::Down;
            update_server(
                transform.translation.x,
                transform.translation.y,
                &mut position_record,
                &message_sender,
                &player_id,
            );
        }
        if keyboard_input.pressed(KeyCode::Left) {
            *logo = Direction::Left;
            update_server(
                transform.translation.x,
                transform.translation.y,
                &mut position_record,
                &message_sender,
                &player_id,
            );
        }
        if keyboard_input.pressed(KeyCode::Right) {
            *logo = Direction::Right;
            update_server(
                transform.translation.x,
                transform.translation.y,
                &mut position_record,
                &message_sender,
                &player_id,
            );
        }
        if keyboard_input.just_released(KeyCode::Up)
            || keyboard_input.just_released(KeyCode::Down)
            || keyboard_input.just_released(KeyCode::Left)
            || keyboard_input.just_released(KeyCode::Right)
        {
            *logo = Direction::Still;
        }
    }
}

fn update_server(
    x: f32,
    y: f32,
    position_record: &mut ResMut<SpritePositions>,
    message_sender: &Res<MessageSender>,
    player_id: &PlayerId,
) {
    let position_record = position_record.position_collections.read().unwrap();
    let position_record = position_record.get(&player_id.0).unwrap();
    let mut x_ = position_record.x.write().unwrap();
    let mut y_ = position_record.y.write().unwrap();
    *x_ = x;
    *y_ = y;

    let _ = message_sender
        .tx
        .lock()
        .unwrap()
        .send(UpdateMessage::PositionChange);
}
