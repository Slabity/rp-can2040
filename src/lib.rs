#![no_std]

pub use rp_can2040_sys as sys;

#[cfg(feature = "embedded-can")]
use core::sync::atomic::{AtomicUsize, Ordering};
use sys::can2040_msg__bindgen_ty_1;

/// Default system clock frequency in Hz for the target chip.
/// Pass to [`Can2040::start`] or [`Can2040::reset`] when running at the
/// standard clock rate.
#[cfg(feature = "rp2040")]
pub const DEFAULT_SYS_FREQ: u32 = 125_000_000;
/// Default system clock frequency in Hz for the target chip.
/// Pass to [`Can2040::start`] or [`Can2040::reset`] when running at the
/// standard clock rate.
#[cfg(feature = "rp2350")]
pub const DEFAULT_SYS_FREQ: u32 = 150_000_000;

const ID_RTR: u32 = sys::CAN2040_ID_RTR as u32;
const ID_EFF: u32 = sys::CAN2040_ID_EFF as u32;
const EXTENDED_ID_MASK: u32 = 0x1FFF_FFFF;

/// Errors reported by the CAN bus.
///
/// can2040's `NOTIFY_ERROR` notification does not distinguish between error
/// types, so only a single variant is possible.
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum CanError {
    Overrun,
}

#[cfg(feature = "embedded-can")]
impl embedded_can::Error for CanError {
    fn kind(&self) -> embedded_can::ErrorKind {
        embedded_can::ErrorKind::Overrun
    }
}

/// A CAN bus frame.
///
/// The raw `id` field follows the can2040 convention:
/// - Bit 31 (`ID_EFF`): set for extended (29-bit) frames.
/// - Bit 30 (`ID_RTR`): set for remote frames.
/// - Bits 28–0: 29-bit extended ID, or bits 10–0: 11-bit standard ID.
#[derive(Clone, Copy)]
pub struct CanFrame(sys::can2040_msg);

impl CanFrame {
    /// Constructs a frame from a raw can2040 id word and data payload.
    /// `dlc` is derived from `data.len()`. Returns `None` if `data` exceeds 8 bytes.
    pub fn new(id: u32, data: &[u8]) -> Option<Self> {
        if data.len() > 8 {
            return None;
        }
        let mut arr = [0u8; 8];
        arr[..data.len()].copy_from_slice(data);
        Some(Self(sys::can2040_msg {
            id,
            dlc: data.len() as u32,
            __bindgen_anon_1: can2040_msg__bindgen_ty_1 { data: arr },
        }))
    }

    /// Constructs a frame with an explicit dlc (0–15).
    /// Per the CAN spec, dlc values 8–15 all carry 8 data bytes, so for
    /// dlc ≥ 8 exactly 8 data bytes must be provided; for dlc < 8 exactly
    /// `dlc` data bytes must be provided. Returns `None` on any mismatch.
    pub fn new_with_dlc(id: u32, dlc: u32, data: &[u8]) -> Option<Self> {
        if dlc > 15 {
            return None;
        }
        if data.len() != (dlc as usize).min(8) {
            return None;
        }
        let mut arr = [0u8; 8];
        arr[..data.len()].copy_from_slice(data);
        Some(Self(sys::can2040_msg {
            id,
            dlc,
            __bindgen_anon_1: can2040_msg__bindgen_ty_1 { data: arr },
        }))
    }

    /// Full raw can2040 id word, including EFF/RTR flag bits.
    pub fn raw_id(&self) -> u32 {
        self.0.id
    }

    /// Frame arbitration ID with flag bits masked out.
    pub fn arb_id(&self) -> u32 {
        if self.0.id & ID_EFF != 0 {
            self.0.id & EXTENDED_ID_MASK
        } else {
            self.0.id & 0x7FF
        }
    }

    /// Returns `true` if this is an extended (29-bit) ID frame.
    pub fn is_extended(&self) -> bool {
        self.0.id & ID_EFF != 0
    }

