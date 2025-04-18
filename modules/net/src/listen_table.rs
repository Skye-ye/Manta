use alloc::{boxed::Box, collections::VecDeque};
use core::{
    ops::{Deref, DerefMut},
    task::Waker,
};

use log::*;
use smoltcp::{
    iface::{SocketHandle, SocketSet},
    socket::tcp::{self, State},
    wire::{IpAddress, IpEndpoint, IpListenEndpoint},
};
use systype::{SysError, SysResult};

use super::{LISTEN_QUEUE_SIZE, SOCKET_SET, SocketSetWrapper};
use crate::Mutex;

const PORT_NUM: usize = 65536;

/// An entry in the listen table, representing a specific listening endpoint.
///
/// This struct holds the information related to a specific listening IP address
/// and port. It also manages the SYN queue and the waker for handling incoming
/// TCP connections.
struct ListenTableEntry {
    /// The IP address and port being listened on.
    listen_endpoint: IpListenEndpoint,
    /// The SYN queue holding incoming TCP connection handles.
    syn_queue: VecDeque<SocketHandle>,
    /// The waker used to wake up the listening socket when a new connection
    /// arrives.
    waker: Waker,
}

impl ListenTableEntry {
    pub fn new(listen_endpoint: IpListenEndpoint, waker: &Waker) -> Self {
        Self {
            listen_endpoint,
            syn_queue: VecDeque::with_capacity(LISTEN_QUEUE_SIZE),
            waker: waker.clone(),
        }
    }

    #[inline]
    /// Linux内核有一个特殊的机制，叫做 IPv4-mapped IPv6
    /// addresses，允许IPv6套接字接收IPv4连接
    ///
    /// 1. 当IPv6套接字绑定到::（全0地址）时，
    ///    内核会允许该套接字接受任何传入的连接，无论其是IPv4还是IPv6地址。
    /// 2. 对于从IPv4地址到来的连接，内核会将其转换为IPv4-mapped
    ///    IPv6地址，即::ffff:a.b.c.d格式，其中a.b.c.d是IPv4地址。
    fn can_accept(&self, dst: IpAddress) -> bool {
        match self.listen_endpoint.addr {
            Some(addr) => {
                if addr == dst {
                    return true;
                }
                if let IpAddress::Ipv6(v6) = addr {
                    if v6.is_unspecified()
                        || (dst.as_bytes().len() == 4
                            && v6.is_ipv4_mapped()
                            && v6.as_bytes()[12..] == dst.as_bytes()[..])
                    {
                        return true;
                    }
                }
                false
            }
            None => true,
        }
    }

    pub fn wake(self) {
        self.waker.wake_by_ref()
    }
}

impl Drop for ListenTableEntry {
    fn drop(&mut self) {
        for &handle in &self.syn_queue {
            SOCKET_SET.remove(handle);
        }
    }
}

/// A table for managing TCP listen ports.
/// Each index corresponds to a specific port number.
///
/// Using an array allows direct access to the corresponding listen entry
/// through the port number, improving lookup efficiency.
/// A Mutex ensures thread safety, as multiple threads may access and modify
/// the state of the listening ports in a multithreaded environment.
pub struct ListenTable {
    /// An array of Mutexes, each protecting an optional ListenTableEntry for a
    /// specific port.
    tcp: Box<[Mutex<Option<Box<ListenTableEntry>>>]>,
}

impl ListenTable {
    pub fn new() -> Self {
        let tcp = unsafe {
            let mut buf = Box::new_uninit_slice(PORT_NUM);
            for i in 0..PORT_NUM {
                buf[i].write(Mutex::new(None));
            }
            buf.assume_init()
        };
        Self { tcp }
    }

    pub fn can_listen(&self, port: u16) -> bool {
        self.tcp[port as usize].lock().is_none()
    }

    pub fn listen(&self, listen_endpoint: IpListenEndpoint, waker: &Waker) -> SysResult<()> {
        let port = listen_endpoint.port;
        assert_ne!(port, 0);
        let mut entry = self.tcp[port as usize].lock();
        if entry.is_none() {
            *entry = Some(Box::new(ListenTableEntry::new(listen_endpoint, waker)));
            Ok(())
        } else {
            warn!("socket listen() failed");
            Err(SysError::EADDRINUSE)
        }
    }

