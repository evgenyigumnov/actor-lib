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
use tokio::runtime::Runtime;
use futures::lock::Mutex;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use async_trait::async_trait;
use tokio::sync::oneshot::Sender;

pub type CallbackFuture<O> = Pin<Box<dyn Future<Output = O> + Send>>;
pub type Callback<T,E> = Box<dyn (Fn(T) -> CallbackFuture<Result<(), E>>) + Send + Sync>;



#[derive(Debug)]
pub struct ActorRef<Actor, Message, State, Response, Error> {
    tx: Mutex<Option<mpsc::Sender<(Message, i32)>>>,
    rt:  Mutex<Option<Arc<Runtime>>>,
    join_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    state: Option<Arc<Mutex<State>>>,
    self_ref: Mutex<Option<Arc<ActorRef<Actor, Message, State, Response, Error>>>>,
    message_id: Mutex<i32>,
    promise: Mutex<HashMap<i32, Sender<Result<Response, Error>>>>,
    name: String,
    actor: PhantomData<Actor>,

}
#[derive(Debug)]
pub struct Context<Actor,Message, State, Response, Error> {
    pub mgs: Message,
    pub state: Arc<Mutex<State>>,
    pub self_ref: Arc<ActorRef<Actor, Message, State, Response, Error>>,
}

#[async_trait]
pub trait Handler <Actor,Message, State, Response, Error> {
    async fn receive(&self, ctx: Arc<Context<Actor, Message, State, Response, Error>>) -> Result<Response, Error>;
}

impl<Actor: Handler<Actor, Message, State, Response, Error> + Debug + Send + Sync + 'static, Message: Debug + Send + Sync + 'static, State: Debug + Send + Sync + 'static,
    Response: Debug + Send + Sync + 'static, Error: std::error::Error +  Debug + Send + Sync + From<std::io::Error> + 'static> ActorRef<Actor, Message, State, Response, Error> {
     pub async fn new(name: String, actor:Actor, state: State, buffer: usize) -> Arc<Self>
    {
        let state_arc = Arc::new(Mutex::new(state));
        let state_clone = state_arc.clone();
        let (tx, mut rx) = mpsc::channel(buffer);
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();

        let actor_ref = ActorRef {
            tx: Mutex::new(Some(tx)),
            rt: Mutex::new(None),
            join_handle: Mutex::new(None),
            state: Some(state_clone),
            self_ref: Mutex::new(None),
            message_id: Mutex::new(0),
            promise: Mutex::new(HashMap::new()),
            name: name,
            actor: PhantomData,
        };

        let ret = Arc::new(actor_ref);
        let ret_clone = ret.clone();
        let ret_clone2 = ret.clone();
        let ret_clone3 = ret.clone();
        let mut me = ret.self_ref.lock().await;
        *me = Some(ret.clone());


        let _join_handle = rt.spawn(async move {
            let me = ret_clone2.clone();
            while let Some(message) = rx.recv().await {
                let msg = message.0;
                let message_id = message.1;
                let state_clone = state_arc.clone();
                {
                    let state_lock = state_clone.lock().await;
                    log::debug!("<{}> Got message: {:?} Current state: {:?}", me.name, msg, state_lock);

                }
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
                        log::error!("<{}> Work error: {:?}", me.name, result);
                    }
                    let mut promise_lock = ret_clone3.promise.lock().await;
                    let promise = promise_lock.remove(&message_id);
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
                let state_lock = state_clone.lock().await;
                log::trace!("<{}> After work on message new state: {:?}", me.name, state_lock);
            }
        });
        let mut rt_mutex = ret.rt.lock().await;
        *rt_mutex = Some(Arc::new(rt));
        log::info!("<{}> Actor started", ret_clone.name);

        ret_clone
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
                    let mut message_id_lock = self.message_id.lock().await;
                    *message_id_lock += 1;
                    let mut promise_lock = self.promise.lock().await;
                    promise_lock.insert(*message_id_lock, sender);
                    let r = tx.send((mgs, *message_id_lock)).await;
                    if r.is_err() {
                        return Err(std::io::Error::new(std::io::ErrorKind::Other, "Err").into());
                    }
                }
                let r = receiver.await;
                match r {
                    Ok(res) => {res}
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
                match r {
                    Ok(_) => {}
                    Err(_) => {
                        return Err(std::io::Error::new(std::io::ErrorKind::Other, "Err").into());
                    }
                }
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

    pub async fn stop(&self){
        let mut tx_lock = self.tx.lock().await;
        *tx_lock = None;
        let mut rt_lock = self.rt.lock().await;
        let worker = rt_lock.take().unwrap();
        std::mem::forget(worker);
        *rt_lock = None;
        let mut me_lock = self.self_ref.lock().await;
        *me_lock = None;
        let mut join_handle_lock = self.join_handle.lock().await;
        *join_handle_lock = None;
        log::debug!("<{}> Stop worker", self.name);

    }
}



