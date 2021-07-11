extern crate libc;

// standard library
use std::sync::{mpsc, Arc, Mutex};
use std::net::{SocketAddr,};
// third-party crates
use tokio::net::{TcpStream, TcpListener};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// root crates
use crate::core::ffi::ArcFFIManagerStub;
use crate::core::traits::IPCProtocol;

const VDM_CLIENT_ID_LEN:usize = 16;

pub struct IPCServer<T>
where T:IPCProtocol
{
    server_port:u16,
    ffi: ArcFFIManagerStub,
    _protocol: T,
    conns: Vec<T>
}

impl<T> IPCServer<T>
where T:IPCProtocol
{
    pub fn new(server_port:u16, ffi: ArcFFIManagerStub, _protocol:T) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(
            IPCServer{
                server_port, ffi, _protocol, conns:Vec::new()
            }
        ))
    }

    async fn try_connect(_self: Arc<Mutex<Self>>, mut socket:TcpStream) {
        let mut buf = [0; VDM_CLIENT_ID_LEN+1];
        let mut id_buf = Vec::<u8>::new();
        let (tx, rx) = mpsc::channel::< T::Message >();

        // handshake-I(a): recv id
        let n = match socket.read(&mut buf).await {
            Ok(n) => n,
            Err(e) => {
                eprintln!("hs1: failed to write to socket; err = {:?}", e);
                return
            }
        };
        id_buf.extend( buf[..n].iter().copied() );
        let _id = std::str::from_utf8(&id_buf).unwrap();
        let _protocol = {
            if let Ok(mut _self) = _self.lock() {
                let ffi = _self.ffi.clone();
                Some( T::new(format!("{}", _id), ffi) )
            } else {None}
        }.unwrap();
        

        // handshake-I(b): spawn "send" thread
        _protocol.spawn_send_thread(rx);

        // handshake-II: write back
        if let Err(e) = socket.write_all(&buf).await {
            eprintln!("hs2: failed to write to socket; err = {:?}", e);
            return;
        }

        // handshake-III: spawn "recv" thread
        if let Err(e) = socket.read(&mut buf).await {
            eprintln!("hs3: failed to read from socket; err = {:?}", e);
            return;
        }
        _protocol.spawn_recv_thread(tx);

        // record this connection
        if let Ok(mut _self) = _self.lock() {
            _self.conns.push( _protocol );
        }
    }

    pub async fn daemon(_self:Arc<Mutex<Self>>)
    {
        let sock_addr = {
            if let Ok(self_obj) = _self.lock() {
                Some(SocketAddr::new( "127.0.0.1".parse().unwrap(), self_obj.server_port ))
            } else {None}
        }.unwrap();
        let listener = TcpListener::bind(sock_addr).await.unwrap();

        loop {
            let (socket, _) = listener.accept().await.unwrap();
            let _self = _self.clone();

            tokio::spawn(async move {
                Self::try_connect(_self, socket).await
            });
        }

    }

}
