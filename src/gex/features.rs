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

    pub fn increment_group_number(flags: u64, group_shift: u16) -> u64 {
        let mut new_flags = flags;
        let big_new_number: u64 = (GexFeatures::group_number(flags) as u64) + group_shift as u64;
        let cast_result: Result<u16, _> = big_new_number.try_into();
        match cast_result {
            Ok(new_group_number) => {
                new_flags &= flags & !(FlagMasks::CapturingGroup as u64);
                new_flags |= (new_group_number as u64) << FlagShifts::CapturingGroup as u64;
                new_flags
            }
            Err(e) => {
                panic!("New group number exceeds size of u16; error: {:?}", e)
            }
        }
    }
}

pub enum FlagShifts {
    CapturingGroup = 0,
    CloseGroup = 16,
    ShortCircuit = 17,
}

pub enum FlagMasks {
    CapturingGroup = (0xFFFF << FlagShifts::CapturingGroup as u64),
    CloseGroup = 0x1 << FlagShifts::CloseGroup as u64,
    ShortCircuit = 0x1 << FlagShifts::ShortCircuit as u64,
}
