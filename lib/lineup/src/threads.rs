// Copyright © 2021 VMware, Inc. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::hash::{Hash, Hasher};
use core::{fmt, mem, ptr};

use fringe::generator::{Generator, Yielder};
use rawtime::Instant;

use crate::stack::LineupStack;
use crate::tls2::{self, ThreadControlBlock};
use crate::upcalls::Upcalls;
use crate::{CoreId, IrqVector};

/// Type alias for our generic generator.
pub(crate) type Runnable<'a> = Generator<'a, YieldResume, YieldRequest, LineupStack>;

/// The id of a thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ThreadId(pub usize);

impl fmt::Display for ThreadId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ThreadId {{ id={} }}", self.0)
    }
}

pub(crate) struct Thread {
    /// Thread ID
    pub(crate) id: ThreadId,

    /// Current core affinity of the thread.
    pub(crate) affinity: CoreId,

    /// Storage area for resume result (is thread was put in waiting list).
    pub(crate) return_with: Option<YieldResume>,

    /// If thread is registered to wake up for the specific interrupt vector.
    pub(crate) _interrupt_vector: Option<IrqVector>,

    /// Threads currently waiting (join, blocked) on us to exit.
    pub(crate) joinlist: Vec<(ThreadId, CoreId)>,

    /// Storage to remember the pointer to the TCB
    ///
    /// TODO(correctness): It's not really static (it's on the thread's stack),
    /// but keeps it easier for now.
    pub(crate) state: *mut ThreadControlBlock<'static>,
}

impl fmt::Debug for Thread {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Thread#{}", self.id.0)
    }
}

impl PartialEq for Thread {
    fn eq(&self, other: &Thread) -> bool {
        self.id.0 == other.id.0
    }
}

impl Eq for Thread {}

impl Hash for Thread {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Thread {
    /// TODO: Use builder pattern to create threads.
    #[allow(clippy::too_many_arguments)]
    pub(crate) unsafe fn new<'a, F>(
        tid: ThreadId,
        affinity: CoreId,
        stack: LineupStack,
        f: F,
        arg: *mut u8,
        upcalls: Upcalls,
        _interrupt_vector: Option<IrqVector>,
        tcb: *mut ThreadControlBlock<'static>,
    ) -> (
        Thread,
        Generator<'a, YieldResume, YieldRequest, LineupStack>,
    )
    where
        F: 'static + FnOnce(*mut u8) + Send,
    {
        // Finish initalization of TCB (except for yielder, see generator)
        (*tcb).tid = tid;
        (*tcb).current_core = affinity;
        (*tcb).upcalls = upcalls;

        let thread = Thread {
            id: tid,
            affinity,
            return_with: None,
            _interrupt_vector,
            joinlist: Vec::with_capacity(crate::scheduler::SmpScheduler::MAX_THREADS),
            state: tcb,
        };

        let generator = Generator::unsafe_new(stack, move |yielder, _| {
            tls2::Environment::thread().yielder = Some(mem::transmute::<
                &Yielder<YieldResume, YieldRequest>,
                &'static Yielder<YieldResume, YieldRequest>,
            >(yielder));

            // rump lwp switchproc stuff here
            let r = f(arg);

            // Reset TCB/TLS once thread completes
            tls2::arch::set_tcb(ptr::null_mut());

            // deallocate TLS? this shouldnt be done if the tls pointer comes from _rtld_tls_alloc
            // just ignore it for now
            //alloc::alloc::dealloc(tls_base, tls_layout);

            r
        });

        (thread, generator)
    }
}

/// Requests that go from the thread-context to the scheduler.
#[derive(Debug, PartialEq)]
pub(crate) enum YieldRequest {
    /// Just yield for now?
    None,
    /// Block thread until we reach Instant.
    Timeout(Instant),
    /// Tell scheduler to make ThreadId runnable.
    Runnable(ThreadId),
    /// Tell scheduler to make ThreadId unrunnable.
    Unrunnable(ThreadId),
    /// Make everything in the given list runnable.
    RunnableList(Vec<ThreadId>),
    /// Wait until the thread with given ID is finished.
    JoinOn(ThreadId),
    /// Spawn a new thread that runs the provided function and argument.
    Spawn(
        Option<unsafe extern "C" fn(arg1: *mut u8) -> *mut u8>,
        *mut u8,
        CoreId,
        Option<IrqVector>,
    ),
    /// Spawn a new thread that runs function/argument on the provided stack.
    SpawnWithArgs(
        LineupStack,
        Option<unsafe extern "C" fn(arg1: *mut u8) -> *mut u8>,
        *mut u8,
        CoreId,
        Option<IrqVector>,
        *mut ThreadControlBlock<'static>,
    ),
}

/// Corresponding response to a thread after we yielded back to
/// the scheduler with a request (see `YieldRequest`)
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum YieldResume {
    /// The request was completed (we immediately resumed without a context switch).
    Completed,
    /// The thread was done (and is resumed now after a context switch).
    Interrupted,
    /// A child thread was spawned with the given ThreadId.
    Spawned(ThreadId),
    /// Thread has completed (and has been removed from the scheduler state)
    DoNotResume,
}
