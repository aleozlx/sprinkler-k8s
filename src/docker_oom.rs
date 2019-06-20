use std::io::Write;
use std::sync::{Arc, Mutex, RwLock};
use std::collections::HashMap;
use tokio::prelude::*;
use super::*;

#[derive(Clone)]
pub struct DockerOOM {
    options: Arc<SprinklerOptions>,
    _deactivate: Arc<Mutex<bool>>
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum Anomaly {
    Negative,       // No anomaly has been detected
    Positive,       // Anomaly has occurred
    Fixing(usize),  // Has attempted to intervene N times
    OutOfControl    // Has given up trying because the programmed strategy will not work
}

impl Anomaly {
    pub fn get_retry_unchecked(&self) -> usize {
        match self {
            Anomaly::Negative | Anomaly::Positive => 0,
            Anomaly::Fixing(n) => *n,
            Anomaly::OutOfControl => std::usize::MAX
        }
    }

    pub fn escalate(&self, max_retry: usize) -> AnomalyTransition {
        match self {
            Anomaly::Negative => (*self >> Anomaly::Positive).unwrap(),
            Anomaly::Positive => (*self >> Anomaly::Fixing(1)).unwrap(),
            Anomaly::Fixing(n) => if *n < max_retry {
                AnomalyTransition::Fixing
            } else {
                (*self >> Anomaly::OutOfControl).unwrap()
            },
            Anomaly::OutOfControl => (*self >> Anomaly::OutOfControl).unwrap(),
        }
    }

    pub fn diminish(&self) -> AnomalyTransition {
        (*self >> Anomaly::Negative).unwrap()
    }
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum AnomalyTransition {
    Normal,         // Negative -> Negative
    Occurred,       // Negative -> Positive
    Unhandled,      // Positive -> Positive
    Disappeared,    // Positive | OutOfControl -> Negative
    Fixed,          // Fixing(_) -> Negative
    Fixing,         // Positive -> Fixing(1) || Fixing(n) -> Fixing(n+1)
    GaveUp,         // Fixing(m) -> OutOfControl
    HasGivenUp      // OutOfControl -> OutOfControl
}

pub trait ImportantExt {
    fn is_important(&self) -> bool;
}

pub trait EscalateExt {
    fn max_retry() -> usize;
    fn escalate(&mut self) -> AnomalyTransition;
}

use std::ops::Shr;
use std::ops::ShrAssign;
impl Shr for Anomaly {
    type Output = Option<AnomalyTransition>;
    fn shr(self, rhs: Self) -> Option<AnomalyTransition> {
        match (self, rhs) {
            (Anomaly::Negative, Anomaly::Negative) => Some(AnomalyTransition::Normal),
            (Anomaly::Negative, Anomaly::Positive) => Some(AnomalyTransition::Occurred),
            (Anomaly::Positive, Anomaly::Positive) => Some(AnomalyTransition::Unhandled),
            (Anomaly::Positive, Anomaly::Negative) => Some(AnomalyTransition::Disappeared),
            (Anomaly::Positive, Anomaly::Fixing(1)) => Some(AnomalyTransition::Fixing),
            (Anomaly::Fixing(i), Anomaly::Fixing(j)) if i+1==j => Some(AnomalyTransition::Fixing),
            (Anomaly::Fixing(_), Anomaly::Negative) => Some(AnomalyTransition::Fixed),
            (Anomaly::Fixing(_), Anomaly::OutOfControl) => Some(AnomalyTransition::GaveUp),
            (Anomaly::OutOfControl, Anomaly::Negative) => Some(AnomalyTransition::Disappeared),
            (Anomaly::OutOfControl, Anomaly::OutOfControl) => Some(AnomalyTransition::HasGivenUp),
            _ => None
        }
    }
}

impl Shr<AnomalyTransition> for Anomaly {
    type Output = Anomaly;
    fn shr(self, rhs: AnomalyTransition) -> Anomaly {
        match (self, rhs) {
            (Anomaly::Negative, AnomalyTransition::Occurred) => Anomaly::Positive,
            (Anomaly::Positive, AnomalyTransition::Disappeared) => Anomaly::Negative,
            (Anomaly::OutOfControl, AnomalyTransition::Disappeared) => Anomaly::Negative,
            (Anomaly::Fixing(_), AnomalyTransition::Fixed) => Anomaly::Negative,
            (Anomaly::Positive, AnomalyTransition::Fixing) => Anomaly::Fixing(1),
            (Anomaly::Fixing(n), AnomalyTransition::Fixing) => Anomaly::Fixing(n+1),
            (Anomaly::Fixing(_), AnomalyTransition::GaveUp) => Anomaly::OutOfControl,
            _ => self
        }
    }
}

impl ShrAssign<AnomalyTransition> for Anomaly {
    fn shr_assign(&mut self, rhs: AnomalyTransition) {
        let next = *self >> rhs;
        *self = next;
    }
}

pub struct EventRateMeter {
    count: usize,                        // Event counter
    t0: chrono::DateTime<chrono::Local>, // Starting point of this period
    last_rate: f32,                      // Last reading
    interval: chrono::Duration,          // Measurement interval
    pub state: Anomaly                   // aka Is this (ab)normal?
}

impl Default for EventRateMeter {
    fn default() -> Self {
        EventRateMeter {
            count: 0,
            t0: chrono::Local::now(),
            last_rate: 0f32,
            interval: chrono::Duration::seconds(1),
            state: Anomaly::Negative
        }
    }
}

impl EventRateMeter {
    /// Trigger the meter counter
    pub fn tick(&mut self) {
        self.count += 1;
        if chrono::Local::now() - self.t0 > self.interval {
            self.last_rate = self.read();
            self.count = 0;
        }
    }