    /// Returns `true` if this is a remote transmission request (RTR) frame.
    pub fn is_remote(&self) -> bool {
        self.0.id & ID_RTR != 0
    }

    /// Raw DLC field value (0–15).
    ///
    /// Per the CAN spec, DLC values 9–15 are valid but all carry exactly 8
    /// data bytes. [`data`](Self::data) always returns `min(dlc, 8)` bytes,
    /// so `dlc()` and `data().len()` may differ for DLC > 8.
    pub fn dlc(&self) -> usize {
        self.0.dlc as usize
    }

    pub fn data(&self) -> &[u8] {
        // dlc > 8 is valid per CAN spec but the data array is only 8 bytes
        let len = (self.0.dlc as usize).min(8);
        unsafe { &self.0.__bindgen_anon_1.data[..len] }
    }
}

impl core::fmt::Debug for CanFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "CanFrame {{ id: {:#x}, extended: {}, remote: {}, data: {:x?} }}",
            self.arb_id(),
            self.is_extended(),
            self.is_remote(),
            self.data()
        )
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for CanFrame {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(
            f,
            "CanFrame {{ id: {:#x}, extended: {=bool}, remote: {=bool}, data: {:x} }}",
            self.arb_id(),
            self.is_extended(),
            self.is_remote(),
            self.data()
        );
    }
}

#[cfg(feature = "embedded-can")]
impl embedded_can::Frame for CanFrame {
    fn new(id: impl Into<embedded_can::Id>, data: &[u8]) -> Option<Self> {
        if data.len() > 8 {
            return None;
        }
        let raw_id = match id.into() {
            embedded_can::Id::Standard(id) => id.as_raw() as u32,
            embedded_can::Id::Extended(id) => id.as_raw() | ID_EFF,
        };
        let mut arr = [0u8; 8];
        arr[..data.len()].copy_from_slice(data);
        Some(Self(sys::can2040_msg {
            id: raw_id,
            dlc: data.len() as u32,
            __bindgen_anon_1: can2040_msg__bindgen_ty_1 { data: arr },
        }))
    }

    fn new_remote(id: impl Into<embedded_can::Id>, dlc: usize) -> Option<Self> {
        if dlc > 15 {
            return None;
        }
        let raw_id = match id.into() {
            embedded_can::Id::Standard(id) => id.as_raw() as u32 | ID_RTR,
            embedded_can::Id::Extended(id) => id.as_raw() | ID_EFF | ID_RTR,
        };
        Some(Self(sys::can2040_msg {
            id: raw_id,
            dlc: dlc as u32,
            __bindgen_anon_1: can2040_msg__bindgen_ty_1 { data: [0u8; 8] },
        }))
    }

    fn is_extended(&self) -> bool {
        self.is_extended()
    }

    fn is_remote_frame(&self) -> bool {
        self.is_remote()
    }

    fn id(&self) -> embedded_can::Id {
        if self.is_extended() {
            embedded_can::Id::Extended(
                embedded_can::ExtendedId::new(self.raw_id() & EXTENDED_ID_MASK).unwrap(),
            )
        } else {
            embedded_can::Id::Standard(
                embedded_can::StandardId::new((self.raw_id() & 0x7FF) as u16).unwrap(),
            )
        }
    }

    fn dlc(&self) -> usize {
        self.dlc()
    }

    fn data(&self) -> &[u8] {
        if self.is_remote() {
            return &[];
        }
        self.data()
    }
}

/// Event delivered to the user callback from interrupt context.
pub enum Notification {
    Rx(CanFrame),
    Tx(CanFrame),
    Error,
}

/// User-provided callback invoked from interrupt context on each CAN event.
///
/// **Must be ISR-safe**: no blocking, no allocations, no non-reentrant
/// operations. Typical implementations write to a lock-free queue or signal an
/// `embassy_sync::channel::Channel`.
pub type CanCallback = fn(Notification);

