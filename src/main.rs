use std::net::{IpAddr, Ipv4Addr};

use clap::{Parser, Subcommand};
use const_format::formatcp;

mod connect;
mod serve;

#[cfg(not(any(test, test_remote_usbmuxd)))]
const SOCKET_LOCATION: &str = "/var/run/usbmuxd";
#[cfg(any(test, test_remote_usbmuxd))]
const SOCKET_LOCATION: &str = env!("SOCKET_LOCATION");
const SOCKET_LOCATION_ORIG: &str = formatcp!("{SOCKET_LOCATION}_orig");

const DEFAULT_PORT: u16 = 24801;
const DEFAULT_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));

const BUF_SIZE: usize = 16384; // 2^14, hopefully big enough for all packets

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Serves usbmuxd at the specified port and IP.
    Serve {
        #[arg(short, long, default_value_t = DEFAULT_IP)]
        /// The IP to serve usbmuxd at.
        ip: IpAddr,
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        /// The port to serve usbmuxd at.
        port: u16,
    },
    /// Connects to usbmuxd through the specified port and IP.
    Connect {
        #[arg(short, long)]
        /// The IP where usbmuxd is being served.
        ip: IpAddr,
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        /// The port where usbmuxd is being served.
        port: u16,
    },
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    init_logger();

    match args.command {
        CliCommand::Serve { ip, port } => crate::serve::serve(ip, port).await,
        CliCommand::Connect { ip, port } => crate::connect::connect(ip, port).await,
    }
}

fn init_logger() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("trace"))
        .filter(Some("mio"), log::LevelFilter::Off)
        .init();
}

// and now we present: a very very messy macro to generate the common code while still adding logging and stuff specific to serve/connect
// it's most likely possible to do this with generics and a bunch of closures but that's even more messy

