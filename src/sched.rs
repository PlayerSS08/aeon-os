//! Preemptive round-robin scheduler for kernel threads.
//!
//! A task's whole CPU context lives on its own kernel stack. The timer (and the voluntary-yield
//! interrupt) pushes registers, calls [`aeon_sched_irq`] with the resulting stack pointer, and resumes
//! whichever stack pointer it returns (see the stubs in `arch`). Task 0 is the boot thread and doubles
//! as the idle task: it only runs when nothing else is ready.

use core::sync::atomic::{AtomicU64, Ordering};

use crate::arch;
use crate::global::Global;
use crate::mm;

pub const MAX_TASKS: usize = 16;
const STACK_PAGES: usize = 8; // 32 KiB per task
const STACK_BYTES: usize = STACK_PAGES * 4096;
/// Stack-overflow canary. Starts as a fixed value and is replaced by a boot-seed-derived one before
/// the first task is created (`set_canary`), so it cannot be guessed from the binary.
static CANARY_VALUE: AtomicU64 = AtomicU64::new(0xAE0A_5AFE_C0DE_D00D);

pub fn set_canary(v: u64) {
    CANARY_VALUE.store(v, Ordering::Relaxed);
}

fn canary() -> u64 {
    CANARY_VALUE.load(Ordering::Relaxed)
}
const NAME_LEN: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Free,
    Ready,
    Running,
    Sleeping,
    Dead,
}

impl State {
    pub fn name(self) -> &'static str {
        match self {
            State::Free => "free",
            State::Ready => "ready",
            State::Running => "running",
            State::Sleeping => "sleeping",
            State::Dead => "dead",
        }
    }
}

#[derive(Clone, Copy)]
struct Task {
    state: State,
    rsp: u64,
    /// Lowest address of the stack allocation (0 for the boot thread), holds the overflow canary.
    stack: u64,
    wake_tick: u64,
    ticks: u64,
    name: [u8; NAME_LEN],
    name_len: usize,
}

const FREE_TASK: Task =
    Task { state: State::Free, rsp: 0, stack: 0, wake_tick: 0, ticks: 0, name: [0; NAME_LEN], name_len: 0 };

struct Sched {
    tasks: [Task; MAX_TASKS],
    current: usize,
    active: bool,
}

static SCHED: Global<Sched> = Global::new(Sched { tasks: [FREE_TASK; MAX_TASKS], current: 0, active: false });

/// Initial x87/SSE state for new tasks: FCW = 0x037F, MXCSR = 0x1F80, everything else zero.
const fn fx_template() -> [u8; 512] {
    let mut t = [0u8; 512];
    t[0] = 0x7F;
    t[1] = 0x03;
    t[24] = 0x80;
    t[25] = 0x1F;
    t
}
static FX_TEMPLATE: [u8; 512] = fx_template();

/// Register-frame layout built for a new task (low to high): fxsave area, 15 GPRs, iretq frame.
const GPR_COUNT: usize = 15;
const FRAME_QWORDS: usize = 512 / 8 + GPR_COUNT + 5;

/// Turns the running boot thread into task 0. Call once, before enabling preemption.
pub fn init() {
    arch::without_interrupts(|| {
        // SAFETY: interrupts off on a single CPU.
        let s = unsafe { SCHED.get() };
        s.tasks[0] = Task { state: State::Running, name: *b"idle\0\0\0\0\0\0\0\0\0\0\0\0", name_len: 4, ..FREE_TASK };
        s.current = 0;
        s.active = true;
    });
}

/// Gives a task stack (and its guard page) back to the frame allocator.
fn free_stack(stack: u64, guarded: bool) {
    let base = stack - 4096;
    if guarded {
        crate::paging::restore_page(base);
    }
    mm::free_frames(base, STACK_PAGES + 1);
}

extern "C" fn task_exit() -> ! {
    exit()
}

/// Creates a kernel thread running `entry(arg)`. Returns its task id.
pub fn spawn(name: &str, entry: extern "C" fn(usize), arg: usize) -> Option<usize> {
    // One extra page below the stack is unmapped as a guard, so an overflow faults immediately.
    let base = mm::alloc_frames(STACK_PAGES + 1)?;
    let guarded = crate::paging::guard_page(base);
    let stack = base + 4096;
    let top = stack + STACK_BYTES as u64;
    // The entry function starts as if it had been called: rsp = 8 (mod 16) with a return address on
    // top (task_exit, in case it returns). The saved register frame sits below that reserved area.
    let entry_rsp = top - 40;
    let sp = (top - 64 - 8 * FRAME_QWORDS as u64) & !0xF;
    // SAFETY: the frames were just allocated and are identity-mapped, writable and exclusively ours.
    unsafe {
        (stack as *mut u64).write(canary());
        // Return address for an entry function that returns: terminate the task.
        ((entry_rsp) as *mut u64).write(task_exit as *const () as u64);
        core::ptr::copy_nonoverlapping(FX_TEMPLATE.as_ptr(), sp as *mut u8, 512);
        let g = (sp + 512) as *mut u64;
        for i in 0..GPR_COUNT {
            g.add(i).write(0);
        }
        // GPR order from the lowest address: r15 r14 r13 r12 r11 r10 r9 r8 rdi rsi rbp rbx rdx rcx rax.
        g.add(8).write(arg as u64); // rdi (unused by win64, kept for clarity)
        g.add(13).write(arg as u64); // rcx: first argument in the Microsoft x64 ABI
        let f = g.add(GPR_COUNT);
        f.write(entry as *const () as u64); // rip
        f.add(1).write(arch::KERNEL_CS as u64); // cs
        f.add(2).write(0x202); // rflags: IF set
        f.add(3).write(entry_rsp); // rsp
        f.add(4).write(arch::KERNEL_DS as u64); // ss
    }

    arch::without_interrupts(|| {
        // SAFETY: interrupts off on a single CPU.
        let s = unsafe { SCHED.get() };
        let slot = s.tasks.iter().position(|t| t.state == State::Free)?;
        let mut nm = [0u8; NAME_LEN];
        let n = name.len().min(NAME_LEN);
        nm[..n].copy_from_slice(&name.as_bytes()[..n]);
        s.tasks[slot] = Task { state: State::Ready, rsp: sp, stack, wake_tick: 0, ticks: 0, name: nm, name_len: n };
        Some(slot)
    })
    .or_else(|| {
        free_stack(stack, guarded);
        None
    })
}

