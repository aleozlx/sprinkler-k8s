use std::io::Write;
use std::thread;
use std::sync::{mpsc, Arc, Mutex};
use futures::future::{self, Either};
use tokio::prelude::*;
use super::{Sprinkler, SprinklerProto, SprinklerOptions, Message};

#[derive(Clone)]
pub struct DockerOOM {
    options: Arc<SprinklerOptions>,
    _deactivate: Arc<Mutex<bool>>
}

impl Sprinkler for DockerOOM {
    fn build(options: SprinklerOptions) -> Self {
        DockerOOM {
            options: Arc::new(options),
            _deactivate: Arc::new(Mutex::new(false))
        }
    }

    fn id(&self) -> usize {
        self.options._id
    }

    fn hostname(&self) -> &str {
        &self.options._hostname
    }

    fn activate_master(&self) -> mpsc::Sender<Message> {
        unimplemented!();
        // 


        let clone = self.clone();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut state = false;
            loop {
                // detect oom
                let state_recv = false; //rx.try_recv().is_ok();
                
                if state != state_recv {
                    state = state_recv;
                    // lookup pod
                    // kill pod, kill continer, rm --force
                    // info!(
                    //     "sprinkler[{}] (CommCheck) {} => {}",
                    //     clone.id(), clone.hostname(), if state {"online"} else {"offline"}
                    // );
                }
                thread::sleep(std::time::Duration::from_secs(clone.options.heart_beat));
                if *clone._deactivate.lock().unwrap() { break; }
                else { trace!("sprinkler[{}] heartbeat", clone.id()); }
            }
        });
        tx
    }

    fn activate_agent(&self) {
        let docker = shiplift::Docker::new();
        // let ref now = chrono::Local::now().timestamp() as u64;
        // let ref event_opts = shiplift::EventsOptions::builder().since(now).build();
        let monitor = docker
            .events(&Default::default())
            .for_each(|e| {
                println!("event -> {:?}", e);
                Ok(())
            })
            .map_err(|e| error!("{}", e));
        tokio::run(monitor);
    }

    fn deactivate(&self) {
        *self._deactivate.lock().unwrap() = true;
    }
}