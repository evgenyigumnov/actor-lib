//! # Actor-lib SDK
//! This is the Rust SDK for Actor-lib.
//!
//! ## Usage
//! ```rust
//!use actor_lib::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), BoxDynError> {
//!
//!    Ok(())
//! }

use std::collections::HashMap;
use std::error::Error;
use std::fmt::Debug;
use std::sync::{Arc};
use tokio::sync::{mpsc, oneshot};
use tokio::runtime::Runtime;
use futures::lock::Mutex;
use std::future::Future;
use std::pin::Pin;
use tokio::sync::oneshot::Sender;

pub type BoxDynError =  Box<dyn Error + Send + Sync>;
pub type CallbackFuture<O> = Pin<Box<dyn Future<Output = O> + Send>>;
pub type Callback<T> = Box<dyn (Fn(T) -> CallbackFuture<Result<(), BoxDynError>>) + Send + Sync>;



#[derive(Debug)]
pub struct Actor<Message, State, Response> {
    tx: Mutex<Option<mpsc::Sender<(Message, i32)>>>,
    rt:  Mutex<Option<Arc<Runtime>>>,
    join_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    state: Option<Arc<Mutex<State>>>,
    self_ref: Mutex<Option<Arc<Actor<Message, State, Response>>>>,
    message_id: Mutex<i32>,
    call_back: Mutex<HashMap<i32, Callback<Result<Response, BoxDynError>>>>,
    promise: Mutex<HashMap<i32, Sender<Result<Response, BoxDynError>>>>,
    name: String,

}
#[derive(Debug)]
pub struct Context<Message, State, Response> {
    pub mgs: Message,
    pub state: Arc<Mutex<State>>,
    pub self_ref: Arc<Actor<Message, State, Response>>,
}


impl<Message: Debug + Send + Sync + 'static, State: Debug + Send + Sync + 'static, Response: Debug + Send + Sync + 'static> Actor<Message, State, Response> {
     pub async fn new<F>(name: String, handler: F, state: State, buffer: usize) -> Arc<Self>
        where
            F: Fn(Context<Message, State, Response>) -> CallbackFuture<Result<Response, BoxDynError>>,
            F: Send + Sync + 'static,
    {
        let state_arc = Arc::new(Mutex::new(state));
        let state_clone = state_arc.clone();
        let (tx, mut rx) = mpsc::channel(buffer);
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();

        let actor = Actor {
            tx: Mutex::new(Some(tx)),
            rt: Mutex::new(None),
            join_handle: Mutex::new(None),
            state: Some(state_clone),
            self_ref: Mutex::new(None),
            message_id: Mutex::new(0),
            call_back: Mutex::new(HashMap::new()),
            promise: Mutex::new(HashMap::new()),
            name: name,
        };

        let ret = Arc::new(actor);
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
                let r = handler(context);
                {
                    let result = r.await;
                    log::trace!("<{}> Work result: {:?}", me.name, result);
                    if result.is_err() {
                        log::error!("<{}> Work error: {:?}", me.name, result);
                    }
                    let mut call_back_lock = ret_clone3.call_back.lock().await;
                    let call_back = call_back_lock.remove(&message_id);
                    match call_back {
                        None => {
                            log::trace!("<{}> No callback for message_id: {}", me.name, message_id);

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
                        Some(call_back) => {
                            log::trace!("<{}> Callback result: {:?}", me.name, result);
                            let _ = call_back(result).await;
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

    pub async fn callback<F>(&self, msg: Message, handler: F) -> Result<(), BoxDynError>
        where
            F: Fn(Result<Response, BoxDynError>) -> CallbackFuture<Result<(), BoxDynError>>,
            F: Send + Sync + 'static,
    {


        log::trace!("<{}> Ask message: {:?}", self.name, msg);
        let tx_lock = self.tx.lock().await;
        let tx = tx_lock.as_ref();
        match tx {
            None => {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "Err").into());
            }
            Some(tx) => {
                let mut message_id_lock = self.message_id.lock().await;
                *message_id_lock += 1;
                let mut call_back_lock = self.call_back.lock().await;
                call_back_lock.insert(*message_id_lock, Box::new(handler));
                tx.send((msg, *message_id_lock)).await?;
            }
        }
        Ok(())
    }

    pub async fn ask(&self, mgs: Message) -> Result<Response, BoxDynError>
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
                    tx.send((mgs, *message_id_lock)).await?;
                }
                let result:Result<Response, BoxDynError> = receiver.await?;
                result
            }
        }
    }



    pub async fn send(&self, msg: Message) -> Result<(), BoxDynError> {
        log::trace!("<{}> Push message: {:?}", self.name, msg);
        let tx_lock = self.tx.lock().await;
        let tx = tx_lock.as_ref();
        match tx {
            None => {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "Err").into());
            }
            Some(tx) => {
                tx.send((msg, 0)).await?;
            }
        }
        Ok(())
    }

    pub async fn state(&self) -> Result<Arc<Mutex<State>>, BoxDynError> {
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
        // self.state = None;
        let mut join_handle_lock = self.join_handle.lock().await;
        *join_handle_lock = None;
        log::debug!("<{}> Stop worker", self.name);

    }
}



