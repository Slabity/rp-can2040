//! Embassy async receive example
//!
//! Demonstrates bridging the ISR callback to an async task using an
//! `embassy_sync::channel::Channel`. The callback pushes received frames
//! into the channel with `try_send`; the async task awaits them.
//!
//! rp2040-hal is used for hardware initialisation so that the rest of the
//! application stack does not need to switch to embassy-rp.

#![no_std]
#![no_main]

use core::cell::RefCell;
use critical_section::Mutex;
use defmt::info;
use defmt_rtt as _;

#[link_section = ".boot2"]
#[used]
static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use panic_probe as _;
use rp2040_hal::clocks::init_clocks_and_plls;
use rp2040_hal::pac::interrupt;
use rp2040_hal::{pac, Sio, Watchdog};
use rp_can2040::{Can2040, CanCallback, CanFrame, CanStatistics, Notification};


const XOSC_CRYSTAL_FREQ: u32 = 12_000_000;
const BAUD_RATE: u32 = 500_000;
const GPIO_RX: i32 = 16;
const GPIO_TX: i32 = 17;

// Channel capacity should be sized to the worst-case burst between task wakeups.
static CHANNEL: Channel<CriticalSectionRawMutex, CanFrame, 8> = Channel::new();
static CAN: Mutex<RefCell<Option<Can2040>>> = Mutex::new(RefCell::new(None));

#[interrupt]
fn PIO0_IRQ_0() {
    critical_section::with(|cs| {
        if let Some(can) = CAN.borrow(cs).borrow_mut().as_mut() {
            can.on_irq();
        }
    });
}

fn on_can_event(notification: Notification) {
    if let Notification::Rx(frame) = notification {
        // try_send is non-blocking; frames are silently dropped if the channel
        // is full. Increase the channel capacity if this is a concern.
        CHANNEL.try_send(frame).ok();
    }
}

#[embassy_executor::task]
async fn can_task() {
    loop {
        let frame = CHANNEL.receive().await;
        info!("RX: {:?}", frame);
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
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

    critical_section::with(|cs| {
        *CAN.borrow(cs).borrow_mut() = Some(can);
    });

    unsafe {
        core.NVIC.set_priority(pac::Interrupt::PIO0_IRQ_0, 0);
        cortex_m::peripheral::NVIC::unmask(pac::Interrupt::PIO0_IRQ_0);
    }

    info!("CAN bus listening (embassy)");

    spawner.spawn(can_task()).unwrap();

    let mut prev = CanStatistics { rx_total: 0, tx_total: 0, tx_attempt: 0, parse_error: 0 };

    loop {
        // Blocking delay: the executor stalls here, but frames are still queued
        // by the ISR into CHANNEL (capacity 8) and drained by can_task between
        // stat prints.
        cortex_m::asm::delay(125_000_000); // ~1s at 125MHz

        let current = critical_section::with(|cs| {
            CAN.borrow(cs).borrow().as_ref().map(|c| c.statistics())
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
