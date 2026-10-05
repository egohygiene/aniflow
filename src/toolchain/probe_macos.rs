//! Non-reaping macOS child observation through safe nix kqueue wrappers.
//!
//! XNU f6217f891ac0bb64f3d375211650a4c1ff8ca1ea: kern_event.c
//! filt_procattach holds a proc_find reference through KNOTE_ATTACH; kern_exit.c
//! drains these references before emitting NOTE_EXIT. An already-exiting child
//! instead fails registration with ESRCH. Neither path reaps the owned child.
//! See docs/toolchain-probes.md for the exclusive child-wait ownership contract.

use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;

use nix::errno::Errno;
use nix::sys::event::{EvFlags, EventFilter, FilterFlag, KEvent, Kqueue};

pub(super) enum ExitObserver {
    Events { queue: Kqueue, pid: u32, status: Option<ExitStatus> },
    /// Registration lost the race with exit. This is not an exit status.
    ExitingBeforeRegistration,
}

impl ExitObserver {
    pub(super) fn register(pid: u32) -> io::Result<Self> {
        let queue = Kqueue::new().map_err(io::Error::from)?;
        let change = KEvent::new(pid as usize, EventFilter::EVFILT_PROC,
            EvFlags::EV_ADD | EvFlags::EV_ENABLE | EvFlags::EV_ONESHOT,
            FilterFlag::NOTE_EXIT | FilterFlag::NOTE_EXITSTATUS, 0, 0);
        // An empty event list preserves any immediate exit notification for
        // poll(). Registration failures are returned as errno, not EV_ERROR.
        match queue.kevent(&[change], &mut [], Some(zero_timeout())) {
            Ok(_) => Ok(Self::Events { queue, pid, status: None }),
            Err(Errno::ESRCH) => Ok(Self::ExitingBeforeRegistration),
            Err(error) => Err(io::Error::from(error)),
        }
    }

    pub(super) fn poll(&mut self) -> io::Result<Option<ExitStatus>> {
        let Self::Events { queue, pid, status } = self else {
            return Err(io::Error::other("exit registration race requires signal-before-reap fallback"));
        };
        if status.is_some() { return Ok(*status); }
        let empty = KEvent::new(0, EventFilter::EVFILT_PROC,
            EvFlags::empty(), FilterFlag::empty(), 0, 0);
        let mut events = [empty];
        let count = match queue.kevent(&[], &mut events, Some(zero_timeout())) {
            Ok(count) => count,
            Err(Errno::EINTR) => return Ok(None),
            Err(error) => return Err(io::Error::from(error)),
        };
        if count == 0 { return Ok(None); }
        *status = Some(decode_exit(&events[0], *pid)?);
        Ok(*status)
    }
}

fn zero_timeout() -> nix::libc::timespec {
    nix::libc::timespec { tv_sec: 0, tv_nsec: 0 }
}

fn decode_exit(event: &KEvent, pid: u32) -> io::Result<ExitStatus> {
    if event.flags().contains(EvFlags::EV_ERROR) {
        return Err(io::Error::other(format!("macOS exit event error: {}", event.data())));
    }
    if event.ident() != pid as usize
        || event.filter().map_err(io::Error::from)? != EventFilter::EVFILT_PROC
        || !event.fflags().contains(FilterFlag::NOTE_EXIT | FilterFlag::NOTE_EXITSTATUS)
        || !(0..=0xffff).contains(&event.data()) {
        return Err(io::Error::other("macOS exit event lacks the exact child exit-status evidence"));
    }
    let status = ExitStatus::from_raw(event.data() as i32);
    if status.code().is_none() && status.signal().is_none() {
        return Err(io::Error::other("macOS exit event contains a nonterminal process status"));
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn exit_events_require_the_exact_child_terminal_status_and_no_error_flag() {
        let event = |pid, flags, notes, data| KEvent::new(pid, EventFilter::EVFILT_PROC,
            flags, notes, data, 0);
        let notes = FilterFlag::NOTE_EXIT | FilterFlag::NOTE_EXITSTATUS;
        assert_eq!(decode_exit(&event(42, EvFlags::EV_EOF, notes, 7 << 8), 42).unwrap().code(), Some(7));
        assert_eq!(decode_exit(&event(42, EvFlags::EV_EOF, notes, 9), 42).unwrap().signal(), Some(9));
        assert!(decode_exit(&event(43, EvFlags::EV_EOF, notes, 0), 42).is_err());
        assert!(decode_exit(&event(42, EvFlags::EV_ERROR, notes, 0), 42).is_err());
        assert!(decode_exit(&event(42, EvFlags::EV_EOF, FilterFlag::NOTE_EXIT, 0), 42).is_err());
        assert!(decode_exit(&event(42, EvFlags::EV_EOF, notes, 0x7f), 42).is_err());
    }

    #[test]
    fn registering_after_observed_exit_preserves_the_unreaped_child_status() {
        // Synthetic fixture only. The first observer establishes exit without
        // reaping, making the second registration exercise the ESRCH path.
        let mut child = Command::new("/bin/sh").args(["-c", "exit 7"]).spawn().unwrap();
        let mut first = ExitObserver::register(child.id()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if matches!(first, ExitObserver::ExitingBeforeRegistration)
                || first.poll().unwrap().is_some() { break; }
            assert!(Instant::now() < deadline, "synthetic child exit was not observed");
            thread::sleep(Duration::from_millis(1));
        }
        assert!(matches!(ExitObserver::register(child.id()).unwrap(), ExitObserver::ExitingBeforeRegistration));
        assert_eq!(child.wait().unwrap().code(), Some(7));
    }
}
