//! # Actorlib SDK
//! This is the Rust SDK for Actorlib.
//!
//! ## Usage
//! ```rust
//!use actorlib::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), std::io::Error> {
//!
//!    Ok(())
//! }

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc};
use tokio::sync::{mpsc, oneshot};
use futures::lock::{Mutex};
use std::future::Future;
use std::pin::Pin;
use async_trait::async_trait;
use tokio::sync::oneshot::Sender;

pub type CallbackFuture<O> = Pin<Box<dyn Future<Output=O> + Send>>;
pub type Callback<T, E> = Box<dyn (Fn(T) -> CallbackFuture<Result<(), E>>) + Send + Sync>;


#[derive(Debug)]
pub struct ActorRef<Actor, Message, State, Response, Error> {
    tx: Mutex<Option<mpsc::Sender<(Message, i32)>>>,
    join_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    state: Option<Arc<Mutex<State>>>,
    self_ref: Mutex<Option<Arc<ActorRef<Actor, Message, State, Response, Error>>>>,
    message_id: Mutex<i32>,
    promise: Mutex<HashMap<i32, Sender<Result<Response, Error>>>>,
    name: String,
    actor: Arc<Actor>,
}

#[derive(Debug)]
pub struct Context<Actor, Message, State, Response, Error> {
    pub mgs: Message,
    pub state: Arc<Mutex<State>>,
    pub self_ref: Arc<ActorRef<Actor, Message, State, Response, Error>>,
}

#[async_trait]
pub trait Handler<Actor: Sync + Send + 'static, Message: Sync + Send + 'static, State: Sync + Send + 'static, Response: Sync + Send + 'static, Error: Sync + Send + 'static> {
    async fn receive(&self, ctx: Arc<Context<Actor, Message, State, Response, Error>>) -> Result<Response, Error>;
    async fn pre_start(&self, _state: Arc<Mutex<State>>, _self_ref: Arc<ActorRef<Actor, Message, State, Response, Error>>) -> Result<(), Error> {
        Ok(())
    }
    async fn pre_stop(&self, _state: Arc<Mutex<State>>, _self_ref: Arc<ActorRef<Actor, Message, State, Response, Error>>) -> Result<(), Error> {
        Ok(())
    }
}

