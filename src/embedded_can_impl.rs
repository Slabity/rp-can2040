use crate::{CanError, CanFrame, ID_EFF, ID_RTR, EXTENDED_ID_MASK};
use crate::sys::can2040_msg__bindgen_ty_1;

impl embedded_can::Error for CanError {
    fn kind(&self) -> embedded_can::ErrorKind {
        embedded_can::ErrorKind::Overrun
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
        Some(Self(crate::sys::can2040_msg {
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
        Some(Self(crate::sys::can2040_msg {
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
