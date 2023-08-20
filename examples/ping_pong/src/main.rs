use actorlib::*;

#[tokio::main]
async fn main() -> Result<(), BoxDynError> {

    println!("Ctrl-C to stop");
    tokio::signal::ctrl_c().await?;
    Ok(())
}