//!
//! coordinator.rs
//! Implementation of 2PC coordinator
//!
extern crate log;
extern crate stderrlog;
extern crate rand;
extern crate ipc_channel;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
// use std::collections::HashMap;

use coordinator::ipc_channel::ipc::IpcSender as Sender;
use coordinator::ipc_channel::ipc::IpcReceiver as Receiver;
// use coordinator::ipc_channel::ipc::IpcReceiverSet as ReceiverSet;
use coordinator::ipc_channel::ipc::IpcSelectionResult;
use coordinator::ipc_channel::ipc::TryRecvError;
use coordinator::ipc_channel::ipc::channel;

use message;
use message::MessageType;
use message::ProtocolMessage;
use message::RequestStatus;
use oplog;

/// CoordinatorState
/// States for 2PC state machine
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoordinatorState {
    Quiescent,
    TakeRequests,
    ReceivedRequest,
    ProposalSent,
    ReceivedVotesAbort,
    ReceivedVotesCommit,
    SentGlobalDecision,
    Done
}

#[derive(Debug)]
pub struct Comms {
    sender: Sender<ProtocolMessage>,
    receiver: Receiver<ProtocolMessage>
}

/// Coordinator
/// Struct maintaining state for coordinator
// #[derive(Debug)]
pub struct Coordinator {
    state: CoordinatorState,
    last_state: CoordinatorState,
    running: Arc<AtomicBool>,
    log: oplog::OpLog,
    client_comms: HashMap<String, Comms>,
    num_clients: u32,
    participant_comms: HashMap<String, Comms>,
    num_participants:u32,
    client_message: message::ProtocolMessage,
    participant_message: message::ProtocolMessage,
    num_voted_commit: u32,
    action: u32,
    successful_ops: u32,
    failed_ops: u32,
    timeout: u32

}

///
/// Coordinator
/// Implementation of coordinator functionality
/// Required:
/// 1. new -- Constructor
/// 2. protocol -- Implementation of coordinator side of protocol
/// 3. report_status -- Report of aggregate commit/abort/unknown stats on exit.
/// 4. participant_join -- What to do when a participant joins
/// 5. client_join -- What to do when a client joins
///
impl Coordinator {
    /// new()
    /// Initialize a new coordinator
    /// <params>
    ///     log_path: directory for log files --> create a new log there.
    ///     r: atomic bool --> still running?
    pub fn new(
        log_path: String,
        r: &Arc<AtomicBool>) -> Coordinator {

        Coordinator {
            state: CoordinatorState::Quiescent,
            last_state: CoordinatorState::Quiescent,
            log: oplog::OpLog::new(log_path),
            running: r.clone(),
            client_comms: HashMap::new(),
            num_clients: 0,
            participant_comms: HashMap::new(),
            num_participants: 0,
            client_message: ProtocolMessage::instantiate(message::MessageType::ReadyToRecieve, 100, "coord-init".to_string(), "coord-init".to_string(), 0),
            participant_message: ProtocolMessage::instantiate(message::MessageType::ReadyToRecieve, 100, "coord-init".to_string(), "coord-init".to_string(), 0),
            num_voted_commit: 0, // TODO
            action: 0,
            successful_ops: 0,
            failed_ops: 0,
            timeout: 0


        }
    }

    /// participant_join()
    /// Adds a new participant for the coordinator to keep track of
    /// HINT: Keep track of any channels involved!
    /// HINT: You may need to change the signature of this function
    /// insert into map 
    pub fn participant_join(&mut self, name: &String, send: Sender<ProtocolMessage>, receive: Receiver<ProtocolMessage>) {
        assert!(self.state == CoordinatorState::Quiescent);
        self.num_participants = self.num_participants + 1; 
 
        let com =  Comms{sender: send, receiver: receive};
        self.participant_comms.insert(name.clone(), com);
    }

