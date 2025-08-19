use std::{marker::PhantomData, mem::ManuallyDrop};

use solana_instruction::Instruction;
use solana_stable_layout::{stable_instruction::StableInstruction, stable_vec::StableVec};

/// Similarly to [`StableInstruction`], this type represents an instruction with a stable (`repr(C)` memory layout).
/// Unlike `StableInstruction`, it does not semantically own the buffers inside the instruction, and they will not be dropped
/// when the type is.
pub(crate) struct StableInstructionBorrowed<'ix> {
    /// A [`StableInstruction`] is constructed from a shared reference to an [`Instruction`] to ensure a valid memory layout.
    /// [`ManuallyDrop`] is used to ensure the borrowed data is not dropped when the type is.
    stabilized_instruction: ManuallyDrop<StableInstruction>,
    /// We don't actually need access to the original instruction, but we do need to ensure it is borrowed for as long as this
    /// type is accessible to ensure it is not moved/invalidated.
    _marker: PhantomData<&'ix Instruction>,
}

impl<'ix> StableInstructionBorrowed<'ix> {
    #[inline(always)]
    pub(crate) fn new(ix: &'ix Instruction) -> Self {
        let data = StableVecBorrowed::from(&ix.data);
        let accounts = StableVecBorrowed::from(&ix.accounts);
        // SAFETY:
        // We transmute between two `repr(C)` types with the same layout (and verify this) assumption
        // in `test_layout_matches`
        // We then immediately move our constructed `StableInstruction` into `ManuallyDrop` to prevent it
        // being dropped and freeing data we don't own.
        let fake_stable_ix = unsafe {
            ManuallyDrop::new(StableInstruction {
                accounts: core::mem::transmute::<StableVecBorrowed<_>, StableVec<_>>(accounts),
                data: core::mem::transmute::<StableVecBorrowed<_>, StableVec<_>>(data),
                program_id: ix.program_id,
            })
        };

        Self {
            stabilized_instruction: fake_stable_ix,
            _marker: PhantomData,
        }
    }

    pub(crate) fn instruction_addr(&self) -> *const u8 {
        &self.stabilized_instruction as *const ManuallyDrop<StableInstruction> as *const u8
    }
}

/// Similarly to [`StableVec`] this type represents a vector with a stable (`repr(C)` memory layout).
/// However, unlike `StableVec` it does not own its contents, instead borrowing the data immutably.
#[repr(C)]
struct StableVecBorrowed<'vec, T> {
    addr: u64,
    cap: u64,
    len: u64,
    _marker: PhantomData<&'vec T>,
}

impl<'a, T> From<&'a Vec<T>> for StableVecBorrowed<'a, T> {
    fn from(value: &'a Vec<T>) -> Self {
        Self {
            addr: value.as_ptr() as u64,
            cap: value.capacity() as u64,
            len: value.len() as u64,
            _marker: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_layout_matches() {
        // This relies on the memory layout of `StableVec` and `StableVecBorrowed` to match as we transmute between them
        let vector: Vec<u8> = vec![1, 2, 3, 4];
        let borrowed = StableVecBorrowed::from(&vector);
        let StableVecBorrowed {
            addr: b_addr,
            cap: b_cap,
            len: b_len,
            ..
        } = &borrowed;
        let StableVec { addr, cap, len, .. } =
            unsafe { std::mem::transmute::<&StableVecBorrowed<u8>, &StableVec<u8>>(&borrowed) };
        assert_eq!(addr, b_addr, "Address field layout does not match");
        assert_eq!(cap, b_cap, "Capacity field layout does not match");
        assert_eq!(len, b_len, "Length field layout does not match");
    }
}