    fn dt(&self) -> f32 {
        (self.interval.num_milliseconds() as f32) / 1e3
    }

    /// Compute event frequency (Hz)
    pub fn read(&self) -> f32 {
        if self.count < 6 { self.last_rate }
        else { (self.count as f32) / self.dt() }
    }
}

// TODO add a frequency divider

impl ImportantExt for AnomalyTransition {
    fn is_important(&self) -> bool {
        match self {
            AnomalyTransition::Occurred | AnomalyTransition::Disappeared | AnomalyTransition::Fixed | AnomalyTransition::GaveUp => true,
            _ => false
        }
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
        let mut meters = HashMap::new();
        meters.insert(String::from("!"), Default::default()); // Other types of message flooding
        meters.insert(String::from("."), Default::default()); // Unidentified OOM
        let meters: Arc<RwLock<HashMap<String, Mutex<EventRateMeter>>>> = Arc::new(RwLock::new(meters));

        let monitor = docker
            .events(&Default::default())
            .for_each({ let meters = meters.clone(); move |e| {
                if e.typ == "container" && e.action == "oom" {
                    if let Some(pod_name) = e.actor.attributes.get("io.kubernetes.pod.name") {
                        let need_new_meter = !meters.read().unwrap().contains_key(pod_name);
                        if need_new_meter {
                            let meter = EventRateMeter { count: 1, state: Anomaly::Fixing(1), ..Default::default() };
                            meters.write().unwrap().insert(pod_name.clone(), Mutex::new(meter));
                        }
                        else {
                            let meters = meters.read().unwrap();
                            let mut meter = meters[pod_name].lock().unwrap();
                            if meter.read() > 10.0 {

                            }
                            else {
                                let transition = meter.state.diminish();
                                match transition {
                                    AnomalyTransition::Disappeared => {
                                        // TODO notify master
                                    }
                                    AnomalyTransition::Fixed => {
                                        // TODO notify master
                                    }
                                    _ => {}
                                }
                                meter.state >>= transition;
                            }
                        }
                    }
                    else { // The container is not managed by Kubernetes
                        let meters = meters.read().unwrap();
                        let mut meter = meters["."].lock().unwrap();
                        meter.tick();
                        if meter.read() > 10.0 {
                            let transition = meter.state.escalate(20);
                            if transition == AnomalyTransition::Fixing {
                                // ? How to add delay between fixes
                                let docker = shiplift::Docker::new();
                                let container = shiplift::Container::new(&docker, e.actor.id);
                                let fut_kill = container.kill(None)  // Should send SIGKILL by default
                                    .map_err(|_| {});
                                // TODO report the kill to the master
                                tokio::spawn(fut_kill);
                            }
                            if transition.is_important() {
                                // TODO notify master
                            }
                            meter.state >>= transition;
                        }
                        else {
                            let transition = meter.state.diminish();
                            if transition.is_important() {
                                // TODO notify master
                            }
                            meter.state >>= transition;
                        }
                    }
                }
                else {
                    let meters = meters.read().unwrap();
                    let mut meter = meters["."].lock().unwrap();
                    meter.tick();
                    if meter.read() > 70.0 {
                        if let Some(transition) = meter.state >> Anomaly::Positive {
                            if transition.is_important() {
                                // TODO notify master
                            }
                            meter.state = Anomaly::Positive;
                        }
                    }
                    else {
                        let transition = meter.state.diminish();
                        if transition.is_important() {
                            // TODO notify master
                        }
                        meter.state >>= transition;
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