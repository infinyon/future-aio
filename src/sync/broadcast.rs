/// In memory implementation of event stream
/// Event Stream has fixed capacity.
/// there can be multiple producer and consumer.

pub use tokio::sync::broadcast::Receiver;
pub use tokio::sync::broadcast::Sender;
pub use tokio::sync::broadcast::channel;
pub use tokio::sync::broadcast::RecvError;




#[derive(Debug)]
pub struct Channel<T> {
    receiver: Receiver<T>,
    sender: Sender<T>
}

impl <T>Channel<T> {

    pub fn new(capacity: usize) -> Self {

        let (sender,receiver) = channel(capacity);
        Self {
            receiver,
            sender
        }
    }

    /// create new clone of sender
    pub fn receiver(&self) -> Receiver<T> {
        self.sender.subscribe()
    }


    pub fn sender(&self) -> Sender<T> {
        self.sender.clone()
    }

   
}



#[cfg(test)]
mod test {
    use std::time::Duration;

    use log::debug;


    use crate::timer::sleep;
    use crate::test_async;
    use crate::task::spawn;
    
    use super::Channel;
    use super::Receiver;
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
                        RecvError::Lagged(lag) => {
                            debug!("conn: {}, lagging: {}",count,lag);
                        }
                    }
                    
                }
            }
            

        }

    }


    /// test multiple send and receiver
    #[test_async]
    async fn test_conn() -> Result<(), ()> {
        
        let channel = Channel::new(2);

        // create multiple receiver

        // need to create dummy receiver
        for i in 0..2 {
            spawn(test_receive(channel.receiver(), i));
        }

       // spawn(test_receive(receiver,2));

        sleep(Duration::from_millis(100)).await;

        //let sender2 = sender.clone();

        let sender = channel.sender();

        for i in 0..5 {
            sender.send(i as u16).expect("should be sent");
            sleep(Duration::from_millis(10)).await;
        }

        sleep(Duration::from_millis(100)).await;
        debug!("waiting 5 seconds");
        drop(channel);

        sleep(Duration::from_millis(100)).await;
        debug!("finished test");

        Ok(())
    }

}