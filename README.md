# remote-usbmuxd

A program allowing you to forward packets to/from usbmuxd to another computer, allowing the devices connected on the that computer to appear connected on your computer.

Probably similar to usbfluxd

> **Warning**
>
> I am not responsible for any damage caused by this program. All packets are transmitted as-is through TCP, there is no added encryption. (Obviously, all of the lockdown stuff will be encrypted, but
> normal messages will not be.) You are responsible for ensuring the connection between the 2 computers is secure.

## Usage

> **Warning**
>
> remote-usbmuxd is currently only partially functionally. While some simple functions such as listing devices work, it seems there are issues stopping lockdown and services from working.

remote-usbmuxd is mainly useful for developing on a remote machine.

First, build the binary for both machines.

On the machine you want to physically connect your iDevices to, run `remote-usbmuxd serve`.

On the remote machine, run `remote-usbmuxd connect --ip [IP OF SERVER/MACHINE 1]`.

## Testing

The connect and serve components have tests that you can run via `SOCKET_LOCATION=$HOME/socket cargo test <serve or connect>`.

To test both at once, follow these steps:

1. In a new terminal, run `socat UNIX-LISTEN:$HOME/socket_real -`. This is the "usbmuxd" socket
2. In a new terminal, run `SOCKET_LOCATION=$HOME/socket RUSTFLAGS=--cfg=test_remote_usbmuxd cargo run -- serve`.
3. In a new terminal, run `SOCKET_LOCATION=$HOME/socket RUSTFLAGS=--cfg=test_remote_usbmuxd cargo run -- connect --ip 0.0.0.0`.
4. In a new terminal, run `socat UNIX-CONNECT:$HOME/socket -`. This is the socket connecting to "usbmuxd"
5. Now ensure that messages mirror between the two `socat` terminals. You will encounter issues if you close and reopen the socket created in step 4. However, this does not seem to happen with
   usbmuxd.

There is currently not a cargo test for testing both.
