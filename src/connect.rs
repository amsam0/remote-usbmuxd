use std::error::Error;
use std::net::{IpAddr, SocketAddr as TcpAddr};

use log::{debug, info, trace};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{unix::SocketAddr as UnixAddr, TcpStream, UnixListener, UnixStream};

use crate::{BUF_SIZE, SOCKET_LOCATION, SOCKET_LOCATION_ORIG};

#[wrap_match::wrap_match(
    success_message = "finished connecting",
    error_message = "an error occurred when connecting (caused by `{expr}` on line {line}): {error:?}",
    error_message_without_info = "an error occurred when connecting: {error:?}",
    disregard_result = true
)]
pub async fn connect(ip: IpAddr, port: u16) -> Result<(), Box<dyn Error>> {
    info!("moving {SOCKET_LOCATION} to {SOCKET_LOCATION_ORIG}");
    #[cfg(not(test_remote_usbmuxd))]
    tokio::fs::rename(SOCKET_LOCATION, SOCKET_LOCATION_ORIG).await?;

    info!("binding to {SOCKET_LOCATION} (connections will go to {ip}:{port})");
    let listener = UnixListener::bind(SOCKET_LOCATION)?;

    // ensure all users can access our new socket
    use std::os::unix::fs::PermissionsExt;
    let mut fake_perms = std::fs::metadata(SOCKET_LOCATION)?.permissions();
    fake_perms.set_mode(0o777);
    std::fs::set_permissions(SOCKET_LOCATION, fake_perms)?;

    let addr = TcpAddr::new(ip, port);

    loop {
        tokio::select! {
            socket = listener.accept() => { new_connection(socket, &addr).await }
            _ = tokio::signal::ctrl_c() => { break }
        }
    }

    drop(listener);
    info!("removing {SOCKET_LOCATION}");
    tokio::fs::remove_file(SOCKET_LOCATION).await?;

    info!("moving {SOCKET_LOCATION_ORIG} to {SOCKET_LOCATION}");
    #[cfg(not(test_remote_usbmuxd))]
    tokio::fs::rename(SOCKET_LOCATION_ORIG, SOCKET_LOCATION).await?;

    Ok(())
}

crate::connection_functions!(
    socket_ty: UnixStream,
    socket_addr_ty: UnixAddr,
    usbmuxd_addr_ty: TcpAddr,

    [new_connection]
    usbmuxd_ty: TcpStream,
    log_new: "got a new unix connection",
    log_finished: "connection has finished, cleaning up",
    log_shutdown_connection_ok: "shutdown unix connection",
    log_shutdown_connection_err: "failed to shutdown unix connection: {error}",
    log_shutdown_usbmuxd_ok: "shutdown usbmuxd through server connection",
    log_shutdown_usbmuxd_err: "failed to shutdown usbmuxd through server connection: {error}",

    [handle_read_socket]
    log_sent: "sent {size} bytes to usbmuxd through server",

    [handle_read_usbmuxd]
    log_recv: "received {size} bytes from usbmuxd through server",
);
