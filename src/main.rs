#[macro_use]
extern crate log;
extern crate stderrlog;
extern crate clap;
extern crate ctrlc;
extern crate ipc_channel;
use std::env;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::process::{Child,Command};
use ipc_channel::ipc::IpcSender as Sender;
use ipc_channel::ipc::IpcReceiver as Receiver;
use ipc_channel::ipc::IpcOneShotServer;
use ipc_channel::ipc::channel;
pub mod message;
pub mod oplog;
pub mod coordinator;
pub mod participant;
pub mod client;
pub mod checker;
pub mod tpcoptions;
use message::ProtocolMessage;

///
/// pub fn spawn_child_and_connect(child_opts: &mut tpcoptions::TPCOptions) -> (std::process::Child, Sender<ProtocolMessage>, Receiver<ProtocolMessage>)
///
///     child_opts: CLI options for child process
///
/// 1. Set up IPC
/// 2. Spawn a child process using the child CLI options
/// 3. Do any required communication to set up the parent / child communication channels
/// 4. Return the child process handle and the communication channels for the parent
///
/// HINT: You can change the signature of the function if necessary
///
fn spawn_child_and_connect(child_opts: &mut tpcoptions::TPCOptions, server_send: IpcOneShotServer<Sender<ProtocolMessage>>, server_send_name: String, server_send2: IpcOneShotServer<Receiver<ProtocolMessage>>, server_send_name2: String) -> (Child, Sender<ProtocolMessage>, Receiver<ProtocolMessage>) {
    let mut gen_opts:tpcoptions::TPCOptions = child_opts.clone();
    gen_opts.ipc_path = server_send_name.clone();
    gen_opts.ipc_path2 = server_send_name2.clone();
    

    let child = Command::new(env::current_exe().unwrap())
        .args(gen_opts.as_vec())
        .spawn()
        .expect("Failed to execute child process");
    
    //each server single channel
    // TODO
    
    let (_, tx): (_, Sender<ProtocolMessage>) = server_send.accept().unwrap();
    let (_, rx):(_, Receiver<ProtocolMessage>) =  server_send2.accept().unwrap();
    
    (child, tx, rx)
}

/// pub fn connect_to_coordinator(opts: &tpcoptions::TCPOptions) -> (Sender<ProtocolMessage>, Receiver<ProtocolMessage>)
///
///     opts: CLI options for this process
///
/// 1. Connect to the parent via IPC
/// 2. Do any required communication to set up the parent / child communication channels
/// 3. Return the communication channels for the child
///
/// HINT: You can change the signature of the function if necessasry
///
// fn connect_to_coordinator(opts: &tpcoptions::TPCOptions) -> (Sender<ProtocolMessage>, Receiver<ProtocolMessage>) {
//     let (tx, rx) = channel().unwrap();
//     // TODO
//     // opts.ipc_path
//     (tx, rx)
// }

//client sends as blocking

/// pub fn run(opts: &tpcoptions:TPCOptions, running: Arc<AtomicBool>)
///     opts: An options structure containing the CLI arguments
///     running: An atomically reference counted (ARC) AtomicBool(ean) that is
///         set to be false whenever Ctrl+C is pressed
///
/// 1. Creates a new coordinator
/// 2. Spawns and connects to new clients processes and then registers them with
///    the coordinator
/// 3. Spawns and connects to new participant processes and then registers them
///    with the coordinator
/// 4. Starts the coordinator protocol
/// 5. Wait until the children finish execution
///

fn run(opts: & tpcoptions::TPCOptions, running: Arc<AtomicBool>) {
    let coord_log_path = format!("{}//{}", opts.log_path, "coordinator.log");
    // TODO
    let mut coordinator = coordinator::Coordinator::new(coord_log_path, &running); 

    let mut child_opts = opts.clone();
    child_opts.mode = "client".to_string();
    
    //rust vector 


    for n in 0..opts.num_clients{
        child_opts.num = n;
        let (server_child, server_child_name) = IpcOneShotServer::new().unwrap();
        let (server_child2, server_child_name2) = IpcOneShotServer::new().unwrap();
        let (child, tx, rx) = spawn_child_and_connect(&mut child_opts, server_child, 
                                                        server_child_name, server_child2, server_child_name2);

        //connect to coordinator with client_join
        coordinator.client_join(format!("client_{}",n.to_string().clone() ), tx, rx)

        // println!("got this back from client: {:?}", a);
    }


    child_opts.mode = "participant".to_string();
    for n in 0..opts.num_participants {
        child_opts.num = n;
        let (server_child, server_child_name) = IpcOneShotServer::new().unwrap();
        let (server_child2, server_child_name2) = IpcOneShotServer::new().unwrap();
        let (child, tx, rx) = spawn_child_and_connect(&mut child_opts, server_child, 
                                                        server_child_name, server_child2, server_child_name2);

        //connect to coordinator with client_join
        coordinator.participant_join(&n.to_string(), tx, rx)

        // println!("got this back from client: {:?}", a);
    }



    coordinator.protocol();

}

