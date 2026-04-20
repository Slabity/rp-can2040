//! Dual CAN loopback example (physical transceivers)
//!
//! Runs two independent can2040 instances on PIO0 and PIO1, each connected to
//! a separate SN65HVD230 transceiver. The two transceivers are wired together
//! over a short CAN bus. The built-in 120 Ω resistor on each board provides
//! correct bus termination (two 120 Ω end-terminators → 60 Ω total).
//!
//! CAN0 transmits a frame with ID 0x100 every 500ms. CAN1 receives it and
//! sends a reply with ID 0x200. Both nodes log every received frame.
//!
//! Wiring:
//!
//!   RP2040 GPIO16  →  Transceiver 0 RXD
//!   RP2040 GPIO17  →  Transceiver 0 TXD
//!   RP2040 GPIO14  →  Transceiver 1 RXD
//!   RP2040 GPIO15  →  Transceiver 1 TXD
//!
//!   Transceiver 0 CAN-H  ───  Transceiver 1 CAN-H
//!   Transceiver 0 CAN-L  ───  Transceiver 1 CAN-L
//!
//!   Both transceivers share 3V3 and GND with the RP2040.

#![no_std]
#![no_main]

use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, Ordering};
use cortex_m::interrupt::Mutex;
use defmt::{error, info};
use defmt_rtt as _;
use panic_probe as _;
use rp2040_hal::clocks::init_clocks_and_plls;
use rp2040_hal::pac::interrupt;
use rp2040_hal::{entry, pac, Sio, Watchdog};
use rp_can2040::{Can2040, CanCallback, CanFrame, CanStatistics, Notification};

#[link_section = ".boot2"]
#[used]
static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

const XOSC_CRYSTAL_FREQ: u32 = 12_000_000;
const BAUD_RATE: u32 = 500_000;

const CAN0_GPIO_RX: i32 = 16;
const CAN0_GPIO_TX: i32 = 17;
const CAN1_GPIO_RX: i32 = 14;
const CAN1_GPIO_TX: i32 = 15;

static CAN0: Mutex<RefCell<Option<Can2040>>> = Mutex::new(RefCell::new(None));
static CAN1: Mutex<RefCell<Option<Can2040>>> = Mutex::new(RefCell::new(None));

// Set by the CAN1 callback to request a reply from the main loop.
static CAN1_SEND_REPLY: AtomicBool = AtomicBool::new(false);

#[interrupt]
fn PIO0_IRQ_0() {
    cortex_m::interrupt::free(|cs| {
        if let Some(can) = CAN0.borrow(cs).borrow_mut().as_mut() {
            can.on_irq();
        }
    });
}

#[interrupt]
fn PIO1_IRQ_0() {
    cortex_m::interrupt::free(|cs| {
        if let Some(can) = CAN1.borrow(cs).borrow_mut().as_mut() {
            can.on_irq();
        }
    });
}

fn on_can0_event(notification: Notification) {
    match notification {
        Notification::Rx(frame) => info!("CAN0 RX: {:?}", frame),
        Notification::Error => error!("CAN0 error"),
        _ => {}
    }
}

fn on_can1_event(notification: Notification) {
    match notification {
        Notification::Rx(frame) => {
            info!("CAN1 RX: {:?}", frame);
            CAN1_SEND_REPLY.store(true, Ordering::Relaxed);
        }
        Notification::Error => error!("CAN1 error"),
        _ => {}
    }
}

