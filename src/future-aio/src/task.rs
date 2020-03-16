pub use task::*;

#[cfg(feature = "asyncstd")]
mod task {

    use std::future::Future;

    use async_std::task::JoinHandle;
    use async_std::task;
    use log::trace;

    use crate::timer::sleep;

    /// run future and wait forever
    /// this is typically used in the server
    pub fn run<F>(spawn_closure: F)
    where
        F: Future<Output = ()> + Send + 'static 
    {   
        task::block_on(spawn_closure);
}

    /// run future and wait forever
    /// this is typically used in the server
    pub fn main<F>(spawn_closure: F)
    where
        F: Future<Output = ()> + Send + 'static 
    {   
        use std::time::Duration;

        task::block_on(async{
            spawn_closure.await;
            // do infinite loop for now
            loop {
                sleep(Duration::from_secs(3600)).await;
            }
        });
    }


    pub fn spawn<F,T>(future: F) -> JoinHandle<T>
    where
        F: Future<Output = T> + 'static + Send, 
        T: Send + 'static
    {
        trace!("spawning future");
        task::spawn(future)
    }

    pub fn spawn_blocking<F, T>(future: F) -> JoinHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static
    {
        trace!("spawning blocking");
        task::spawn_blocking(future)
    }


    /// same as async async std block on
    pub fn run_block_on<F,T>(f:F) -> T
        where F: Future<Output = T>
    {
        task::block_on(f)
    } 

}

#[cfg(feature = "tokio2")]
mod task {


    use std::future::Future;
    use std::io::Error as IoError;

    use tokio::runtime;
    use tokio::task;
    use tokio::task::JoinHandle;
    use log::trace;

    fn create_thread_runtime() -> Result<runtime::Runtime,IoError> {
        runtime::Builder::new()
            .threaded_scheduler()
            .enable_all()
            .build()
    }

    /// run future and wait forever
    /// this is typically used in the server
    pub fn run<F>(spawn_closure: F)
    where
        F: Future<Output = ()> + Send + 'static 
    {   
        let mut rt = create_thread_runtime().expect("threaded runtime cannot be build");
        rt.block_on(spawn_closure);
    }

    /// run future and wait forever
    /// this is typically used in the server
    pub fn main<F>(spawn_closure: F)
    where
        F: Future<Output = ()> + Send + 'static 
    {   
        let mut rt = create_thread_runtime().expect("threaded runtime cannot be build");
        rt.block_on(spawn_closure);
    }


    pub fn spawn<F,T>(future: F) -> JoinHandle<T>
    where
        F: Future<Output = T> + 'static + Send, 
        T: Send + 'static
    {
        trace!("spawning future");
        task::spawn(future)
    }

    pub async fn spawn_blocking<F, T>(future: F) -> T
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static
    {
        trace!("spawning blocking");
        match task::spawn_blocking(future).await {
            Ok(output) => output,
            Err(err) => panic!("failure to join: {}",err)
        }
    }


    /// same as async async std block on
    pub fn run_block_on<F,T>(f:F) -> T
        where F: Future<Output = T>
    {
        let mut rt = create_thread_runtime().expect("threaded runtime cannot be build");
        rt.block_on(f)
    } 

}

#[cfg(test)]
mod test {

    use lazy_static::lazy_static;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::{thread, time};

    use super::run;
    use super::spawn;

    #[test]
    fn test_spawn3() {
        lazy_static! {
            static ref COUNTER: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
        }

        assert_eq!(*COUNTER.lock().unwrap(), 0);

        let ft = async {
            thread::sleep(time::Duration::from_millis(100));
            *COUNTER.lock().unwrap() = 10;
        };

        run(async {
            let join_handle = spawn(ft);
            join_handle.await;
        });

        assert_eq!(*COUNTER.lock().unwrap(), 10);
    }

    /*
    // this is sample code to show how to keep test goging
    //#[test]
    fn test_sleep() {
       

        let ft = async {
            for _ in 0..100 {
                println!("sleeping");
                super::sleep(time::Duration::from_millis(1000)).await;
            }
           
        };

        run(async {
            let join_handle = spawn(ft);
            join_handle.await;
        });
    
    }
    */

    /*
    use std::future::Future;
    use std::task::Context;
    use std::task::Poll;
    use std::pin::Pin;
    use std::io;


    use async_std::task::spawn_blocking;

    struct BlockingFuture {
    }

    impl Future for BlockingFuture {

        type Output = io::Result<()>;

        fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<io::Result<()>> {
            
            println!("start poll");
            
            spawn_blocking(move || { 
                println!("start sleeping");
                thread::sleep(time::Duration::from_millis(100));
                println!("wake up from sleeping");
            });
            
            Poll::Pending
        }

    }

    //#[test]
    fn test_block_spawning() {

        run(async {
            let block = BlockingFuture{};
            block.await;
        });        

    }
    */


}
