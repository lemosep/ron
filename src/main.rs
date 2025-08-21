use std::{
    io::{self, Write},
    net::{IpAddr, SocketAddr, ToSocketAddrs, UdpSocket},
    str::FromStr,
};
use stun_rs::{
    self, MessageClass, MessageDecoderBuilder, MessageEncoderBuilder, StunMessageBuilder,
    attributes::stun::XorMappedAddress, methods::BINDING,
};

const DEFAULT_STUN_SERVER: &str = "stun1.l.google.com:19302";

fn main() {
    let sock = UdpSocket::bind("0.0.0.0:7070").expect("couldn't bind to address");

    let stun_addr = get_stun_addr(&sock, DEFAULT_STUN_SERVER);
    println!("Your address: {stun_addr}");

    let peer_addr: String = input("Type peer address: ");

    sock.connect(peer_addr).expect("couldn't connect to peer");
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
