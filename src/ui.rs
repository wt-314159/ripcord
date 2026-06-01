use anyhow::Result;
use std::{
    collections::VecDeque,
    io::{self, BufRead, Write},
    sync::{Mutex, RwLock, RwLockReadGuard, TryLockResult},
};

pub struct Ui {
    lock: RwLock<()>,
    queue: Mutex<VecDeque<String>>,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            lock: RwLock::new(()),
            queue: Mutex::new(VecDeque::new()),
        }
    }

    pub fn try_get_read_lock(&self) -> TryLockResult<RwLockReadGuard<'_, ()>> {
        self.lock.try_read()
    }

    pub fn println(&self, msg: &str) -> Result<()> {
        match self.lock.try_read() {
            Ok(_) => Ok(println!("{}", msg)),
            Err(_) => Ok(self
                .queue
                .lock()
                .map_err(|_| anyhow::anyhow!("Failed to acquire queue lock"))?
                .push_back(msg.to_string())),
        }
    }

    pub fn prompt(&self, message: &str) -> Result<String> {
        // Lock RwLock to prevent writing to stdout
        let lock = self
            .lock
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire lock"))?;

        let mut input = String::new();
        let stdin = io::stdin();
        let mut stdin = stdin.lock();

        print!("{}", message);
        io::stdout().flush()?;
        stdin.read_line(&mut input).ok();

        // Drop RwLock and drain queued messages
        drop(lock);
        self.drain_queue();

        Ok(input.trim().to_string())
    }

    fn drain_queue(&self) {
        let mut queue = self.queue.lock().unwrap();
        while let Some(message) = queue.pop_front() {
            println!("{}", message);
        }
    }
}