    /// client_join()
    /// Adds a new client for the coordinator to keep track of
    /// HINT: Keep track of any channels involved!
    /// HINT: You may need to change the signature of this function
    pub fn client_join(&mut self, name: String, send: Sender<ProtocolMessage>, receive: Receiver<ProtocolMessage>) {
        assert!(self.state == CoordinatorState::Quiescent);
        self.num_clients = self.num_clients + 1; 
     
        let com =  Comms{sender: send, receiver: receive};
        self.client_comms.insert(name.clone(), com);
    }

    /// report_status()
    /// Report the abort/commit/unknown status (aggregate) of all transaction
    /// requests made by this coordinator before exiting.
    pub fn report_status(&mut self) {
        // TODO: Collect actual stats
        let successful_ops: u32 = self.successful_ops;
        let failed_ops: u32 = self.failed_ops;
        let unknown_ops: u32 = 0;

        println!("coordinator:\tCommitted: {:6}\tAborted: {:6}\tUnknown: {:6}", successful_ops, failed_ops, unknown_ops);
    }

    /// protocol()
    /// Implements the coordinator side of the 2PC protocol
    /// HINT: If the simulation ends early, don't keep handling requests!
    /// HINT: Wait for some kind of exit signal before returning from the protocol!
    pub fn protocol(&mut self) {
        // TODO
        loop{
            
            if !self.running.load(Ordering::Relaxed) {
                println!("CTRL-C!");
                self.state = CoordinatorState::Done;
            } 
            
            match self.state { 
                CoordinatorState::Quiescent => {        
                    for n in 0..self.num_clients {
                        
                        let pm = message::ProtocolMessage::generate(message::MessageType::ReadyToRecieve,
                                                                    format!("coordinator_init_{}", n),
                                                                    format!("coordinator"),
                                                                    self.num_clients
                                                                    );

                        self.client_comms[&format!("client_{}",n.to_string().clone())].sender.send(pm).unwrap();
                    }

                    self.state = CoordinatorState::TakeRequests;
                    thread::sleep(Duration::from_millis(50)); 
                }

                CoordinatorState::TakeRequests => {                   
                        let mut try_counter = 0;
                        let mut n = 0;

                        loop {
                                if n == self.num_clients  {
                                    self.state = CoordinatorState::Done;
                                    break
                                }

                                match self.client_comms[&format!("client_{}",n.to_string().clone() )].receiver.try_recv() {
                                    Ok(res) => {
                                        self.client_message = res;

                                        if self.client_message.mtype == message::MessageType::ClientRequest{
                                            self.state = CoordinatorState::ReceivedRequest;
                                            break
                                        }

                                        if self.client_message.mtype == message::MessageType::NoMoreRequests {
                                            n +=1;
                                        } 


                                    },

                                    Err(_) => {
                                        try_counter += 1 ;
                                        thread::sleep(Duration::from_millis(20)); 
                                        if try_counter >3 {
                                            n += 1;
                                        }
                                    }
                                }
                        }

                }
                
                CoordinatorState::ReceivedRequest => {
                    for n in 0..self.num_participants {
                        self.participant_comms[&n.to_string()].sender.send(self.client_message.clone()).unwrap();
                    }
               
                    self.state = CoordinatorState::ProposalSent;
                    
                }

                CoordinatorState::ProposalSent => {
                    let mut try_counter = 0;
                    let mut n = 0;
                    self.num_voted_commit = 0;
                    self.timeout = 0;

                    loop{
                      
                            match self.participant_comms[&n.to_string()].receiver.try_recv() {
                                Ok(res) => {                           
                                    self.participant_message = res;

                                    n += 1;

                                    self.log.append(self.participant_message.mtype, self.participant_message.txid.clone(), self.participant_message.senderid.clone(), self.participant_message.opid.clone());


                                    if self.participant_message.mtype == message::MessageType::ParticipantVoteCommit {
                                        self.num_voted_commit += 1;
                                        
                                    }

                                    if self.num_voted_commit == self.num_participants && n == self.num_participants   {
                                        self.state = CoordinatorState::ReceivedVotesCommit;
                                        break
                                    }

                                    if self.num_voted_commit < self.num_participants && n == self.num_participants  {
                                        self.state = CoordinatorState::ReceivedVotesAbort;
                                        break;   
                                    }


                                },

                                Err(_) => {
                                    try_counter += 1 ;
                                    thread::sleep(Duration::from_millis(20)); 

                                    if try_counter >3 {
                                        self.state = CoordinatorState::ReceivedVotesAbort;
                                        self.timeout = 1;
                                        break
                                    }
                                }
                            } 
                        // }
                    }
                }
                
                CoordinatorState::ReceivedVotesAbort => {
                    //if any abort, send abort messages
               
                    
                    for n in 0..self.num_participants {
                        let mut abort_message =  ProtocolMessage::generate(MessageType::CoordinatorAbort, 
                                                                "".to_string(),
                                                                "".to_string(), 
                                                                0);
                        if self.timeout == 1 {
                            abort_message.senderid = "timeout".to_string();
                        }


                        self.participant_comms[&n.to_string()].sender.send(abort_message).unwrap();
                    }

                    self.failed_ops += 1;
                    self.log.append(MessageType::CoordinatorAbort, self.client_message.txid.to_string().clone(), self.client_message.senderid.to_string(), self.client_message.opid.clone());
                    self.state = CoordinatorState::SentGlobalDecision;
                    self.last_state = CoordinatorState::ReceivedVotesAbort;

                }
                
                CoordinatorState::ReceivedVotesCommit => {
                 //if all commit, send commit messages

                    for n in 0..self.num_participants {
                        let commit_message =  ProtocolMessage::generate(MessageType::CoordinatorCommit,
                                                                "".to_string(), 
                                                                "".to_string(), 
                                                                0);

                        self.participant_comms[&n.to_string()].sender.send(commit_message).unwrap();
                    }

                    self.successful_ops += 1;
                    self.log.append(MessageType::CoordinatorCommit, self.client_message.txid.to_string().clone(), self.client_message.senderid.to_string(), self.client_message.opid.clone());
                    self.state = CoordinatorState::SentGlobalDecision;
                    self.last_state = CoordinatorState::ReceivedVotesCommit;
                }
                
                CoordinatorState::SentGlobalDecision => {
                    //relay messages back to client
                    // println!(" - global decision");

                    if self.last_state == CoordinatorState::ReceivedVotesAbort {
                        let message =  ProtocolMessage::generate(MessageType::ClientResultAbort, 
                                                                "".to_string(), 
                                                                "".to_string(), 
                                                                0);

                        self.client_comms[&self.client_message.senderid].sender.send(message).unwrap();
                    }
                    
                    if self.last_state == CoordinatorState::ReceivedVotesCommit {
                        self.action = self.action + 1;

                        let message =  ProtocolMessage::generate(MessageType::ClientResultCommit, 
                                                                "".to_string(), 
                                                                "".to_string(), 
                                                                0);

                                                              
                        self.client_comms[&self.client_message.senderid].sender.send(message).unwrap();
                        
                       
                    }

                    self.state = CoordinatorState::TakeRequests;
                    self.num_voted_commit = 0;
                }

                CoordinatorState::Done => {

                    for n in 0..self.num_participants {
                        let pm =  ProtocolMessage::generate(MessageType::CoordinatorExit, 
                            "".to_string(), 
                            "".to_string(), 
                            0);
                        self.participant_comms[&n.to_string()].sender.send(pm).unwrap();
                    }


                    for n in 0..self.num_clients {
                        let pm =  ProtocolMessage::generate(MessageType::CoordinatorExit, 
                            "".to_string(), 
                            "".to_string(), 
                            0);

                        let client_id = format!("client_{}", n.to_string().clone());
                        self.client_comms[&client_id].sender.send(pm).unwrap();
                    }



                    thread::sleep(Duration::from_millis(1000)); 
                    break
                }
            }
        }


        // self.running
        
        self.report_status();
        thread::sleep(Duration::from_millis(500)); 
    }
}

