#![no_std]
#![allow(warnings)]

include!(concat!(env!("OUT_DIR"), "/can2040_bindings.rs"));

macro_rules! impl_zeroed_default {
    ($($t:ty),*) => {
        $(
            impl Default for $t {
                fn default() -> Self {
                    // SAFETY: All-zero bytes is a valid representation for these C structs.
                    // Raw pointer fields become null pointers, integer fields become 0.
                    unsafe { core::mem::zeroed() }
                }
            }
        )*
    };
}

impl_zeroed_default!(
    can2040_bitunstuffer,
    can2040_msg__bindgen_ty_1,
    can2040_msg,
    can2040_transmit,
    can2040
);
