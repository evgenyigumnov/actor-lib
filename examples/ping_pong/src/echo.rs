use std::sync::Arc;
use actorlib::*;

pub struct Echo;

#[derive(Debug)]
pub enum Message {
    Ping,
}

#[derive(Debug)]
pub enum Response {
    Pong {counter: u32},
}

#[derive(Debug,Clone)]
pub struct State {
    pub counter: u32,
}

impl Echo {
    pub async fn new() -> Arc<Actor<Message, State, Response>> {

        let state = State {
            counter: 0,
        };

        Actor::new("echo".to_string(), move |ctx| {
            Box::pin(async move {
                match ctx.mgs {
                    Message::Ping => {
                        println!("Got pong");
                        let mut state_lock = ctx.state.lock().await;
                        state_lock.counter += 1;
                        Ok(Response::Pong{counter: state_lock.counter})
                    }
                }

          })
        }, state, 100000).await
    }

}