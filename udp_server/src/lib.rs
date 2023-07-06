use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Default, Debug, Clone)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub id: u32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum BellMessage {
    PositionChangeMessage(Point),
    DeferMessage,
    PlayerInsertionMessage(Point),
    PlayerRegistrationMessage(Point),
}

pub struct GameState {
    capacity: usize,
    process_queue: Vec<Option<BellMessage>>,
    _positions: std::collections::HashMap<u32, (f32, f32)>,
    addrs: std::collections::HashMap<u32, std::net::SocketAddr>,
}
impl GameState {
    pub fn new_with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            process_queue: Vec::<Option<BellMessage>>::with_capacity(capacity),
            _positions: std::collections::HashMap::<u32, (f32, f32)>::with_capacity(2),
            addrs: std::collections::HashMap::<u32, std::net::SocketAddr>::with_capacity(2),
        }
    }

    pub fn insert_player(&mut self, id: u32, x: f32, y: f32, addr: std::net::SocketAddr) {
        self.addrs.insert(id, addr);
        self._positions.insert(id, (x, y));
    }
    pub fn is_full(&self) -> bool {
        self.process_queue.len() >= self.capacity
    }

    pub fn is_empty(&self) -> bool {
        self.process_queue.is_empty()
    }

    pub fn queue_message(&mut self, message: BellMessage) {
        self.process_queue.push(Some(message));
    }

    pub fn retrieve_messages(&mut self) -> Vec<BellMessage> {
        let mut messages = Vec::<BellMessage>::with_capacity(self.process_queue.len());
        while !self.process_queue.is_empty() {
            messages.push(self.process_queue.pop().unwrap().unwrap());
        }

        messages
    }

    pub fn get_collided_pairs(&self) -> Vec<(u32, u32)> {
        vec![]
    }

    pub fn get_addr_from_id(&self, id: u32) -> Option<&std::net::SocketAddr> {
        self.addrs.get(&id)
    }

    pub fn get_addrs_for_id(&self, id: u32) -> Vec<&std::net::SocketAddr> {
        let mut res_addrs = Vec::with_capacity(3);
        println!("addrs: {:?}", self.addrs);
        for (k, v) in self.addrs.iter() {
            if k != &id {
                res_addrs.push(v);
            }
        }

        res_addrs
    }
}
