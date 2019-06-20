use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio::prelude::*;
use super::*;

#[derive(Clone)]
pub struct DockerOOM {
    options: Arc<SprinklerOptions>,
    _deactivate: Arc<Mutex<bool>>
}

#[derive(PartialEq, Debug, Copy, Clone)]
enum Anomaly {
    Negative,       // No anomaly has been detected
    Positive,       // Anomaly has occurred
    Fixing(usize),  // Has attempted to intervene N times
    OutOfControl    // Has given up trying because the programmed strategy will not work
}

#[derive(PartialEq, Debug, Copy, Clone)]
enum AnomalyTransition {
    Normal,         // Negative -> Negative
    Occurred,       // Negative -> Positive
    Unhandled,      // Positive -> Positive
    Disappeared,    // Positive | OutOfControl -> Negative
    Fixed,          // Fixing(_) -> Negative
    Fixing,         // Positive -> Fixing(1) || Fixing(n) -> Fixing(n+1)
    GaveUp,         // Fixing(m) -> OutOfControl
    HasGivenUp      // OutOfControl -> OutOfControl
}

use std::ops::Shr;
impl Shr<Anomaly> for Anomaly {
    type Output = AnomalyTransition;
    fn shr(self, rhs: Self) -> Option<Output> {
        match (self, rhs) {
            (Anomaly::Negative, Anomaly::Negative) => Some(AnomalyTransition::Normal),
            (Anomaly::Negative, Anomaly::Positive) => Some(AnomalyTransition::Occurred), // log
            (Anomaly::Positive, Anomaly::Positive) => Some(AnomalyTransition::Unhandled),
            (Anomaly::Positive, Anomaly::Negative) => Some(AnomalyTransition::Disappeared), // log
            (Anomaly::Positive, Anomaly::Fixing(1)) => Some(AnomalyTransition::Fixing),
            (Anomaly::Fixing(i), Anomaly::Fixing(j)) if i+1==j => Some(AnomalyTransition::Fixing),
            (Anomaly::Fixing(_), Anomaly::Negative) => Some(AnomalyTransition::Fixed), // log
            (Anomaly::Fixing(_), Anomaly::OutOfControl) => Some(AnomalyTransition::GaveUp), // log
            (Anomaly::OutOfControl, Anomaly::Negative) => Some(AnomalyTransition::Disappeared), // log
            (Anomaly::OutOfControl, Anomaly::OutOfControl) => Some(AnomalyTransition::HasGivenUp),
        }
    }
}

struct EventRateMeter {
    count: usize,                        // Event counter
    t0: chrono::DateTime<chrono::Local>, // Starting point of this period
    last_rate: f32,                      // Last reading
    interval: chrono::Duration,          // Measurement interval
    _state: Anomaly                       // aka Is this (ab)normal?
}

impl Default for EventRateMeter {
    fn default() -> Self {
        EventRateMeter {
            count: 0,
            t0: chrono::Local::now(),
            last_rate: 0f32,
            interval: chrono::Duration::seconds(1),
            _state: Anomaly::Negative
        }
    }
}

impl EventRateMeter {
    fn tick(&mut self) {
        self.count += 1;
        if chrono::Local::now() - self.t0 > self.interval {
            self.last_rate = self.read();
            self.count = 0;
        }
    }

    fn dt(&self) -> f32 {
        (self.interval.num_milliseconds() as f32) / 1e3
    }

    fn read(&self) -> f32 {
        if self.count == 0 { self.last_rate }
        else { (self.count as f32) / self.dt() }
    }

    fn state(&self) -> Anomaly {
        self._state
    }
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

        let meter: Arc<Mutex<EventRateMeter>> = Arc::new(Mutex::new(Default::default()));

        let monitor = docker
            .events(&Default::default())
            .for_each({ let meter = meter.clone(); move |e| {
                let meter = meter.lock();
                if e.typ == "container" && e.action == "oom" {
                    if let Some(pod_name) = e.actor.attributes.get("io.kubernetes.pod.name") {

                    }
                    else { // The container is not managed by Kubernetes
                        // TODO check rate meter state
                        let docker = shiplift::Docker::new();
                        let container = shiplift::Container::new(&docker, e.actor.id);
                        let fut_kill = container.kill(None)  // Should send SIGKILL by default
                            .map_err(|_| {});
                        tokio::spawn(fut_kill);

                        // TODO report the kill to the master
                    }
                }
                Ok(())
            }})
            .map_err(|e| error!("{}", e));
        tokio::run(monitor);
    }

    fn deactivate(&self) {
        *self._deactivate.lock().unwrap() = true;
    }
}