use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Default, Debug, Clone)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub id: i32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum BellMessage {
    PositionChangeMessage(Point),
    DeferMessage,
    PlayerInsertionMessage(Point),
}

pub struct GameState {
    capacity: usize,
    process_queue: Vec<Option<BellMessage>>,
}
impl GameState {
    pub fn new_with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            process_queue: Vec::<Option<BellMessage>>::with_capacity(capacity),
        }
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
}
