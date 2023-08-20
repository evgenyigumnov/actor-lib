use std::thread;
use tokio::runtime::Runtime;
use actorlib::*;
use crate::echo::*;

mod echo;

#[tokio::main]
async fn main() -> Result<(), BoxDynError> {
    let echo = Echo::new().await;

    println!("Sent Ping");
    echo.send(Message::Ping).await;

    println!("Sent Ping and ask response");
    let pong = echo.ask(Message::Ping).await?;
    println!("Got Pong: {:?}", pong);

    println!("Sent Ping and wait response in callback");
    echo.callback(Message::Ping, move |result| {
        Box::pin(async move {
            let respone = result?;
            if let Response::Pong { counter } = respone {
                println!("Got Pong with counter: {}", counter);
            }
            Ok(())
        })
    }).await;

    echo.stop();
    thread::sleep(std::time::Duration::from_secs(1));
    Ok(())
}