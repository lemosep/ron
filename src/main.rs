use std::{
    io::{self, Write},
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    str::FromStr,
    sync::mpsc::{self},
    thread::{self},
};
use stun_rs::{
    self, MessageClass, MessageDecoderBuilder, MessageEncoderBuilder, StunMessageBuilder,
    attributes::stun::XorMappedAddress, methods::BINDING,
};

const DEFAULT_STUN_SERVER: &str = "stun1.l.google.com:19302";

struct Message {
    content: String,
}

impl Message {
    fn new(content: String) -> Self {
        Message { content }
    }
}

fn main() {
    let mut sock = UdpSocket::bind("0.0.0.0:7070").expect("couldn't bind to address");

    let stun_addr = get_stun_addr(&sock, DEFAULT_STUN_SERVER);
    println!("Your address: {stun_addr}");

    let username: String = input("Username: ");
    let peer_addr: String = input("Peer address: ");

    sock.connect(peer_addr.clone())
        .expect("couldn't connect to peer");

    thread::scope(|s| {
        let sock1 = sock.try_clone().unwrap();

        let (msg_sender, msg_recvr) = mpsc::channel::<Message>();

        // Fetches messages sent from peer and sends to receiver for logging.
        s.spawn({
            sock = sock1;
            move || {
                let mut buf = vec![0; 256];
                let size = sock.recv(&mut buf).expect("couldn't receive message");

                let peer_msg = Message::new(String::from_utf8_lossy(&buf[..size]).to_string());
                msg_sender.send(peer_msg).expect("couldn't send message");
            }
        });
    });
}

fn get_stun_addr(sock: &UdpSocket, addr: impl ToSocketAddrs) -> SocketAddr {
    // Build STUN message
    let msg = StunMessageBuilder::new(BINDING, MessageClass::Request).build();

    let encoder = MessageEncoderBuilder::default().build();
    let mut buf = vec![0; 512];
    let size = encoder
        .encode(&mut buf, &msg)
        .expect("couldn't encode message");

    sock.connect(addr).expect("couldn't connect to server");
    sock.send(&buf[..size]).unwrap();

    let size = sock.recv(&mut buf).expect("couldn't receive message");

    // Send message
    let decoder = MessageDecoderBuilder::default().build();
    let (msg, dec_size) = decoder
        .decode(&buf[..size])
        .expect("couldn't decode message");
    assert_eq!(size, dec_size);

    *msg.get::<XorMappedAddress>()
        .ok_or("XorMappedAddress not found")
        .unwrap()
        .as_xor_mapped_address()
        .unwrap()
        .socket_address()
}

fn input<T>(prompt: &str) -> T
where
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Debug,
{
    print!("{}", prompt);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let input = input.trim();

    input.parse::<T>().expect("Failed to parse input")
}
