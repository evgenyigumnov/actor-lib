

#[cfg(test)]
mod tests {
    use std::fmt::Debug;
    use std::sync::{Arc};
    use std::thread;
    use actorlib::*;

    #[derive(Debug)]
    pub enum UserMessage {
        CreateAccount { account_id: u32, } ,
        GetBalance { account_id: u32, },
        MoveMoney { from_account_id: u32, to_account_id: u32, amount: u32 },
    }

    #[derive(Debug)]
    pub enum UserResponse {
        Balance { amount: u32, },
        AccountCreated { account_id: u32, },
        Ok,
    }
    #[derive(Debug,Clone)]
    pub struct UserState {
        pub name: String,
        pub accounts:Vec<Arc<Actor<AccountMessage, AccountState ,AccountResponse>>>
    }

    #[derive(Debug)]
    pub enum AccountMessage {
        GetBalance { account_id: u32, },
        AddMoney { account_id: u32, amount: u32 },
    }

    #[derive(Debug)]
    pub enum AccountResponse {
        Balance { amount: u32, },
        AccountCreated { account_id: u32, },
        Ok,
    }

    #[derive(Debug,Clone)]
    pub struct AccountState {
        pub account_id: u32,
        pub balance: u32,
    }

    #[tokio::test]
    async fn test_1() -> Result<(), BoxDynError> {
        let _ = env_logger::Builder::from_env(env_logger::Env::new().default_filter_or("trace")).try_init();

        let user:Arc<Actor<UserMessage, UserState ,UserResponse>>  = Actor::new("user".to_string(), move |ctx| {
            Box::pin(async move {
                match ctx.mgs {
                    UserMessage::GetBalance { .. } => {
                        let state_lock = ctx.state.lock().await;
                        let account = state_lock.accounts.first().unwrap();

                        let result = account.ask(AccountMessage::GetBalance { account_id: 1 }).await?;
                        match result {
                            AccountResponse::Balance { amount } => {   Ok(UserResponse::Balance { amount:amount }) }
                            _ => { Ok(UserResponse::Balance { amount: 0 })}
                        }
                    }
                    UserMessage::CreateAccount { .. } => {
                        let account = Actor::new("account".to_string(), move |ctx| {
                            Box::pin(async move {
                                match ctx.mgs {
                                    AccountMessage::GetBalance { .. } => {
                                        Ok(AccountResponse::Balance { amount: 100 })
                                    }
                                    AccountMessage::AddMoney { .. } => {
                                        Ok(AccountResponse::Ok)
                                    }
                                }
                            })
                        }, AccountState{ account_id: 1, balance: 0 }, 10000).await;
                        let state_lock = &mut ctx.state.lock().await;
                        state_lock.accounts.push(account);

                        Ok(UserResponse::Ok)
                    }
                    _ => {
                        Ok(UserResponse::Ok)
                    }
                }
            })
        }, UserState{ name: "John".to_string(), accounts: vec![] }, 10000).await;

        user.send(UserMessage::CreateAccount{ account_id: 1 }).await?;
        user.send(UserMessage::GetBalance{ account_id: 1 }).await?;
        let res_ask = user.ask(UserMessage::GetBalance{ account_id: 1 }).await.unwrap();
        assert!(matches!(res_ask, UserResponse::Balance { amount: 100 }));
        let state = user.state().await?;
        let state_lock = &mut state.lock().await;
        state_lock.name = "John2".to_string();
        user.stop().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_2() -> Result<(), BoxDynError> {
        let _ = env_logger::Builder::from_env(env_logger::Env::new().default_filter_or("trace")).try_init();



        let mut user:Arc<Actor<UserMessage, i32 ,UserResponse>>  = Actor::new("user".to_string(), move |ctx| {
            Box::pin(async move {
                match ctx.mgs {
                            UserMessage::GetBalance { .. } => {
                                Ok(UserResponse::Balance { amount: 100 })
                            }
                            _ => {Ok(UserResponse::Ok)}
                        }
            })
        }, 1, 10000).await;

        user.send(UserMessage::CreateAccount{ account_id: 0 }).await?;
        user.callback(UserMessage::GetBalance { account_id: 1 }, |result| {
            Box::pin(async move {
                log::debug!("Result: {:?}", result);
                Ok(())
            })
        }).await?;

        let user_arc = user.clone();
        let user2:Arc<Actor<UserMessage, () ,UserResponse>>  = Actor::new("user".to_string(), move |ctx| {
            let user_clone = user_arc.clone();
            Box::pin(async move {
                match ctx.mgs {
                    UserMessage::GetBalance { .. } => {
                        Ok(UserResponse::Balance { amount: 100 })
                    }
                    _ => {
                        user_clone.send(UserMessage::MoveMoney { from_account_id: 0,to_account_id:0 , amount: 0 }).await?;
                        Ok(UserResponse::Ok)
                    }
                }
            })
        }, (), 10000).await;

        user2.send(UserMessage::CreateAccount{ account_id: 0 }).await?;
        user.send(UserMessage::CreateAccount{ account_id: 0 }).await?;

        thread::sleep(std::time::Duration::from_millis(100));
        user.stop().await;
        thread::sleep(std::time::Duration::from_millis(100));
        let result = user.send(UserMessage::CreateAccount{ account_id: 0 }).await;
        let r = result.is_err();
        assert!(matches!( r, true));
        thread::sleep(std::time::Duration::from_millis(100));
        Ok(())
    }
}

fn main() {}