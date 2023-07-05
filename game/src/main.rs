use bevy::prelude::*;
use lib_udp_server::{BellMessage, Point};
use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::atomic::Ordering;
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
struct SpritePositions(
    Arc<RwLock<HashMap<u32, SpritePosition>>>,
    Arc<RwLock<Option<u32>>>,
);

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
    let sprite_position = SpritePosition::default();
    let sprite_collections = {
        let mut map = HashMap::new();
        // TODO: replace this with a proper initialization routine
        map.insert(0, sprite_position);

        let position_collections = Arc::new(RwLock::new(map));
        let injection_order = Arc::new(RwLock::new(None));
        SpritePositions(position_collections, injection_order)
    };

    let (tx, rx) = channel::<UpdateMessage>();

    let sprite_collections_clone = sprite_collections.clone();
    let socket_clone = socket.try_clone().unwrap();
    // Internal listening thread
    thread::spawn(move || {
        while let Ok(message) = rx.recv() {
            match message {
                UpdateMessage::PositionChange => {
                    let target_sprite = sprite_collections_clone.0.read().unwrap();
                    let target_sprite = target_sprite.get(&0).unwrap();
                    let SpritePosition { ref x, ref y, .. } = target_sprite;
                    let (x, y) = {
                        let x = x.read().unwrap();
                        let y = y.read().unwrap();
                        (*x, *y)
                    };
                    let message = BellMessage::PositionChangeMessage(Point { x, y, id: 0 });
                    let message = serde_json::to_vec(&message).unwrap();
                    socket_clone.send_to(&message, "127.0.0.1:8080").unwrap();
                }
                UpdateMessage::PositionChangeExtern(point) => {
                    let target_sprite = sprite_collections_clone.0.read().unwrap();
                    let target_sprite = target_sprite.get(&0).unwrap();
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
                UpdateMessage::PlayerInsertion(point) => {
                    let mut player_registry = sprite_collections_clone.0.write().unwrap();
                    let sprite_position = SpritePosition {
                        x: Arc::new(RwLock::new(point.x)),
                        y: Arc::new(RwLock::new(point.y)),
                        has_extern_changes: Arc::new(RwLock::new(false)),
                    };

                    player_registry.insert(point.id as u32, sprite_position);
                    sprite_collections_clone
                        .1
                        .write()
                        .unwrap()
                        .replace(point.id as u32);
                }
            }
        }
    });

    // External listening thread
    let tx_clone = tx.clone();
    thread::spawn(move || {
        let socket = socket.try_clone().unwrap();
        let mut buf = vec![0; 1024];
        while let Ok((size, _src)) = socket.recv_from(&mut buf) {
            println!("message received");
            let data = &buf[..size];
            let message = serde_json::from_slice::<BellMessage>(&data);
            if let Ok(message) = message {
                match message {
                    BellMessage::PositionChangeMessage(point) => {
                        tx_clone
                            .send(UpdateMessage::PositionChangeExtern(point))
                            .unwrap();
                    }
                    BellMessage::PlayerInsertionMessage(point) => {
                        tx_clone
                            .send(UpdateMessage::PositionChangeExtern(point))
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
        .add_plugin(LogDiagnosticsPlugin::default())
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_startup_system(setup)
        .insert_resource(sprite_collections)
        .insert_resource(message_sender)
        .add_system(sprite_movement)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2dBundle::default());
    commands.spawn((
        SpriteBundle {
            texture: asset_server.load("icon.png"),
            transform: Transform::from_xyz(0., 0., 0.),
            ..default()
        },
        Direction::Still,
        PlayerId(0),
    ));
}

fn maybe_insert_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    sprite_positions: ResMut<SpritePositions>,
) {
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
    for (mut logo, mut transform, player_id) in &mut sprite_position {
        let has_extern_changes = {
            let guard = position_record
                .0
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

        if player_id.0 != 0 && has_extern_changes {
            let position_record = position_record.0.read().unwrap();
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
                let position_record = position_record.0.read().unwrap();
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
    let position_record = position_record.0.read().unwrap();
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
