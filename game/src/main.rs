use bevy::prelude::*;
use lib_udp_server::{BellMessage, Point};
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

#[derive(Resource, Clone, Default)]
struct SpritePosition {
    pub x: Arc<RwLock<f32>>,
    pub y: Arc<RwLock<f32>>,
    pub has_extern_changes: Arc<RwLock<bool>>,
}

// TODO: Use this instead of a single SpritePosition
#[derive(Resource)]
struct SpritePositions(Arc<RwLock<Vec<SpritePosition>>>);

#[derive(Resource)]
struct MessageSender {
    tx: Mutex<std::sync::mpsc::Sender<UpdateMessage>>,
}

#[derive(Clone)]
enum UpdateMessage {
    PositionChange,
    PositionChangeExtern(lib_udp_server::Point),
}

fn main() {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let sprite_position = SpritePosition::default();
    let sprite_position2 = sprite_position.clone();

    let (tx, rx) = channel::<UpdateMessage>();

    let socket_clone = socket.try_clone().unwrap();
    // Internal listening thread
    thread::spawn(move || {
        while let Ok(message) = rx.recv() {
            match message {
                UpdateMessage::PositionChange => {
                    let SpritePosition { ref x, ref y, .. } = sprite_position;
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
                    let SpritePosition {
                        ref x,
                        ref y,
                        ref has_extern_changes,
                    } = sprite_position;

                    {
                        let mut x = x.write().unwrap();
                        let mut y = y.write().unwrap();
                        let mut has_extern_changes = has_extern_changes.write().unwrap();
                        *x = point.x;
                        *y = point.y;
                        *has_extern_changes = true;
                    }
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
        .insert_resource(sprite_position2)
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
    ));
}

/// The sprite is animated by changing its translation depending on the time that has passed since
/// the last frame.
fn sprite_movement(
    time: Res<Time>,
    mut sprite_position: Query<(&mut Direction, &mut Transform)>,
    mut position_record: ResMut<SpritePosition>,
    keyboard_input: Res<Input<KeyCode>>,
    message_sender: Res<MessageSender>,
) {
    for (mut logo, mut transform) in &mut sprite_position {
        let has_extern_changes = {
            let guard = position_record.has_extern_changes.read().unwrap();
            *guard
        };

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
                transform.translation.x = position_record.x.read().unwrap().clone();
                transform.translation.y = position_record.y.read().unwrap().clone();
                let mut has_extern_changes = position_record.has_extern_changes.write().unwrap();
                *has_extern_changes = false;
                *logo = Direction::Still; // reset the direction here otherwise the sprite will
                                          // keep moving
                return;
            }
        }

        if keyboard_input.pressed(KeyCode::Up) {
            *logo = Direction::Up;
            update_server(
                transform.translation.x,
                transform.translation.y,
                &mut position_record,
                &message_sender,
            );
        }
        if keyboard_input.pressed(KeyCode::Down) {
            *logo = Direction::Down;
            update_server(
                transform.translation.x,
                transform.translation.y,
                &mut position_record,
                &message_sender,
            );
        }
        if keyboard_input.pressed(KeyCode::Left) {
            *logo = Direction::Left;
            update_server(
                transform.translation.x,
                transform.translation.y,
                &mut position_record,
                &message_sender,
            );
        }
        if keyboard_input.pressed(KeyCode::Right) {
            *logo = Direction::Right;
            update_server(
                transform.translation.x,
                transform.translation.y,
                &mut position_record,
                &message_sender,
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
    position_record: &mut ResMut<SpritePosition>,
    message_sender: &Res<MessageSender>,
) {
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
