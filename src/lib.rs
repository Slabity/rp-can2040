#![no_std]

pub use rp_can2040_sys as sys;

use sys::can2040_msg__bindgen_ty_1;

/// Default system clock frequencies
#[cfg(feature = "rp2040")]
pub const DEFAULT_SYS_FREQ: u32 = 125_000_000;
#[cfg(feature = "rp2350")]
pub const DEFAULT_SYS_FREQ: u32 = 150_000_000;

const ID_RTR: u32 = 1 << 30;
const ID_EFF: u32 = 1 << 31;
const EXTENDED_ID_MASK: u32 = 0x1FFF_FFFF;

// ─── CanError ─────────────────────────────────────────────────────────────────

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum CanError {
    Overrun,
}

impl embedded_can::Error for CanError {
    fn kind(&self) -> embedded_can::ErrorKind {
        embedded_can::ErrorKind::Overrun
    }
}

// ─── CanFrame ─────────────────────────────────────────────────────────────────

/// A CAN bus frame.
///
/// The raw `id` field follows the can2040 convention:
/// - Bit 31 (`ID_EFF`): set for extended (29-bit) frames.
/// - Bit 30 (`ID_RTR`): set for remote frames.
/// - Bits 28–0: 29-bit extended ID, or bits 10–0: 11-bit standard ID.
#[derive(Clone)]
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
    /// Per the CAN spec, dlc values 8–15 all carry 8 data bytes.
    /// Returns `None` if `dlc` > 15 or `data` exceeds 8 bytes.
    pub fn new_with_dlc(id: u32, dlc: u32, data: &[u8]) -> Option<Self> {
        if dlc > 15 || data.len() > 8 {
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

    pub fn dlc(&self) -> usize {
        self.0.dlc as usize
    }

    pub fn data(&self) -> &[u8] {
        // dlc > 8 is valid per CAN spec but data array is only 8 bytes
        let len = (self.0.dlc as usize).min(8);
        unsafe { &self.0.__bindgen_anon_1.data[..len] }
    }
}

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
        self.0.id & ID_EFF != 0
    }

    fn is_remote_frame(&self) -> bool {
        self.0.id & ID_RTR != 0
    }

    fn id(&self) -> embedded_can::Id {
        if self.0.id & ID_EFF != 0 {
            embedded_can::Id::Extended(
                embedded_can::ExtendedId::new(self.0.id & EXTENDED_ID_MASK).unwrap(),
            )
        } else {
            embedded_can::Id::Standard(
                embedded_can::StandardId::new((self.0.id & 0x7FF) as u16).unwrap(),
            )
        }
    }

    fn dlc(&self) -> usize {
        self.0.dlc as usize
    }

    fn data(&self) -> &[u8] {
        if self.0.id & ID_RTR != 0 {
            return &[];
        }
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
            self.0.id & ID_EFF != 0,
            self.0.id & ID_RTR != 0,
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
            self.0.id & ID_EFF != 0,
            self.0.id & ID_RTR != 0,
            self.data()
        );
    }
}

// ─── Notification ────────────────────────────────────────────────────────────

/// Event delivered to the user callback from interrupt context.
pub enum Notification {
    Rx(CanFrame),
    Tx(CanFrame),
    Error,
}

/// User-provided callback invoked from interrupt context on each CAN event.
///
/// **Must be ISR-safe**: no blocking, no allocations, no non-reentrant
/// operations. Typical implementations write to a lock-free queue or send to
/// an `embassy_sync::channel::Channel`.
pub type CanCallback = fn(Notification);

// ─── Statistics ──────────────────────────────────────────────────────────────

/// Snapshot of can2040 bus counters.
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CanStatistics {
    pub rx_total: u32,
    pub tx_total: u32,
    pub tx_attempt: u32,
    pub parse_error: u32,
}

// ─── Can2040 ─────────────────────────────────────────────────────────────────

// repr(C) with `cbus` as the first field lets us recover a pointer to
// Can2040State from a *mut can2040 inside the C callback. The C library passes
// `cd` (== &state.cbus) back into the callback, and since Can2040State is
// repr(C) with cbus first, casting cd to *mut Can2040State is valid.
#[repr(C)]
struct Can2040State {
    cbus: sys::can2040, // must remain the first field
    callback: CanCallback,
}