/// Snapshot of can2040 bus counters.
///
/// Counters are only reset by [`Can2040::new`]. To compute activity over a
/// discrete period, subtract two snapshots; wrapping subtraction handles
/// 32-bit counter rollovers correctly.
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CanStatistics {
    pub rx_total: u32,
    pub tx_total: u32,
    pub tx_attempt: u32,
    pub parse_error: u32,
}

impl core::ops::Sub for CanStatistics {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            rx_total: self.rx_total.wrapping_sub(rhs.rx_total),
            tx_total: self.tx_total.wrapping_sub(rhs.tx_total),
            tx_attempt: self.tx_attempt.wrapping_sub(rhs.tx_attempt),
            parse_error: self.parse_error.wrapping_sub(rhs.parse_error),
        }
    }
}

/// SPSC ring buffer written by the ISR callback and read by `receive()`.
///
/// Uses wrapping `usize` head/tail indices so capacity is exactly `N` slots.
/// `N` must be > 0.
#[cfg(feature = "embedded-can")]
struct RxQueue<const N: usize> {
    buf: core::cell::UnsafeCell<[core::mem::MaybeUninit<CanFrame>; N]>,
    head: AtomicUsize,
    tail: AtomicUsize,
}

#[cfg(feature = "embedded-can")]
unsafe impl<const N: usize> Sync for RxQueue<N> {}
#[cfg(feature = "embedded-can")]
unsafe impl<const N: usize> Send for RxQueue<N> {}

#[cfg(feature = "embedded-can")]
impl<const N: usize> RxQueue<N> {
    fn new() -> Self {
        assert!(N > 0, "RxQueue capacity N must be > 0");
        Self {
            // Safety: [MaybeUninit<T>; N] has no validity invariant.
            buf: core::cell::UnsafeCell::new(unsafe {
                core::mem::MaybeUninit::<[core::mem::MaybeUninit<CanFrame>; N]>::uninit()
                    .assume_init()
            }),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Called from ISR (single producer). Drops the frame silently if full.
    fn push(&self, frame: CanFrame) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        if tail.wrapping_sub(head) >= N {
            return false;
        }
        // Safety: SPSC — producer exclusively owns slot `tail % N`.
        unsafe {
            (*self.buf.get())[tail % N].write(frame);
        }
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        true
    }

    /// Called from main context (single consumer).
    fn pop(&self) -> Option<CanFrame> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        // Safety: SPSC — consumer exclusively owns slot `head % N`.
        let frame = unsafe { (*self.buf.get())[head % N].assume_init_read() };
        self.head.store(head.wrapping_add(1), Ordering::Release);
        Some(frame)
    }
}

// repr(C) with `cbus` as the first field lets us recover a pointer to
// Can2040State from a *mut can2040 inside the C callback. The C library passes
// `cd` (== &state.cbus) back into the callback, and since Can2040State is
// repr(C) with cbus first, casting cd to *mut Can2040State is valid.
#[repr(C)]
struct Can2040State {
    cbus: sys::can2040, // must remain the first field — enforced by assert below
    pio_num: u32,
    callback: CanCallback,
    // Monomorphised function pointer set by Can2040::new(). dispatch_callback
    // calls this to push received frames into the generic RxQueue<N> without
    // Can2040State itself needing to be generic.
    #[cfg(feature = "embedded-can")]
    rx_enqueue: unsafe fn(*mut Can2040State, CanFrame),
}

