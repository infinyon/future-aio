use std::future::Future;

use crate::timer::sleep;

/// run future and return immediately
pub fn run<F>(spawn_closure: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            wasm_bindgen_futures::spawn_local(spawn_closure);
        } else {
            async_global_executor::block_on(spawn_closure);
        }
    }
}

/// run future and wait forever
/// this is typically used in the server
pub fn main<F>(spawn_closure: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    use std::time::Duration;

    run(async {
        spawn_closure.await;
        // do infinite loop for now
        loop {
            cfg_if::cfg_if! {
                if #[cfg(target_arch = "wasm32")] {
                    sleep(Duration::from_secs(3600)).await.unwrap();
                } else {
                    sleep(Duration::from_secs(3600)).await;
                }
            }
        }
    });
}

// preserve async-std spawn behavior which is always detach task.
// to fully control task life cycle, use spawn_task
cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        pub fn spawn<F: Future<Output = T> + 'static, T: 'static>(future: F) {
            wasm_bindgen_futures::spawn_local(async move {
                let _ = future.await;
            });
        }
    } else {
        pub fn spawn<F: Future<Output = T> + Send + 'static, T: Send + 'static>(future: F) {
            async_global_executor::spawn(future).detach();
        }
        pub fn spawn_task<F: Future<Output = T> + Send + 'static, T: Send + 'static>(future: F) -> async_task::Task<T> {
            async_global_executor::spawn(future)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use async_global_executor::spawn_blocking;

cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        pub fn run_block_on<F, T>(f: F)
            where
                F: Future<Output = T> + 'static,
                T: 'static,
            {
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = f.await;
                });
            }
    } else {
        pub use async_global_executor::block_on as run_block_on;
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub type Task<T> = async_task::Task<T>;

#[cfg(test)]
mod basic_test {

    use std::io::Error;
    use std::thread;
    use std::time;

    use futures_lite::future::zip;
    use tracing::debug;

    use crate::task::spawn;
    use crate::test_async;

    #[test_async]
    async fn future_join() -> Result<(), Error> {
        // with join, futures are dispatched on same thread
        // since ft1 starts first and
        // blocks on thread, it will block future2
        // should see ft1,ft1,ft2,ft2

        //let mut ft_id = 0;

        let ft1 = async {
            debug!("ft1: starting sleeping for 1000ms");
            // this will block ft2.  both ft1 and ft2 share same thread
            thread::sleep(time::Duration::from_millis(1000));
            debug!("ft1: woke from sleep");
            //  ft_id = 1;
            Ok(()) as Result<(), ()>
        };

        let ft2 = async {
            debug!("ft2: starting sleeping for 500ms");
            thread::sleep(time::Duration::from_millis(500));
            debug!("ft2: woke up");
            //   ft_id = 2;
            Ok(()) as Result<(), ()>
        };

        let core_threads = num_cpus::get().max(1);
        debug!("num threads: {}", core_threads);
        let _ = zip(ft1, ft2).await;
        Ok(())
    }

    #[test_async]
    async fn future_spawn() -> Result<(), Error> {
        // with spawn, futures are dispatched on separate thread
        // in this case, thread sleep on ft1 won't block
        // should see  ft1, ft2, ft2, ft1

        let ft1 = async {
            debug!("ft1: starting sleeping for 1000ms");
            thread::sleep(time::Duration::from_millis(1000)); // give time for server to come up
            debug!("ft1: woke from sleep");
        };

        let ft2 = async {
            debug!("ft2: starting sleeping for 500ms");
            thread::sleep(time::Duration::from_millis(500));
            debug!("ft2: woke up");
        };

        let core_threads = num_cpus::get().max(1);
        debug!("num threads: {}", core_threads);

        spawn(ft1);
        spawn(ft2);
        // wait for all futures complete
        thread::sleep(time::Duration::from_millis(2000));

        Ok(())
    }
}