/// Handle to an initialized CAN bus.
///
/// This type owns all CAN state. There are no library-level globals: the
/// caller is responsible for storage and for routing the correct PIO interrupt
/// to [`Can2040::on_irq`].
///
/// Receive is callback-based (see [`CanCallback`]), so `embedded_can::nb::Can`
/// is not implemented. Wrap with a queue if poll-based receive is needed.
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
pub struct Can2040 {
    state: Can2040State,
}

// Can2040 contains *mut c_void (pio_hw) which is not Send by default.
// Safety: Can2040 is designed for single-core Cortex-M use. All access to
// the PIO hardware goes through the can2040 C library which is not
// thread-safe, so the caller is responsible for ensuring exclusive access
// (e.g. via a Mutex or by only accessing from interrupt + main with
// critical sections). Marking Send allows storing Can2040 in a static Mutex.
unsafe impl Send for Can2040 {}

impl Can2040 {
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
                callback,
            },
        };

        unsafe {
            let ptr = &mut can.state.cbus as *mut _;
            sys::can2040_setup(ptr, pio_num);
            sys::can2040_callback_config(ptr, Some(dispatch_callback));
        }

        can
    }

    /// Starts the CAN bus. May also be called after [`stop`](Self::stop) to restart.
    ///
    /// # Arguments
    /// - `sys_clock`: system clock in Hz (use [`DEFAULT_SYS_FREQ`] for the board default).
    /// - `baud_rate`: CAN bit rate in bits per second (e.g. `500_000`).
    /// - `gpio_rx`: GPIO pin for CAN RX.
    /// - `gpio_tx`: GPIO pin for CAN TX. Pass `-1` for receive-only (silent) mode.
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

    /// Must be called from the `PIOx_IRQ_0` interrupt handler for the PIO
    /// chosen at construction.
    pub fn on_irq(&mut self) {
        unsafe {
            sys::can2040_pio_irq_handler(&mut self.state.cbus as *mut _);
        }
    }

    /// Stops the CAN bus and disables the PIO state machines.
    ///
    /// The caller should mask the PIO interrupt **before** calling this
    /// to prevent spurious calls to [`on_irq`](Self::on_irq).
    pub fn stop(&mut self) {
        unsafe {
            sys::can2040_stop(&mut self.state.cbus as *mut _);
        }
    }

    /// Queues a frame for transmission.
    ///
    /// Returns `Err(WouldBlock)` if the transmit queue (4 frames) is full.
    pub fn transmit(&mut self, frame: &CanFrame) -> nb::Result<(), CanError> {
        let ret = unsafe {
            sys::can2040_transmit(
                &mut self.state.cbus as *mut _,
                &frame.0 as *const _ as *mut _,
            )
        };
        if ret < 0 {
            Err(nb::Error::WouldBlock)
        } else {
            Ok(())
        }
    }

    /// Returns `true` if there is space in the transmit queue.
    pub fn check_transmit(&self) -> bool {
        unsafe {
            sys::can2040_check_transmit(&self.state.cbus as *const _ as *mut _) != 0
        }
    }

    /// Returns a snapshot of the current bus counters.
    pub fn statistics(&self) -> CanStatistics {
        let mut raw = sys::can2040_stats {
            rx_total: 0,
            tx_total: 0,
            tx_attempt: 0,
            parse_error: 0,
        };
        unsafe {
            // can2040_get_statistics takes *mut can2040 even though it only reads;
            // cast away the shared reference to satisfy the C signature.
            sys::can2040_get_statistics(
                &self.state.cbus as *const _ as *mut _,
                &mut raw as *mut _,
            );
        }
        CanStatistics {
            rx_total: raw.rx_total,
            tx_total: raw.tx_total,
            tx_attempt: raw.tx_attempt,
            parse_error: raw.parse_error,
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
        cb(Notification::Rx(CanFrame(*msg)));
    } else if notify == sys::CAN2040_NOTIFY_TX as u32 {
        cb(Notification::Tx(CanFrame(*msg)));
    } else if notify == sys::CAN2040_NOTIFY_ERROR as u32 {
        cb(Notification::Error);
    }
}