    pub fn unlisten(&self, port: u16) {
        info!("TCP socket unlisten on {}", port);
        if let Some(entry) = self.tcp[port as usize].lock().take() {
            entry.wake()
        }
        // *self.tcp[port as usize].lock() = None;
    }

    pub fn can_accept(&self, port: u16) -> bool {
        if let Some(entry) = self.tcp[port as usize].lock().deref() {
            entry.syn_queue.iter().any(|&handle| is_connected(handle))
        } else {
            // 因为在listen函数调用时已经将port设为监听状态了，这里应该不会查不到？？
            error!("socket accept() failed: not listen. I think this wouldn't happen !!!");
            false
            // Err(SysError::EINVAL)
        }
    }

    /// 检查端口上的SYN队列，找到已经建立连接的句柄，并将其从队列中取出，
    /// 返回给调用者。
    pub fn accept(&self, port: u16) -> SysResult<(SocketHandle, (IpEndpoint, IpEndpoint))> {
        if let Some(entry) = self.tcp[port as usize].lock().deref_mut() {
            let syn_queue = &mut entry.syn_queue;
            let (idx, addr_tuple) = syn_queue
                .iter()
                .enumerate()
                .find_map(|(idx, &handle)| {
                    is_connected(handle).then(|| (idx, get_addr_tuple(handle)))
                })
                .ok_or(SysError::EAGAIN)?; // wait for connection

            // 记录慢速SYN队列遍历的警告信息是为了监控和诊断性能问题
            // 理想情况: 如果网络连接正常，
            // SYN队列中的连接请求应尽快完成三次握手并从队列前端被取出。因此，
            // 最常见的情况是已连接的句柄在队列的前端，即索引为0。
            // 异常情况: 如果队列中第一个元素（索引为0）的连接请求没有完成，
            // 而后续的某个连接请求已经完成，这可能表明存在性能问题或异常情况,如网络延迟、
            // 资源争用
            if idx > 0 {
                warn!(
                    "slow SYN queue enumeration: index = {}, len = {}!",
                    idx,
                    syn_queue.len()
                );
            }
            let handle = syn_queue.swap_remove_front(idx).unwrap();
            Ok((handle, addr_tuple))
        } else {
            warn!("socket accept() failed: not listen");
            Err(SysError::EINVAL)
        }
    }

    pub fn incoming_tcp_packet(
        &self,
        src: IpEndpoint,
        dst: IpEndpoint,
        sockets: &mut SocketSet<'_>,
    ) {
        if let Some(entry) = self.tcp[dst.port as usize].lock().deref_mut() {
            if !entry.can_accept(dst.addr) {
                // not listening on this address
                warn!(
                    "[ListenTable::incoming_tcp_packet] not listening on address {}",
                    dst.addr
                );
                return;
            }
            if entry.syn_queue.len() >= LISTEN_QUEUE_SIZE {
                // SYN queue is full, drop the packet
                warn!("SYN queue overflow!");
                return;
            }
            entry.waker.wake_by_ref();
            info!(
                "[ListenTable::incoming_tcp_packet] wake the socket who listens port {}",
                dst.port
            );
            let mut socket = SocketSetWrapper::new_tcp_socket();
            if socket.listen(entry.listen_endpoint).is_ok() {
                let handle = sockets.add(socket);
                info!(
                    "TCP socket {}: prepare for connection {} -> {}",
                    handle, src, entry.listen_endpoint
                );
                entry.syn_queue.push_back(handle);
            }
        }
    }
}

fn is_connected(handle: SocketHandle) -> bool {
    SOCKET_SET.with_socket::<tcp::Socket, _, _>(handle, |socket| {
        !matches!(socket.state(), State::Listen | State::SynReceived)
    })
}

fn get_addr_tuple(handle: SocketHandle) -> (IpEndpoint, IpEndpoint) {
    SOCKET_SET.with_socket::<tcp::Socket, _, _>(handle, |socket| {
        (
            socket.local_endpoint().unwrap(),
            socket.remote_endpoint().unwrap(),
        )
    })
}