/// Handle to an initialized CAN bus.
///
/// This type owns all CAN state. There are no library-level globals: the
/// caller is responsible for storage and for routing the correct PIO interrupt
/// to [`Can2040::on_irq`].
///
/// The const generic `N` controls the depth of the internal receive buffer used
/// by the [`embedded_can::nb::Can`] and [`embedded_can::blocking::Can`] trait
/// impls (only present with the `embedded-can` feature). Capacity is exactly
/// `N` frames. When the feature is inactive, `N` has no effect and may be
/// omitted (the default of 8 is used).
///
/// # Thread safety and reentrancy
///
/// The can2040 C library is not reentrant. [`start`](Self::start),
/// [`stop`](Self::stop), [`reset`](Self::reset), [`transmit`](Self::transmit),
/// and [`on_irq`](Self::on_irq) must not be called concurrently from multiple
/// cores or from within the callback. On a dual-core system, protect `Can2040`
/// with a mutex and access it from one context at a time.
///
/// [`check_transmit`](Self::check_transmit) and [`statistics`](Self::statistics)
/// are exceptions: the C library explicitly documents both as safe to call from
/// another core while the interrupt handler is running.
///
/// # Example interrupt handler (bare-metal)
/// ```ignore
/// static CAN: Mutex<RefCell<Option<Can2040>>> = Mutex::new(RefCell::new(None));
///
/// #[interrupt]
/// fn PIO0_IRQ_0() {
///     cortex_m::interrupt::free(|cs| {
///         if let Some(can) = CAN.borrow(cs).borrow_mut().as_mut() {
///             can.on_irq();
///         }
///     });
/// }
/// ```
const _: () = assert!(
    core::mem::offset_of!(Can2040State, cbus) == 0,
    "cbus must be the first field of Can2040State for the C callback pointer cast to be valid",
);

// repr(C) with `state` first is required so that `enqueue_rx<N>` can recover
// a *mut Can2040<N> from the *mut Can2040State that dispatch_callback holds.
#[repr(C)]
pub struct Can2040<const N: usize = 8> {
    state: Can2040State, // must remain the first field — enforced by assert below
    #[cfg(feature = "embedded-can")]
    rx_queue: RxQueue<N>,
}

const _: () = assert!(
    core::mem::offset_of!(Can2040<8>, state) == 0,
    "state must be the first field of Can2040 for enqueue_rx pointer recovery to be valid",
);

// Can2040 contains *mut c_void (pio_hw) which is not Send by default.
// Safety: all PIO hardware access goes through the can2040 C library which is
// not thread-safe; the caller is responsible for exclusive access (e.g. a
// Mutex). Marking Send allows storing Can2040 in a static Mutex.
unsafe impl<const N: usize> Send for Can2040<N> {}

impl<const N: usize> Can2040<N> {
    /// Prepares the CAN peripheral but does not start it.
    ///
    /// After calling this, unmask the appropriate `PIOx_IRQ_0` interrupt and
    /// ensure it calls [`on_irq`](Self::on_irq), then call [`start`](Self::start).
    ///
    /// # Arguments
    /// - `pio_num`: PIO block to use. RP2040: 0–1. RP2350: 0–2.
    /// - `callback`: called from interrupt context on every RX, TX, and error event.
    ///
    /// # Panics
    /// Panics if `pio_num` is out of range for the target.
    pub fn new(pio_num: u32, callback: CanCallback) -> Self {
        #[cfg(feature = "rp2040")]
        assert!(pio_num <= 1, "RP2040 supports PIO0 and PIO1 only");
        #[cfg(feature = "rp2350")]
        assert!(pio_num <= 2, "RP2350 supports PIO0, PIO1, and PIO2 only");

        let mut can = Self {
            state: Can2040State {
                cbus: sys::can2040::default(),
                pio_num,
                callback,
                #[cfg(feature = "embedded-can")]
                rx_enqueue: enqueue_rx::<N>,
            },
            #[cfg(feature = "embedded-can")]
            rx_queue: RxQueue::new(),
        };

        unsafe {
            let ptr = &mut can.state.cbus as *mut _;
            sys::can2040_setup(ptr, pio_num);
            sys::can2040_callback_config(ptr, Some(dispatch_callback));
        }

        can
    }

