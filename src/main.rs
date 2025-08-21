use crossterm::{
    ExecutableCommand, cursor,
    event::{self, KeyCode, KeyEventKind, poll, read},
    execute,
    terminal::{self, ClearType, LeaveAlternateScreen},
};
use std::{
    io::{self, Stdout, Write},
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    process,
    str::FromStr,
    sync::mpsc::{self, Receiver},
    thread::{self},
    time::Duration,
};
use stun_rs::{
    self, MessageClass, MessageDecoderBuilder, MessageEncoderBuilder, StunMessageBuilder,
    attributes::stun::XorMappedAddress, methods::BINDING,
};

const DEFAULT_STUN_SERVER: &str = "stun1.l.google.com:19302";

struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        terminal::disable_raw_mode().unwrap();
        execute!(io::stdout(), LeaveAlternateScreen).expect("failed to exit alternate screen");
    }
}

fn main() {
    let sock = UdpSocket::bind("0.0.0.0:7070").expect("couldn't bind to address");

    let stun_addr = get_stun_addr(&sock, DEFAULT_STUN_SERVER);
    println!("Your address: {stun_addr}");

    let peer_addr: String = input("Peer address: ");

    thread::scope(|s| {
        let sock1 = sock.try_clone().unwrap();
        let sock2 = sock.try_clone().unwrap();

        let (msg_sender, msg_recvr) = mpsc::channel::<String>();

        sock.connect(peer_addr.clone())
            .expect("couldn't connect to peer");

        // Fetches messages sent from peer and sends to receiver for logging.
        s.spawn({
            let sock = sock1;
            let mut buf = vec![0; 256];
            sock.set_nonblocking(true).unwrap();
            move || {
                loop {
                    match sock.recv(&mut buf) {
                        Ok(size) => {
                            let peer_msg = String::from_utf8_lossy(&buf[..size]).to_string();
                            msg_sender.send(peer_msg).expect("couldn't send message");
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(80));
                        }
                        _ => {}
                    }
                }
            }
        });

        let _ = inbox_ui(
            msg_recvr,
            &sock2,
            peer_addr.to_socket_addrs().unwrap().next().unwrap(),
        )
        .unwrap();
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
    println!("received {size} bytes from server");

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

fn inbox_ui(
    msg_recvr: Receiver<String>,
    sock: &UdpSocket,
    peer_addr: SocketAddr,
) -> eyre::Result<()> {
    let mut stdout = io::stdout();

    execute!(stdout, terminal::EnterAlternateScreen).unwrap();
    terminal::enable_raw_mode()?;

    let _g = Guard;

    let mut input = String::new();
    let mut cursor = 0usize;

    print!(">");
    stdout.flush().unwrap();

    loop {
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                event::Event::Key(k) if k.kind != KeyEventKind::Release => {
                    match k.code {
                        KeyCode::Char(c) => {
                            input.insert(cursor, c);
                            cursor += 1;
                        }
                        KeyCode::Backspace if cursor > 0 => {
                            cursor -= 1;
                            input.remove(cursor);
                        }
                        KeyCode::Enter => {
                            println!("\r> {input}");

                            sock.send(input.as_bytes())?;

                            input.clear();
                            cursor = 0;
                        }
                        KeyCode::Left if cursor > 0 => cursor -= 1,
                        KeyCode::Right if cursor < input.len() => cursor += 1,
                        KeyCode::Home => cursor = 0,
                        KeyCode::End => cursor = input.len(),
                        KeyCode::Esc => process::exit(1),
                        _ => {}
                    }
                    print!("\r> {input}");
                    execute!(stdout, cursor::MoveToColumn((cursor + 2) as u16))?;
                    stdout.flush().unwrap();
                }
                _ => {}
            }
        }

        match msg_recvr.try_recv() {
            Ok(peer_msg) => {
                execute!(stdout, terminal::Clear(ClearType::CurrentLine))?;
                println!("\r\n[PEER]: {peer_msg}");
                print!("\r> {}", input);
                execute!(stdout, cursor::MoveToColumn((cursor + 2) as u16))?;
                stdout.flush()?;
            }
            Err(mpsc::TryRecvError::Empty) => continue,
            Err(mpsc::TryRecvError::Disconnected) => {
                break;
            }
        }
    }

    execute!(stdout, terminal::LeaveAlternateScreen)?;
    Ok(())
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
