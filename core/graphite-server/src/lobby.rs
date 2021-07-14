use std::sync::mpsc;
use std::time::Duration;

pub struct Connection<T> {
    sender: mpsc::Sender<T>,
    receiver: mpsc::Receiver<T>,
}

impl<T> Connection<T> {
    pub fn new() -> (Self, Self) {
        let (tx1, rx1) = mpsc::channel();
        let (tx2, rx2) = mpsc::channel();
        (
            Self {
                sender: tx1,
                receiver: rx2,
            },
            Self {
                sender: tx2,
                receiver: rx1,
            },
        )
    }

    pub fn send(&self, t: T) -> Result<(), mpsc::SendError<T>> {
        self.sender.send(t)
    }

    pub fn try_recv(&self) -> Result<T, mpsc::TryRecvError> {
        self.receiver.try_recv()
    }

    pub fn recv(&self) -> Result<T, mpsc::RecvError> {
        self.receiver.recv()
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, mpsc::RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }

    pub fn iter(&self) -> mpsc::Iter<T> {
        self.receiver.iter()
    }

    pub fn try_iter(&self) -> mpsc::TryIter<T> {
        self.receiver.try_iter()
    }
}

pub struct Lobby {
    connection: Connection<String>,
}

pub struct Listener {
    connections: Vec<Connection<String>>,
}

impl Listener {
    /// Creates a new Listener with no lobbys.
    pub fn new() -> Self {
        Self {
            connections: Vec::new(),
        }
    }

    /// Creates a new Lobby that is listened to by the Listener.
    pub fn new_lobby(&mut self) -> Lobby {
        let (c1, c2) = Connection::new();
        self.connections.push(c1);
        Lobby { connection: c2 }
    }
}
