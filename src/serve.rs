use std::error::Error;
use std::net::{IpAddr, SocketAddr as TcpAddr};

use log::{debug, info, trace};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UnixStream};

use crate::BUF_SIZE;
#[cfg(not(test_remote_usbmuxd))]
use crate::SOCKET_LOCATION;
#[cfg(test_remote_usbmuxd)]
const SOCKET_LOCATION: &str = const_format::formatcp!("{}_real", crate::SOCKET_LOCATION);

#[wrap_match::wrap_match(
    success_message = "finished serving",
    error_message = "an error occurred when serving (caused by `{expr}` on line {line}): {error:?}",
    error_message_without_info = "an error occurred when serving: {error:?}",
    disregard_result = true
)]
pub async fn serve(ip: IpAddr, port: u16) -> Result<(), Box<dyn Error>> {
    info!("serving at {ip}:{port}, connections will go to {SOCKET_LOCATION}");

    let listener = TcpListener::bind(TcpAddr::new(ip, port)).await?;

    loop {
        tokio::select! {
            socket = listener.accept() => { new_connection(socket, &SOCKET_LOCATION).await }
            _ = tokio::signal::ctrl_c() => { break }
        }
    }

    Ok(())
}

crate::connection_functions!(
    socket_ty: TcpStream,
    socket_addr_ty: TcpAddr,
    usbmuxd_addr_ty: &str,

    [new_connection]
    usbmuxd_ty: UnixStream,
    log_new: "got a new connection from {addr}",
    log_finished: "connection from {addr} has finished, cleaning up",
    log_shutdown_connection_ok: "shutdown tcp connection for {addr}",
    log_shutdown_connection_err: "failed to shutdown tcp connection for {addr}: {error}",
    log_shutdown_usbmuxd_ok: "shutdown usbmuxd connection for {addr}",
    log_shutdown_usbmuxd_err: "failed to shutdown usbmuxd connection for {addr}: {error}",

    [handle_read_socket]
    log_sent: "sent {size} bytes to usbmuxd from {addr}",

    [handle_read_usbmuxd]
    log_recv: "sent {size} bytes to {addr} from usbmuxd",
);
