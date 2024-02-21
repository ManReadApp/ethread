use egui::Context;
use poll_promise::Promise;
use std::future::Future;
#[cfg(target_family = "unix")]
use std::os::unix::thread::JoinHandleExt;
#[cfg(target_family = "windows")]
use std::os::windows::thread::JoinHandleExt;
use std::sync::{Arc, Mutex};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;

#[cfg(not(target_arch = "wasm32"))]
pub enum Handles {
    Std(std::thread::JoinHandle<()>),
    Tokio(tokio::task::JoinHandle<()>),
}

pub struct ThreadHandler<T: Send + 'static> {
    ctx: Arc<Mutex<Option<Context>>>,
    pub task: Promise<T>,
    #[cfg(not(target_arch = "wasm32"))]
    handle: Handles,
}

impl<T: Send + 'static> ThreadHandler<T> {
    /// executes async function and stores result in task
    #[cfg(target_arch = "wasm32")]
    pub fn new_async<A: Future<Output=T> + 'static>(future: A) -> Self {
        let (sender, promise) = Promise::new();
        let ctx =Arc::new(Mutex::new(None::<Context>));
        let ctx_move = ctx.clone();
        spawn_local(async move {
            let res = future.await;
            sender.send(res);
            let is_ctx = ctx_move.lock().unwrap().is_some();
            if is_ctx {
                ctx_move.lock().unwrap().as_ref().unwrap().request_repaint();
            }
        });
        Self { task: promise, ctx }
    }

    /// executes async function and stores result in task
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_async<A: Future<Output=T> + 'static + Send>(
        future: A,
    ) -> Self {
        let (sender, promise) = Promise::new();
        let ctx = Arc::new(Mutex::new(None::<Context>));
        let ctx_move = ctx.clone();
        let handle = tokio::spawn(async move {
            let res = future.await;
            sender.send(res);
            let is_ctx = ctx_move.lock().unwrap().is_some();
            if is_ctx {
                ctx_move.lock().unwrap().as_ref().unwrap().request_repaint();
            }
        });

        Self {
            ctx,
            task: promise,
            handle: Handles::Tokio(handle),
        }
    }

    /// executes sync function in async thread and stores result in task
    #[cfg(target_arch = "wasm32")]
    pub fn new<A: 'static + Send + FnOnce() -> T>(func: A) -> Self {
        let (sender, promise) = Promise::new();
        let ctx = Arc::new(Mutex::new(None::<Context>));
        let ctx_move = ctx.clone();
        spawn_local(async move {
            sender.send(func());
            let is_ctx = ctx_move.lock().unwrap().is_some();
            if is_ctx {
                ctx_move.lock().unwrap().as_ref().unwrap().request_repaint();
            }
        });
        Self { task: promise, ctx }
    }

    /// executes sync function in a tokio(asnyc) thread and stores result in task
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new<A: FnOnce() -> T + 'static + Send>(func: A) -> Self {
        let (sender, promise) = Promise::new();
        let ctx =Arc::new(Mutex::new(None::<Context>));
        let ctx_move = ctx.clone();
        let handle = tokio::spawn(async move {
            let data = async { func() }.await;
            sender.send(data);
            let is_ctx = ctx_move.lock().unwrap().is_some();
            if is_ctx {
                ctx_move.lock().unwrap().as_ref().unwrap().request_repaint();
            }
        });

        Self {
            ctx,
            task: promise,
            handle: Handles::Tokio(handle),
        }
    }

    /// executes sync function in std thread and stores result in task
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_sync<A: FnOnce() -> T + 'static + Send>(func: A) -> Self {
        let (sender, promise) = Promise::new();
        let ctx = Arc::new(Mutex::new(None::<Context>));
        let ctx_move = ctx.clone();
        let handle = std::thread::spawn(move || {
            sender.send(func());
            let is_ctx = ctx_move.lock().unwrap().is_some();
            if is_ctx {
                ctx_move.lock().unwrap().as_ref().unwrap().request_repaint();
            }
        });

        Self {
            task: promise,
            handle: Handles::Std(handle),
            ctx,
        }
    }

    pub fn set_context(&mut self, ctx: Context) {
        *self.ctx.lock().unwrap() = Some(ctx);
    }

    /// executes Self::new
    /// this function exists to prevent errors when compiling to native and web
    #[cfg(target_arch = "wasm32")]
    pub fn new_sync<A: FnOnce() -> T + 'static + Send>(func: A) -> Self {
        Self::new(func)
    }

    /// kills running thread
    /// only works on native
    pub fn dispose_of_thread(self) {
        #[cfg(not(target_arch = "wasm32"))]
        match self.handle {
            Handles::Std(handle) => {
                #[cfg(target_family = "unix")]
                unsafe {
                    libc::pthread_cancel(handle.as_pthread_t())
                };
                #[cfg(target_family = "windows")]
                unsafe {
                    kernel32::TerminateThread(handle.as_raw_handle(), 0)
                };
            }
            Handles::Tokio(handle) => handle.abort(),
        }
    }
}
