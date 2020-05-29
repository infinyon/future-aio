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

    

}



#[cfg(test)]
mod basic_test {

    use std::io::Error;
    use std::thread;
    use std::time;

    use futures::future::join;
    use log::debug;


    use crate::test_async;
    use crate::task::spawn;



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
            Ok(()) as Result<(),()>
        };

        let ft2 = async {
            debug!("ft2: starting sleeping for 500ms");
            thread::sleep(time::Duration::from_millis(500)); 
            debug!("ft2: woke up");
         //   ft_id = 2;            
            Ok(()) as Result<(), ()>
        };

        let core_threads = num_cpus::get().max(1);
        debug!("num threads: {}",core_threads);
        let _rt = join(ft1,ft2).await;
        assert!(true);
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
        debug!("num threads: {}",core_threads);

        spawn(ft1);
        spawn(ft2);
        // wait for all futures complete
        thread::sleep(time::Duration::from_millis(2000));

        assert!(true);
       

        Ok(())
    }



}