#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    let mut core = pac::CorePeripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let sio = Sio::new(pac.SIO);

    let _clocks = init_clocks_and_plls(
        XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let _pins = rp2040_hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let mut can0 = Can2040::new(0 /* PIO0 */, on_can0_event as CanCallback);
    can0.start(rp_can2040::DEFAULT_SYS_FREQ, BAUD_RATE, CAN0_GPIO_RX, CAN0_GPIO_TX);
    cortex_m::interrupt::free(|cs| {
        *CAN0.borrow(cs).borrow_mut() = Some(can0);
    });
    unsafe {
        core.NVIC.set_priority(pac::Interrupt::PIO0_IRQ_0, 0);
        cortex_m::peripheral::NVIC::unmask(pac::Interrupt::PIO0_IRQ_0);
    }

    let mut can1 = Can2040::new(1 /* PIO1 */, on_can1_event as CanCallback);
    can1.start(rp_can2040::DEFAULT_SYS_FREQ, BAUD_RATE, CAN1_GPIO_RX, CAN1_GPIO_TX);
    cortex_m::interrupt::free(|cs| {
        *CAN1.borrow(cs).borrow_mut() = Some(can1);
    });
    unsafe {
        core.NVIC.set_priority(pac::Interrupt::PIO1_IRQ_0, 0);
        cortex_m::peripheral::NVIC::unmask(pac::Interrupt::PIO1_IRQ_0);
    }

    info!("Dual CAN loopback ready at {} baud", BAUD_RATE);

    let mut counter: u8 = 0;
    let mut prev0 = CanStatistics { rx_total: 0, tx_total: 0, tx_attempt: 0, parse_error: 0 };
    let mut prev1 = CanStatistics { rx_total: 0, tx_total: 0, tx_attempt: 0, parse_error: 0 };

    loop {
        cortex_m::asm::delay(62_500_000); // ~500ms at 125MHz

        // CAN0 sends a frame.
        let frame = CanFrame::new(0x100, &[counter]).unwrap();
        let stats0 = cortex_m::interrupt::free(|cs| {
            if let Some(can) = CAN0.borrow(cs).borrow_mut().as_mut() {
                match can.transmit(&frame) {
                    Ok(()) => info!("CAN0 TX: {:?}", frame),
                    Err(_) => error!("CAN0 TX queue full"),
                }
                Some(can.statistics())
            } else {
                None
            }
        });

        // If CAN1 received a frame, send a reply.
        let stats1 = cortex_m::interrupt::free(|cs| {
            if let Some(can) = CAN1.borrow(cs).borrow_mut().as_mut() {
                if CAN1_SEND_REPLY.load(Ordering::Relaxed) {
                    CAN1_SEND_REPLY.store(false, Ordering::Relaxed);
                    let reply = CanFrame::new(0x200, &[counter]).unwrap();
                    match can.transmit(&reply) {
                        Ok(()) => info!("CAN1 TX: {:?}", reply),
                        Err(_) => error!("CAN1 TX queue full"),
                    }
                }
                Some(can.statistics())
            } else {
                None
            }
        });

        if let Some(current) = stats0 {
            let delta = current - prev0;
            info!(
                "CAN0 stats: rx={} tx={} tx_attempt={} errors={}",
                delta.rx_total, delta.tx_total, delta.tx_attempt, delta.parse_error
            );
            // Bus-off detection: attempts were made but none succeeded. Reset
            // to recover the PIO state machine and drain the stale TX queue.
            if delta.tx_attempt > 0 && delta.tx_total == 0 {
                error!("CAN0 bus-off detected, resetting");
                cortex_m::interrupt::free(|cs| {
                    if let Some(can) = CAN0.borrow(cs).borrow_mut().as_mut() {
                        can.reset(rp_can2040::DEFAULT_SYS_FREQ, BAUD_RATE, CAN0_GPIO_RX, CAN0_GPIO_TX);
                    }
                });
                prev0 = CanStatistics { rx_total: 0, tx_total: 0, tx_attempt: 0, parse_error: 0 };
            } else {
                prev0 = current;
            }
        }

        if let Some(current) = stats1 {
            let delta = current - prev1;
            info!(
                "CAN1 stats: rx={} tx={} tx_attempt={} errors={}",
                delta.rx_total, delta.tx_total, delta.tx_attempt, delta.parse_error
            );
            if delta.tx_attempt > 0 && delta.tx_total == 0 {
                error!("CAN1 bus-off detected, resetting");
                cortex_m::interrupt::free(|cs| {
                    if let Some(can) = CAN1.borrow(cs).borrow_mut().as_mut() {
                        can.reset(rp_can2040::DEFAULT_SYS_FREQ, BAUD_RATE, CAN1_GPIO_RX, CAN1_GPIO_TX);
                    }
                });
                prev1 = CanStatistics { rx_total: 0, tx_total: 0, tx_attempt: 0, parse_error: 0 };
            } else {
                prev1 = current;
            }
        }

        counter = counter.wrapping_add(1);
    }
}