/// Called by the interrupt stubs with the interrupted context's stack pointer; returns the stack
/// pointer to resume. Runs with interrupts disabled.
#[no_mangle]
extern "C" fn aeon_sched_irq(sp: u64, timer: u64) -> u64 {
    if timer != 0 {
        arch::timer_tick();
    }
    // SAFETY: interrupt context on a single CPU; no other reference to SCHED is live because every
    // other access disables interrupts first.
    let s = unsafe { SCHED.get() };
    if !s.active {
        return sp;
    }
    let now = arch::ticks();
    let cur = s.current;
    s.tasks[cur].rsp = sp;
    if timer != 0 {
        s.tasks[cur].ticks += 1;
    }
    if cur != 0 && s.tasks[cur].stack != 0 {
        // SAFETY: the canary word is the first word of the task's stack allocation.
        let c = unsafe { (s.tasks[cur].stack as *const u64).read() };
        if c != canary() {
            panic!("kernel stack overflow in task {} ({})", cur, core::str::from_utf8(&s.tasks[cur].name[..s.tasks[cur].name_len]).unwrap_or("?"));
        }
    }
    if s.tasks[cur].state == State::Running {
        s.tasks[cur].state = State::Ready;
    }
    // Release stacks of finished tasks (never the one we are switching away from this instant).
    for i in 1..MAX_TASKS {
        if s.tasks[i].state == State::Dead && i != cur {
            free_stack(s.tasks[i].stack, true);
            s.tasks[i] = FREE_TASK;
        }
    }
    for t in s.tasks.iter_mut() {
        if t.state == State::Sleeping && t.wake_tick <= now {
            t.state = State::Ready;
        }
    }
    let mut next = 0;
    for k in 1..=MAX_TASKS {
        let i = (cur + k) % MAX_TASKS;
        if i != 0 && s.tasks[i].state == State::Ready {
            next = i;
            break;
        }
    }
    s.current = next;
    s.tasks[next].state = State::Running;
    s.tasks[next].rsp
}

/// Gives up the CPU until the scheduler runs this task again.
pub fn yield_now() {
    arch::yield_cpu();
}

/// Sleeps for at least `ms` milliseconds (10 ms granularity).
pub fn sleep_ms(ms: u64) {
    // SAFETY: plain read of a flag written only during init.
    if !unsafe { (*SCHED.as_ptr()).active } {
        // No scheduler (timer unavailable): wait on the TSC instead.
        let t0 = arch::rdtsc();
        let ticks = crate::log::tsc_hz() / 1000 * ms;
        while arch::rdtsc().wrapping_sub(t0) < ticks {
            arch::pause();
        }
        return;
    }
    arch::without_interrupts(|| {
        // SAFETY: interrupts off on a single CPU.
        let s = unsafe { SCHED.get() };
        let c = s.current;
        s.tasks[c].wake_tick = arch::ticks() + ms.div_ceil(1000 / arch::TIMER_HZ).max(1);
        s.tasks[c].state = State::Sleeping;
    });
    yield_now();
}

/// Ends the calling task.
pub fn exit() -> ! {
    arch::without_interrupts(|| {
        // SAFETY: interrupts off on a single CPU.
        let s = unsafe { SCHED.get() };
        let c = s.current;
        if c != 0 {
            s.tasks[c].state = State::Dead;
        }
    });
    loop {
        yield_now();
        arch::wait_for_interrupt();
    }
}

/// Terminates task `id` (not the idle task). Returns false if there is no such live task.
pub fn kill(id: usize) -> bool {
    arch::without_interrupts(|| {
        // SAFETY: interrupts off on a single CPU.
        let s = unsafe { SCHED.get() };
        if id == 0 || id >= MAX_TASKS || id == s.current || matches!(s.tasks[id].state, State::Free | State::Dead) {
            return false;
        }
        s.tasks[id].state = State::Dead;
        true
    })
}

pub fn current_id() -> usize {
    // SAFETY: plain read of one word; written only with interrupts off.
    unsafe { (*SCHED.as_ptr()).current }
}

pub struct TaskInfo<'a> {
    pub id: usize,
    pub name: &'a str,
    pub state: State,
    pub ticks: u64,
}

/// Calls `f` for every live task (with interrupts disabled, so keep `f` short).
pub fn for_each(mut f: impl FnMut(TaskInfo<'_>)) {
    arch::without_interrupts(|| {
        // SAFETY: interrupts off on a single CPU.
        let s = unsafe { SCHED.get() };
        for (id, t) in s.tasks.iter().enumerate() {
            if t.state != State::Free {
                f(TaskInfo { id, name: core::str::from_utf8(&t.name[..t.name_len]).unwrap_or("?"), state: t.state, ticks: t.ticks });
            }
        }
    });
}
