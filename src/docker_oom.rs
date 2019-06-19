use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio::prelude::*;
use super::*;

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

    fn activate_master(&self) -> ActivationResult {
        // let clone = self.clone();
        let (tx, rx) = futures::sync::mpsc::channel::<Message>(512);
        tokio::run({
            rx.for_each(|message| {
                println!("Got new message");

            // let mut last_seen = chrono::Local::now();
            //     // detect oom
            //     let state_recv = if let Ok(message) = rx.try_recv() {
            //         last_seen = chrono::Local::now();
            //         // message.body == COMMCHK
            //         debug!("{}", message.body);
            //         true
            //     } else {
            //         if state {
            //             // Tolerance (secs) for accumulated network delays
            //             const TOLERANCE: i64 = 2;
            //             if chrono::Local::now() - last_seen < chrono::Duration::seconds((clone.options.heart_beat as i64)+TOLERANCE)  {
            //                 debug!("sprinkler[{}] (CommCheck) on {} may be delayed.", clone.id(), clone.hostname());
            //                 true
            //             }
            //             else { false }
            //         }
            //         else { false }
            //     };
                
            //     if state != state_recv {
            //         state = state_recv;
            //         // lookup pod
            //         // kill pod, kill continer, rm --force
            //         // info!(
            //         //     "sprinkler[{}] (CommCheck) {} => {}",
            //         //     clone.id(), clone.hostname(), if state {"online"} else {"offline"}
            //         // );
            //     }
            //     thread::sleep(std::time::Duration::from_secs(clone.options.heart_beat));
            //     if *clone._deactivate.lock().unwrap() { break; }
            //     else { trace!("sprinkler[{}] heartbeat", clone.id()); }

                Ok(())
            })
        });
        ActivationResult::AsyncMonitor(tx)
    }

    fn activate_agent(&self) {
        let docker = shiplift::Docker::new();
        // let ref now = chrono::Local::now().timestamp() as u64;
        // let ref event_opts = shiplift::EventsOptions::builder().since(now).build();
        let monitor = docker
            .events(&Default::default())
            .for_each(|e| {
                if e.typ == "container" && e.action == "oom" {
                    println!("event -> {:?}", e);
                }
                Ok(())
            })
            .map_err(|e| error!("{}", e));
        tokio::run(monitor);
    }

    fn deactivate(&self) {
        *self._deactivate.lock().unwrap() = true;
    }
}