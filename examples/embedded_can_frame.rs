//! embedded-can frame construction example
//!
//! Demonstrates building CAN frames using the `embedded_can::Frame` trait,
//! which accepts `StandardId` and `ExtendedId` instead of raw id words.
//! Requires the `embedded-can` feature.

#![no_std]
#![no_main]

use core::cell::RefCell;
use cortex_m::interrupt::Mutex;
use defmt::{error, info};
use defmt_rtt as _;

#[link_section = ".boot2"]
#[used]
static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;
use embedded_can::{ExtendedId, Frame, Id, StandardId};
use panic_probe as _;
use rp2040_hal::clocks::init_clocks_and_plls;
use rp2040_hal::pac::interrupt;
use rp2040_hal::{entry, pac, Sio, Watchdog};
use rp_can2040::{Can2040, CanCallback, CanFrame, CanStatistics, Notification};


const XOSC_CRYSTAL_FREQ: u32 = 12_000_000;
const BAUD_RATE: u32 = 500_000;
const GPIO_RX: i32 = 16;
const GPIO_TX: i32 = 17;

static CAN: Mutex<RefCell<Option<Can2040>>> = Mutex::new(RefCell::new(None));

#[interrupt]
fn PIO0_IRQ_0() {
    cortex_m::interrupt::free(|cs| {
        if let Some(can) = CAN.borrow(cs).borrow_mut().as_mut() {
            can.on_irq();
        }
    });
}

fn on_can_event(notification: Notification) {
    match notification {
        Notification::Rx(frame) => {
            // Inspect the frame using the embedded_can::Frame trait methods
            match frame.id() {
                Id::Standard(id) => info!("RX standard id={:#x} data={:x}", id.as_raw(), frame.data()),
                Id::Extended(id) => info!("RX extended id={:#x} data={:x}", id.as_raw(), frame.data()),
            }
        }
        Notification::Tx(_) => info!("TX confirmed"),
        Notification::Error => error!("CAN bus error"),
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

    let mut can = Can2040::new(0 /* PIO0 */, on_can_event as CanCallback);

    can.start(rp_can2040::DEFAULT_SYS_FREQ, BAUD_RATE, GPIO_RX, GPIO_TX);

    cortex_m::interrupt::free(|cs| {
        *CAN.borrow(cs).borrow_mut() = Some(can);
    });

    unsafe {
        core.NVIC.set_priority(pac::Interrupt::PIO0_IRQ_0, 0);
        cortex_m::peripheral::NVIC::unmask(pac::Interrupt::PIO0_IRQ_0);
    }

    // Build frames using the embedded_can::Frame trait via UFCS.
    // CanFrame::new(u32, ..) is the inherent constructor; the trait version
    // requires explicit qualification to disambiguate.
    let std_frame = <CanFrame as Frame>::new(
        StandardId::new(0x123).unwrap(),
        &[0x01, 0x02, 0x03],
    )
    .unwrap();

    let ext_frame = <CanFrame as Frame>::new(
        ExtendedId::new(0x1234_5678).unwrap(),
        &[0xDE, 0xAD, 0xBE, 0xEF],
    )
    .unwrap();

    let remote_frame = <CanFrame as Frame>::new_remote(
        StandardId::new(0x456).unwrap(),
        4,
    )
    .unwrap();

    info!("Sending standard frame:  {:?}", std_frame);
    info!("Sending extended frame:  {:?}", ext_frame);
    info!("Sending remote frame:    {:?}", remote_frame);

    let mut prev = CanStatistics { rx_total: 0, tx_total: 0, tx_attempt: 0, parse_error: 0 };

    loop {
        cortex_m::asm::delay(62_500_000); // ~500ms

        let current = cortex_m::interrupt::free(|cs| {
            if let Some(can) = CAN.borrow(cs).borrow_mut().as_mut() {
                for frame in [&std_frame, &ext_frame, &remote_frame] {
                    if let Err(_) = can.transmit(frame) {
                        error!("TX queue full");
                    }
                }
                Some(can.statistics())
            } else {
                None
            }
        });

        if let Some(current) = current {
            let delta = current - prev;
            info!(
                "stats: rx={} tx={} tx_attempt={} errors={}",
                delta.rx_total, delta.tx_total, delta.tx_attempt, delta.parse_error
            );
            prev = current;
        }
    }
}
