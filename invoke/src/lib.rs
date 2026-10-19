#![doc = include_str!("../README.md")]
#![allow(unexpected_cfgs)]

use solana_account_info::AccountInfo;
use solana_instruction::Instruction;
use solana_program_entrypoint::ProgramResult;

mod stable_instruction_borrowed;

pub fn invoke(instruction: &Instruction, account_infos: &[AccountInfo]) -> ProgramResult {
    invoke_signed(instruction, account_infos, &[])
}

pub fn invoke_unchecked(instruction: &Instruction, account_infos: &[AccountInfo]) -> ProgramResult {
    invoke_signed_unchecked(instruction, account_infos, &[])
}

pub fn invoke_signed(
    instruction: &Instruction,
    account_infos: &[AccountInfo],
    signers_seeds: &[&[&[u8]]],
) -> ProgramResult {
    // Check that the account RefCells are consistent with the request
    for account_meta in instruction.accounts.iter() {
        for account_info in account_infos.iter() {
            if account_meta.pubkey == *account_info.key {
                if account_meta.is_writable {
                    let _ = account_info.try_borrow_mut_lamports()?;
                    let _ = account_info.try_borrow_mut_data()?;
                } else {
                    let _ = account_info.try_borrow_lamports()?;
                    let _ = account_info.try_borrow_data()?;
                }
                break;
            }
        }
    }

    invoke_signed_unchecked(instruction, account_infos, signers_seeds)
}

#[cfg(target_os = "solana")]
pub fn invoke_signed_unchecked(
    instruction: &Instruction,
    account_infos: &[AccountInfo],
    signers_seeds: &[&[&[u8]]],
) -> ProgramResult {
    use stable_instruction_borrowed::StableInstructionBorrowed;
    let stable = StableInstructionBorrowed::new(instruction);
    let instruction_addr = stable.instruction_addr();

    let result = unsafe {
        solana_define_syscall::definitions::sol_invoke_signed_rust(
            instruction_addr,
            account_infos as *const _ as *const u8,
            account_infos.len() as u64,
            signers_seeds as *const _ as *const u8,
            signers_seeds.len() as u64,
        )
    };

    match result {
        solana_program_entrypoint::SUCCESS => Ok(()),
        _ => Err(result.into()),
    }
}

#[cfg(not(target_os = "solana"))]
pub use solana_sysvar::program_stubs::sol_invoke_signed as invoke_signed_unchecked;

#[cfg(all(test, not(target_os = "solana")))]
mod tests {
    use {
        super::*,
        solana_pubkey::Pubkey,
        solana_sysvar::program_stubs::{set_syscall_stubs, SyscallStubs},
        std::sync::atomic::{AtomicUsize, Ordering},
    };

    static INVOCATIONS: AtomicUsize = AtomicUsize::new(0);

    struct InvokeStub;

    impl SyscallStubs for InvokeStub {
        fn sol_invoke_signed(
            &self,
            _instruction: &Instruction,
            _account_infos: &[AccountInfo],
            _signers_seeds: &[&[&[u8]]],
        ) -> ProgramResult {
            INVOCATIONS.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn invoke_delegates_to_host_stub() {
        INVOCATIONS.store(0, Ordering::SeqCst);
        let old_stubs = set_syscall_stubs(Box::new(InvokeStub));
        let result = invoke(
            &Instruction::new_with_bytes(Pubkey::new_unique(), &[], vec![]),
            &[],
        );
        set_syscall_stubs(old_stubs);

        assert!(result.is_ok());
        assert_eq!(INVOCATIONS.load(Ordering::SeqCst), 1);
    }
}