#[macro_export]
macro_rules! connection_functions {
    (
        socket_ty: $socket:ty,
        socket_addr_ty: $socket_addr:ty,
        usbmuxd_addr_ty: $usbmuxd_addr:ty,

        [new_connection]
        usbmuxd_ty: $usbmuxd:ty,
        log_new: $log_new:literal,
        log_finished: $log_finished:literal,
        log_shutdown_connection_ok: $log_shutdown_connection_ok:literal,
        log_shutdown_connection_err: $log_shutdown_connection_err:literal,
        log_shutdown_usbmuxd_ok: $log_shutdown_usbmuxd_ok:literal,
        log_shutdown_usbmuxd_err: $log_shutdown_usbmuxd_err:literal,

        [handle_read_socket]
        log_sent: $log_sent:literal,

        [handle_read_usbmuxd]
        log_recv: $log_recv:literal,
    ) => {
        #[wrap_match::wrap_match(log_success = false, disregard_result = true)]
        async fn new_connection(
            socket: std::io::Result<($socket, $socket_addr)>,
            usbmuxd_addr: &$usbmuxd_addr,
        ) -> Result<(), Box<dyn Error>> {
            let (mut socket, addr) = socket?;

            // real rust programmers manually replace format specifiers /j
            debug!("{}", $log_new.replace("{addr}", &format!("{addr:?}")));
            let mut usbmuxd = <$usbmuxd>::connect(usbmuxd_addr).await?;

            tokio::spawn(async move {
                loop_read(&mut socket, &mut usbmuxd, &addr).await;
                debug!("{}", $log_finished.replace("{addr}", &format!("{addr:?}")));
                // rust without wrap-match
                match socket.shutdown().await {
                    Ok(_) => debug!(
                        "{}",
                        $log_shutdown_connection_ok.replace("{addr}", &format!("{addr:?}"))
                    ),
                    Err(e) => debug!(
                        "{}",
                        $log_shutdown_connection_err.replace("{addr}", &format!("{addr:?}")).replace("{error}", &format!("{e:?}"))
                    ),
                }
                match usbmuxd.shutdown().await {
                    Ok(_) => debug!(
                        "{}",
                        $log_shutdown_usbmuxd_ok.replace("{addr}", &format!("{addr:?}"))
                    ),
                    Err(e) => debug!(
                        "{}",
                        $log_shutdown_usbmuxd_err.replace("{addr}", &format!("{addr:?}")).replace("{error}", &format!("{e:?}"))
                    ),
                }
            });

            Ok(())
        }

        async fn loop_read(
            socket: &mut $socket,
            usbmuxd: &mut $usbmuxd,
            addr: &$socket_addr,
        ) {
            let mut socket_buf = vec![0; BUF_SIZE];
            let mut usbmuxd_buf = vec![0; BUF_SIZE];

            loop {
                tokio::select! {
                    size = socket.read(&mut socket_buf) => {
                        if let Err(_) = handle_read_socket(size, &socket_buf, usbmuxd, addr).await {
                            break
                        }
                    }
                    size = usbmuxd.read(&mut usbmuxd_buf) => {
                        if let Err(_) = handle_read_usbmuxd(size, &usbmuxd_buf, socket, addr).await {
                            break
                        }
                    }
                    _ = tokio::signal::ctrl_c() => { break }
                }
            }
        }

        #[wrap_match::wrap_match(log_success = false)]
        async fn handle_read_socket(
            size: std::io::Result<usize>,
            socket_buf: &[u8],
            usbmuxd: &mut $usbmuxd,
            addr: &$socket_addr,
        ) -> Result<(), Box<dyn Error>> {
            let size = size?;
            if size == 0 {
                return Ok(());
            }

            usbmuxd.write_all(&socket_buf[..size]).await?;
            trace!("{}", $log_sent.replace("{addr}", &format!("{addr:?}")).replace("{size}", &size.to_string()));
            Ok(())
        }

        #[wrap_match::wrap_match(log_success = false)]
        async fn handle_read_usbmuxd(
            size: std::io::Result<usize>,
            usbmuxd_buf: &[u8],
            socket: &mut $socket,
            addr: &$socket_addr,
        ) -> Result<(), Box<dyn Error>> {
            let size = size?;
            if size == 0 {
                return Ok(());
            }

            socket.write_all(&usbmuxd_buf[..size]).await?;
            trace!("{}", $log_recv.replace("{addr}", &format!("{addr:?}")).replace("{size}", &size.to_string()));
            Ok(())
        }
    };
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream, UnixListener, UnixStream},
    };

    use crate::{SOCKET_LOCATION, SOCKET_LOCATION_ORIG};

    #[tokio::test]
    async fn serve() {
        crate::init_logger();

        // Create a dummy socket that echos everything
        tokio::spawn(async {
            let listener = UnixListener::bind(SOCKET_LOCATION).expect("failed to bind to socket");

            loop {
                let (mut socket, _) = match listener.accept().await {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                tokio::spawn(async move {
                    let mut buf = vec![0; 1024];

                    loop {
                        let n = socket
                            .read(&mut buf)
                            .await
                            .expect("failed to read data from socket");

                        if n == 0 {
                            return;
                        }

                        socket
                            .write_all(&buf[0..n])
                            .await
                            .expect("failed to write data to socket");
                    }
                });
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Serve the dummy socket
        tokio::spawn(async { crate::serve::serve(crate::DEFAULT_IP, crate::DEFAULT_PORT).await });
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Connect to the dummy socket through the TCP connection we are serving and ensure it echos the bytes back
        let mut socket =
            TcpStream::connect(SocketAddr::new(crate::DEFAULT_IP, crate::DEFAULT_PORT))
                .await
                .unwrap();

        const DATA: &[u8] = &[1, 2, 3, 4];
        socket.write_all(DATA).await.unwrap();
        let mut out = [0u8; DATA.len()];
        socket.read_exact(&mut out).await.unwrap();
        assert_eq!(DATA, out);

        tokio::fs::remove_file(SOCKET_LOCATION).await.unwrap();
        tokio::fs::remove_file(SOCKET_LOCATION_ORIG).await.unwrap();
    }

    #[tokio::test]
    async fn connect() {
        crate::init_logger();

        // Create a dummy socket that echos everything
        tokio::spawn(async {
            let listener =
                TcpListener::bind(SocketAddr::new(crate::DEFAULT_IP, crate::DEFAULT_PORT))
                    .await
                    .expect("failed to bind to address");

            loop {
                let (mut socket, _) = match listener.accept().await {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                tokio::spawn(async move {
                    let mut buf = vec![0; 1024];

                    loop {
                        let n = socket
                            .read(&mut buf)
                            .await
                            .expect("failed to read data from listener");

                        if n == 0 {
                            return;
                        }

                        socket
                            .write_all(&buf[0..n])
                            .await
                            .expect("failed to write data to listener");
                    }
                });
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Connect to the dummy socket through TCP and serve it over a unix socket
        tokio::spawn(async {
            crate::connect::connect(crate::DEFAULT_IP, crate::DEFAULT_PORT).await
        });
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Connect to the dummy socket through the new unix socket and ensure it echos the bytes back
        let mut socket = UnixStream::connect(SOCKET_LOCATION).await.unwrap();

        const DATA: &[u8] = &[1, 2, 3, 4];
        socket.write_all(DATA).await.unwrap();
        let mut out = [0u8; DATA.len()];
        socket.read_exact(&mut out).await.unwrap();
        assert_eq!(DATA, out);

        tokio::fs::remove_file(SOCKET_LOCATION).await.unwrap();
    }
}
