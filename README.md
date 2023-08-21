# Actorlib

[![Project license](https://img.shields.io/github/license/evgenyigumnov/actor-lib.svg?style=flat-square)](LICENSE)
[![Pull Requests welcome](https://img.shields.io/badge/PRs-welcome-ff69b4.svg?style=flat-square)](https://github.com/evgenyigumnov/actor-lib/issues?q=is%3Aissue+is%3Aopen+label%3A%22help+wanted%22)
[![Rust crate](https://img.shields.io/crates/v/actorlib.svg)](https://crates.io/crates/actorlib)


Indeed, an actor library, not a framework, written in Rust

## Features

- [x] Async for sending messages
- [x] Async for messages processing in actor
- [x] Support messaging like send and forget 
- [x] Support messaging like send and wait response
- [x] Mutable state of actor
- [x] Self reference in actor from context

## Usage

Cargo.toml

```toml
[dependencies]
actorlib = "1.0.0"
```

echo.rs

```rust
#[derive(Debug)]
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

#[derive(Debug, Error)]
pub enum EchoError {
    #[error("unknown error")]
    Unknown,
    #[error("std::io::Error")]
    StdErr(#[from] std::io::Error),
}

#[async_trait]
impl Handler<Echo, Message, State, Response, EchoError> for Echo {
    async fn receive(&self, ctx: Context<Echo, Message, State, Response, EchoError>) -> Result<Response, EchoError> {
        match ctx.mgs {
            Message::Ping => {
                println!("Received Ping");
                let mut state_lock = ctx.state.lock().await;
                state_lock.counter += 1;
                if state_lock.counter > 10 {
                    Err(EchoError::Unknown)
                } else {
                    Ok(Response::Pong{counter: state_lock.counter})
                }
            }
        }
    }
}
```

main.rs

```rust
#[tokio::main]
async fn main() -> Result<(), EchoError> {
    let state = State {
        counter: 0,
    };

    let echo_ref = ActorRef::new("echo".to_string(), Echo{},  state, 100000).await;

    println!("Sent Ping");
    echo_ref.send(Message::Ping).await?;

    println!("Sent Ping and ask response");
    let pong = echo_ref.ask(Message::Ping).await?;
    println!("Got {:?}", pong);

    _ = echo_ref.stop();
    thread::sleep(std::time::Duration::from_secs(1));
    Ok(())
}
```

Example output:

```text 
Sent Ping
Sent Ping and ask response
Received Ping
Received Ping
Got Pong { counter: 2 }
```

Example sources: https://github.com/evgenyigumnov/actor-lib/tree/main/test