    /// Starts the CAN bus. May also be called after [`stop`](Self::stop) to
    /// restart without clearing the transmit queue. Use [`reset`](Self::reset)
    /// if the queue must be cleared.
    ///
    /// # Arguments
    /// - `sys_clock`: system clock in Hz (use [`DEFAULT_SYS_FREQ`] for the board default).
    /// - `baud_rate`: CAN bit rate in bits per second (e.g. `500_000`).
    /// - `gpio_rx`: GPIO pin for CAN RX.
    /// - `gpio_tx`: GPIO pin for CAN TX. Pass `-1` for receive-only (silent)
    ///   mode — the TX GPIO will not be driven, so the caller must ensure the
    ///   transceiver TXD pin is held high (recessive) externally.
    pub fn start(&mut self, sys_clock: u32, baud_rate: u32, gpio_rx: i32, gpio_tx: i32) {
        unsafe {
            sys::can2040_start(
                &mut self.state.cbus as *mut _,
                sys_clock,
                baud_rate,
                gpio_rx,
                gpio_tx,
            );
        }
    }

    /// Stops the bus, clears the transmit queue, then restarts.
    ///
    /// Use this instead of [`stop`](Self::stop) + [`start`](Self::start) when
    /// the TX queue needs to be cleared before resuming. Statistics counters
    /// are also reset to zero.
    ///
    /// The PIO interrupt **must be masked** before calling this (same requirement
    /// as [`stop`](Self::stop)).
    pub fn reset(&mut self, sys_clock: u32, baud_rate: u32, gpio_rx: i32, gpio_tx: i32) {
        unsafe {
            let ptr = &mut self.state.cbus as *mut _;
            sys::can2040_stop(ptr);
            sys::can2040_setup(ptr, self.state.pio_num);
            sys::can2040_callback_config(ptr, Some(dispatch_callback));
            sys::can2040_start(ptr, sys_clock, baud_rate, gpio_rx, gpio_tx);
        }
    }

    /// Must be called from the `PIOx_IRQ_0` interrupt handler for the PIO
    /// chosen at construction.
    pub fn on_irq(&mut self) {
        unsafe {
            sys::can2040_pio_irq_handler(&mut self.state.cbus as *mut _);
        }
    }

    /// Stops the CAN bus and disables the PIO state machines.
    ///
    /// The PIO interrupt **must be masked** before calling this to prevent
    /// spurious calls to [`on_irq`](Self::on_irq).
    ///
    /// The transmit queue is **not** cleared by `stop`. Call
    /// [`reset`](Self::reset) instead if the queue must be empty before
    /// resuming.
    ///
    /// If a frame is queued for transmission at the time `stop` is called, it
    /// may be transmitted successfully without a corresponding `NOTIFY_TX`
    /// callback.
    pub fn stop(&mut self) {
        unsafe {
            sys::can2040_stop(&mut self.state.cbus as *mut _);
        }
    }

    /// Queues a frame for transmission.
    ///
    /// Returns `Err(WouldBlock)` if the transmit queue (4 frames) is full.
    /// Frames are transmitted in FIFO order with no priority-based reordering.
    ///
    /// May be called from within the [`CanCallback`], though doing so is
    /// discouraged as it adds latency to the interrupt handler.
    pub fn transmit(&mut self, frame: &CanFrame) -> nb::Result<(), CanError> {
        let ret = unsafe {
            sys::can2040_transmit(&mut self.state.cbus as *mut _, &frame.0 as *const _ as *mut _)
        };
        if ret < 0 {
            Err(nb::Error::WouldBlock)
        } else {
            Ok(())
        }
    }

    /// Returns `true` if there is space in the transmit queue.
    ///
    /// Safe to call from another core while the interrupt handler is running.
    /// The result is an instantaneous snapshot; the queue may fill between
    /// this check and a subsequent [`transmit`](Self::transmit) call.
    /// `transmit` will return `Err(WouldBlock)` if that happens.
    pub fn check_transmit(&self) -> bool {
        unsafe { sys::can2040_check_transmit(&self.state.cbus as *const _ as *mut _) != 0 }
    }