impl<Actor: Handler<Actor, Message, State, Response, Error> + Debug + Send + Sync + 'static, Message: Debug + Send + Sync + 'static, State: Debug + Send + Sync + 'static,
    Response: Debug + Send + Sync + 'static, Error: std::error::Error + Debug + Send + Sync + From<std::io::Error> + 'static> ActorRef<Actor, Message, State, Response, Error> {
    pub async fn new(name: String, actor: Actor, state: State, buffer: usize) -> Result<Arc<Self>, Error>
    {
        let state_arc = Arc::new(Mutex::new(state));
        let state_clone = state_arc.clone();
        let (tx, mut rx) = mpsc::channel(buffer);
        let actor_arc= Arc::new(actor);
        let actor = actor_arc.clone();
        let actor_ref = ActorRef {
            tx: Mutex::new(Some(tx)),
            join_handle: Mutex::new(None),
            state: Some(state_clone),
            self_ref: Mutex::new(None),
            message_id: Mutex::new(0),
            promise: Mutex::new(HashMap::new()),
            name: name,
            actor: actor_arc.clone(),
        };

        let ret = Arc::new(actor_ref);
        let ret_clone = ret.clone();
        let ret_clone2 = ret.clone();
        let ret_clone3 = ret.clone();
        *ret.self_ref.lock().await = Some(ret.clone());

        let join_handle = tokio::spawn(async move {
            let me = ret_clone2.clone();

            loop {
                tokio::select! {
                _ = futures::future::pending::<()>() => {

                },
                msg_opt = rx.recv() => {

                        match msg_opt {
                            None => {
                                log::debug!("<{}> No message", me.name);
                                break;
                            }
                            Some(message) => {

                                    let msg = message.0;
                                    let message_id = message.1;
                                    let state_clone = state_arc.clone();
                                    log::debug!("<{}> Got message: {:?} Current state: {:?}", me.name, msg, state_clone.lock().await);
                                    let msg_debug = format!("{:?}", msg);
                                    let state = state_arc.clone();
                                    let context = Context {
                                        mgs: msg,
                                        state: state,
                                        self_ref: me.clone(),
                                    };

                                    let r = actor.receive(Arc::new(context));
                                    {
                                        let result = r.await;
                                        log::trace!("<{}> Work result: {:?}", me.name, result);
                                        if result.is_err() {
                                            log::error!("<{}> Work error: {:?} on message {}", me.name, result, msg_debug);
                                        }
                                        let promise = ret_clone3.promise.lock().await.remove(&message_id);
                                        match promise {
                                            None => {
                                                log::trace!("<{}> No promise for message_id: {}", me.name, message_id);
                                            }
                                            Some(promise) => {
                                                log::trace!("<{}> Promise result: {:?}", me.name, result);
                                                let _ = promise.send(result);
                                            }
                                        }

                                    }
                                    let state_clone = state_arc.clone();
                                    log::trace!("<{}> After work on message new state: {:?}", me.name, state_clone.lock().await);



                            }
                        };


                }
            }
            }
        });
        *ret.join_handle.lock().await = Some(join_handle);
        let _ = actor_arc.pre_start(ret_clone.state.clone().unwrap(), ret_clone.clone()).await?;
        log::info!("<{}> Actor started", ret_clone.name);
        Ok(ret_clone)
    }


    pub async fn ask(&self, mgs: Message) -> Result<Response, Error>
    {
        log::trace!("<{}> Result message: {:?}", self.name, mgs);
        let tx_lock = self.tx.lock().await;
        let tx = tx_lock.as_ref();
        match tx {
            None => {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "Err").into());
            }
            Some(tx) => {
                let (sender, receiver) = oneshot::channel();
                {
                    *self.message_id.lock().await += 1;
                    self.promise.lock().await.insert(*self.message_id.lock().await, sender);
                    let r = tx.send((mgs, *self.message_id.lock().await)).await;
                    if r.is_err() {
                        return Err(std::io::Error::new(std::io::ErrorKind::Other, "Err").into());
                    }
                }
                let r = receiver.await;
                match r {
                    Ok(res) => { res }
                    Err(_) => {
                        return Err(std::io::Error::new(std::io::ErrorKind::Other, "Err").into());
                    }
                }
            }
        }
    }


    pub async fn send(&self, msg: Message) -> Result<(), std::io::Error> {
        log::trace!("<{}> Push message: {:?}", self.name, msg);
        let tx_lock = self.tx.lock().await;
        let tx = tx_lock.as_ref();
        match tx {
            None => {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "Err").into());
            }
            Some(tx) => {
                let r = tx.send((msg, 0)).await;
                if r.is_err() {
                    return Err(std::io::Error::new(std::io::ErrorKind::Other, "Err").into());
                }
                // let r = tx.try_send((msg, 0));
                // match r {
                //     Ok(_) => {}
                //     Err(err) => {
                //         let err_str = format!("{:?}", err);
                //         return Err(std::io::Error::new(std::io::ErrorKind::Other, err_str).into());
                //     }
                // }
            }
        }
        Ok(())
    }

    pub async fn state(&self) -> Result<Arc<Mutex<State>>, std::io::Error> {
        let state_opt = self.state.clone();
        log::trace!("<{}> State: {:?}", self.name, state_opt);
        match state_opt {
            None => {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "No state").into());
            }
            Some(state) => {
                Ok(state)
            }
        }
    }

    pub async fn stop(&self) -> Result<(), Error> {
        let self_ref =  self.self_ref.lock().await.clone().unwrap();
        let _ = self.actor.pre_stop(self.state.clone().unwrap(), self_ref).await?;
        *self.tx.lock().await = None;
        *self.self_ref.lock().await = None;
        let join_handle = self.join_handle.lock().await.take();
        match join_handle {
            None => {}
            Some(join_handle) => {
                let _ = join_handle.abort();
                log::debug!("join_handle abort()");
            }
        }
        *self.join_handle.lock().await = None;
        log::debug!("<{}> Stop worker", self.name);
        Ok(())
    }
}