/// pub fn run_client(opts: &tpcoptions:TPCOptions, running: Arc<AtomicBool>)
///     opts: An options structure containing the CLI arguments
///     running: An atomically reference counted (ARC) AtomicBool(ean) that is
///         set to be false whenever Ctrl+C is pressed
/// 1. Connects to the coordinator to get tx/rx
/// 2. Constructs a new client
/// 3. Starts the client protocol
///
fn run_client(opts: & tpcoptions::TPCOptions, running: Arc<AtomicBool>) {
    // TODO
    let client_id: String = opts.num.to_string();
    let (client_tx, client_rx):(Sender<ProtocolMessage>, Receiver<ProtocolMessage>) = channel().unwrap();
    let (parent_tx, parent_rx):(Sender<ProtocolMessage>, Receiver<ProtocolMessage>) = channel().unwrap();
 
    let tx2: Sender<Sender<ProtocolMessage>> = Sender::connect(opts.ipc_path.clone()).unwrap();
    tx2.send(client_tx).unwrap();
    let tx3: Sender<Receiver<ProtocolMessage>> = Sender::connect(opts.ipc_path2.clone()).unwrap();
    tx3.send(parent_rx).unwrap();


    let mut client = client::Client::new(format!("client_{}",client_id), running, client_rx, parent_tx);    
    client.protocol(opts.num_requests);
}

/// pub fn run_participant(opts: &tpcoptions:TPCOptions, running: Arc<AtomicBool>)
///     opts: An options structure containing the CLI arguments
///     running: An atomically reference counted (ARC) AtomicBool(ean) that is
///         set to be false whenever Ctrl+C is pressed
/// 1. Connects to the coordinator to get tx/rx
/// 2. Constructs a new participant
/// 3. Starts the participant protocol
fn run_participant(opts: & tpcoptions::TPCOptions, running: Arc<AtomicBool>) {
    let participant_id_str = format!("participant_{}", opts.num);
    let participant_log_path = format!("{}//{}.log", opts.log_path, participant_id_str);
    //#TODO
    let (participant_tx, participant_rx):(Sender<ProtocolMessage>, Receiver<ProtocolMessage>) = channel().unwrap();
    let (parent_tx, parent_rx):(Sender<ProtocolMessage>, Receiver<ProtocolMessage>) = channel().unwrap();
     
    let tx2: Sender<Sender<ProtocolMessage>> = Sender::connect(opts.ipc_path.clone()).unwrap();
    tx2.send(participant_tx).unwrap();
    let tx3: Sender<Receiver<ProtocolMessage>> = Sender::connect(opts.ipc_path2.clone()).unwrap();
    tx3.send(parent_rx).unwrap();
    
    
    let mut participant = participant::Participant::new(participant_id_str, participant_log_path, running, opts.send_success_probability,opts.operation_success_probability, participant_rx, parent_tx);
    participant.protocol();
}

fn main() {
    // Parse CLI arguments
    let opts = tpcoptions::TPCOptions::new();
    // Set-up logging and create OpLog path if necessary
    stderrlog::new()
            .module(module_path!())
            .quiet(false)
            .timestamp(stderrlog::Timestamp::Millisecond)
            .verbosity(opts.verbosity)
            .init()
            .unwrap();
    match fs::create_dir_all(opts.log_path.clone()) {
        Err(e) => error!("Failed to create log_path: \"{:?}\". Error \"{:?}\"", opts.log_path, e),
        _ => (),
    }

    // Set-up Ctrl-C / SIGINT handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let m = opts.mode.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
        if m == "run" {
            println!("\n");
        }
    }).expect("Error setting signal handler!");

    // Execute main logic
    match opts.mode.as_ref() {
        "run" => run(&opts, running),
        "client" => run_client(&opts, running),
        "participant" => run_participant(&opts, running),
        "check" => checker::check_last_run(opts.num_clients, opts.num_requests, opts.num_participants, &opts.log_path),
        _ => panic!("Unknown mode"),
    }
}
