//! Synthetic isolation probe. Not exposed as an MCP or browser command.
use std::{
    env, fs, io,
    io::Write,
    net::{SocketAddr, TcpStream, UdpSocket},
    os::unix::net::{UnixListener, UnixStream},
    process::{Command, ExitCode},
    time::{Duration, Instant},
};

pub fn check(operation: &str, target: &str) -> io::Result<()> {
    match operation {
        "read" => fs::read(target).map(|_| ()),
        "write" => fs::write(target, b"brave-isole-synthetic-witness"),
        "tcp" => {
            let address: SocketAddr = target.parse().map_err(io::Error::other)?;
            TcpStream::connect_timeout(&address, Duration::from_secs(2)).map(|_| ())
        }
        "udp" => {
            let address: SocketAddr = target.parse().map_err(io::Error::other)?;
            let bind = if address.is_ipv4() {
                "127.0.0.1:0"
            } else {
                "[::1]:0"
            };
            let socket = UdpSocket::bind(bind)?;
            socket.send_to(b"brave-isole-probe", address).map(|_| ())
        }
        "unix-connect" => UnixStream::connect(target).map(|_| ()),
        "unix-serve" => {
            let server = UnixListener::bind(target)?;
            fs::set_permissions(target, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
            server.set_nonblocking(true)?;
            println!("{{\"ready\":true}}");
            io::stdout().flush()?;
            let start = Instant::now();
            let result = loop {
                match server.accept() {
                    Ok(_) => break Ok(()),
                    Err(error)
                        if error.kind() == io::ErrorKind::WouldBlock
                            && start.elapsed() < Duration::from_secs(3) =>
                    {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    Err(error) => break Err(error),
                }
            };
            drop(server);
            fs::remove_file(target)?;
            result
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unknown operation",
        )),
    }
}

pub fn run(arguments: Vec<String>, child_prefix: &[&str]) -> io::Result<ExitCode> {
    let mut args = arguments.into_iter();
    let operation = args
        .next()
        .ok_or_else(|| io::Error::other("missing operation"))?;
    if operation == "child" {
        let rest: Vec<_> = args.collect();
        if rest.first().is_none_or(|value| value == "child") {
            return Err(io::Error::other("invalid child probe"));
        }
        let status = Command::new(env::current_exe()?)
            .args(child_prefix)
            .args(rest)
            .status()?;
        return Ok(if status.success() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        });
    }
    let target = args
        .next()
        .ok_or_else(|| io::Error::other("missing target"))?;
    if args.next().is_some()
        || !["read", "write", "tcp", "udp", "unix-connect", "unix-serve"]
            .contains(&operation.as_str())
    {
        return Err(io::Error::other("invalid probe arguments"));
    }
    match check(&operation, &target) {
        Ok(()) => println!("{{\"ok\":true,\"errno\":0}}"),
        Err(error) => println!(
            "{{\"ok\":false,\"errno\":{}}}",
            error.raw_os_error().unwrap_or(-1)
        ),
    }
    Ok(ExitCode::SUCCESS)
}
