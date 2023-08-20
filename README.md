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
actorlib = "0.1.2"
```

echo.rs

```rust
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
                        println!("Received Ping");
                        let mut state_lock = ctx.state.lock().await;
                        state_lock.counter += 1;
                        Ok(Response::Pong{counter: state_lock.counter})
                    }
                }

            })
        }, state, 100000).await
    }

}
```

main.rs

```rust
#[tokio::main]
async fn main() -> Result<(), BoxDynError> {
    let echo = Echo::new().await;

    println!("Sent Ping");
    echo.send(Message::Ping).await?;

    println!("Sent Ping and ask response");
    let pong = echo.ask(Message::Ping).await?;
    println!("Got Pong: {:?}", pong);

    println!("Sent Ping and wait response in callback");
    echo.callback(Message::Ping, move |result| {
        Box::pin(async move {
            let response = result?;
            if let Response::Pong { counter } = response {
                println!("Got Pong with counter: {}", counter);
            }
            Ok(())
        })
    }).await?;

    _ = echo.stop();
    thread::sleep(std::time::Duration::from_secs(1));
    Ok(())
}
```

Example output:

```text 
Sent Ping
Sent Ping and ask response
Got ping
Got ping
Got Pong: Pong { counter: 2 }
Sent Ping and wait response in callback
Got ping
Got Pong with counter: 3

```

Example sources: https://github.com/evgenyigumnov/actor-lib/tree/main/examples/ping_pong
