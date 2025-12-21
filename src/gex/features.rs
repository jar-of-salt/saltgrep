use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct GexFeatures {
    pub state_flags: HashMap<usize, Vec<u64>>,
}

impl GexFeatures {
    pub fn new() -> Self {
        GexFeatures {
            state_flags: HashMap::new(),
        }
    }

    pub fn group_numbers(&self, state_label: usize) -> Option<Vec<(u16, u8)>> {
        self.state_flags.get(&state_label).map(|flags_vec| {
            flags_vec
                .iter()
                .map(|flags| {
                    (
                        (flags >> FlagShifts::CapturingGroup as usize) as u16,
                        ((flags & FlagMasks::CloseGroup as u64) >> FlagShifts::CloseGroup as u64)
                            as u8,
                    )
                })
                .collect()
        })
    }

    pub fn group_number(flags: u64) -> u16 {
        (flags >> FlagShifts::CapturingGroup as usize) as u16
    }
}

pub enum FlagShifts {
    ShortCircuit = 0,
    CloseGroup = 1,
    CapturingGroup = 48,
}

pub enum FlagMasks {
    ShortCircuit = 0x1,
    CloseGroup = 0x2,
    EndAnchor = 0x5,
    CapturingGroup = (0xFFFF << FlagShifts::CapturingGroup as u64),
}