    /// Returns a snapshot of the current bus counters.
    ///
    /// Safe to call from another core while the interrupt handler is running.
    ///
    /// To compute activity over a period, subtract two snapshots:
    /// ```ignore
    /// let delta = can.statistics() - prev;
    /// ```
    pub fn statistics(&self) -> CanStatistics {
        let mut raw =
            sys::can2040_stats { rx_total: 0, tx_total: 0, tx_attempt: 0, parse_error: 0 };
        unsafe {
            sys::can2040_get_statistics(&self.state.cbus as *const _ as *mut _, &mut raw as *mut _);
        }
        CanStatistics {
            rx_total: raw.rx_total,
            tx_total: raw.tx_total,
            tx_attempt: raw.tx_attempt,
            parse_error: raw.parse_error,
        }
    }
}

#[cfg(feature = "embedded-can")]
impl<const N: usize> embedded_can::nb::Can for Can2040<N> {
    type Frame = CanFrame;
    type Error = CanError;

    /// Queues a frame for transmission.
    ///
    /// can2040 does not support priority-based frame replacement, so the
    /// displaced-frame slot is always `None`. Returns `Err(WouldBlock)` when
    /// the 4-frame hardware queue is full.
    fn transmit(&mut self, frame: &CanFrame) -> nb::Result<Option<CanFrame>, CanError> {
        self.transmit(frame).map(|()| None)
    }

    /// Returns the next received frame, or `Err(WouldBlock)` if none is available.
    fn receive(&mut self) -> nb::Result<CanFrame, CanError> {
        self.rx_queue.pop().ok_or(nb::Error::WouldBlock)
    }
}

#[cfg(feature = "embedded-can")]
impl<const N: usize> embedded_can::blocking::Can for Can2040<N> {
    type Frame = CanFrame;
    type Error = CanError;

    /// Spins until there is space in the transmit queue, then queues the frame.
    fn transmit(&mut self, frame: &CanFrame) -> Result<(), CanError> {
        loop {
            match self.transmit(frame) {
                Ok(()) => return Ok(()),
                Err(nb::Error::WouldBlock) => core::hint::spin_loop(),
                Err(nb::Error::Other(e)) => return Err(e),
            }
        }
    }

    /// Spins until a frame is received.
    ///
    /// # Warning
    ///
    /// This will spin indefinitely if the bus is silent. On bare-metal with no
    /// preemption, that is a permanent hang. Additionally, do not call this
    /// while the PIO interrupt is masked — the ISR that feeds the receive queue
    /// will never fire, causing a guaranteed deadlock. Prefer
    /// [`embedded_can::nb::Can::receive`] if you need a timeout or need to
    /// interleave other work.
    fn receive(&mut self) -> Result<CanFrame, CanError> {
        loop {
            if let Some(frame) = self.rx_queue.pop() {
                return Ok(frame);
            }
            core::hint::spin_loop();
        }
    }
}

// Called by the C library from within can2040_pio_irq_handler. `cd` is a
// pointer to Can2040State.cbus, which is the first field of a repr(C) struct,
// so casting to *mut Can2040State recovers the full state including the Rust
// callback.
unsafe extern "C" fn dispatch_callback(
    cd: *mut sys::can2040,
    notify: u32,
    msg: *mut sys::can2040_msg,
) {
    let state = &*(cd as *mut Can2040State);
    let cb = state.callback;

    if notify == sys::CAN2040_NOTIFY_RX as u32 {
        let frame = CanFrame(*msg);
        #[cfg(feature = "embedded-can")]
        (state.rx_enqueue)(cd as *mut Can2040State, frame);
        cb(Notification::Rx(frame));
    } else if notify == sys::CAN2040_NOTIFY_TX as u32 {
        cb(Notification::Tx(CanFrame(*msg)));
    } else if notify == sys::CAN2040_NOTIFY_ERROR as u32 {
        cb(Notification::Error);
    }
}

// Monomorphised enqueue helper stored as a function pointer in Can2040State.
// Can2040<N> is repr(C) with Can2040State as its first field, so a pointer to
// the state field is the same address as a pointer to Can2040<N> itself.
#[cfg(feature = "embedded-can")]
unsafe fn enqueue_rx<const N: usize>(state: *mut Can2040State, frame: CanFrame) {
    let can = &*(state as *const Can2040<N>);
    can.rx_queue.push(frame);
}
