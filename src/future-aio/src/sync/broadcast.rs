/// In memory implementation of event stream
/// Event Stream has fixed capacity.
/// there can be multiple producer and consumer.

pub use tokio::sync::broadcast::Receiver;
pub use tokio::sync::broadcast::Sender;
pub use tokio::sync::broadcast::channel;
pub use tokio::sync::broadcast::RecvError;



#[cfg(test)]
mod test {
    use std::time::Duration;

    use log::debug;
    use futures::stream::StreamExt;
    use futures::select;

    use crate::timer::sleep;
    use crate::test_async;
    use crate::task::spawn;
    
    use super::Receiver;
    use super::Sender;
    use super::channel;
    use super::RecvError;


    async fn test_receive(mut receiver: Receiver<u16>,count: u16) {

        debug!("start receive: {}",count);


        let mut sum = 0;

        loop {

            debug!("conn: {} waiting",count);

            match receiver.recv().await {

                Ok(value) =>  {
                    debug!("conn: {}, received: {}",count,value);
                    sum += value;
                },
                Err(err) => {

                    match err {
                        RecvError::Closed => {
                            debug!("conn: {} end, terminating",count);
                            assert_eq!(sum,10);
                            return;
                        },
                        RecvError::Lagged(_) => {
                            debug!("lagging");
                        }
                    }
                    
                }
            }
            

        }

    }


    /// test multiple send and receiver
    #[test_async]
    async fn test_conn() -> Result<(), ()> {
        
        let (tx, receiver): (Sender<u16>,Receiver<u16>) = channel(100);

        // create multiple receiver

        for i in 0..2 {
            spawn(test_receive(tx.subscribe(), i));
        }

        sleep(Duration::from_millis(100)).await;

        //let sender2 = sender.clone();

        for i in 0..5 {
            tx.send(i as u16).expect("should be sent");
        }

        sleep(Duration::from_millis(100)).await;
        debug!("waiting 5 seconds");
        drop(tx);
        drop(receiver);

        sleep(Duration::from_millis(100)).await;
        debug!("finished test");

        Ok(())
    }

}