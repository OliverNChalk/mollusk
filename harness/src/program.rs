//! Module for working with Solana programs.

use {
    solana_bpf_loader_program::syscalls::create_program_runtime_environment_v1,
    solana_compute_budget::compute_budget::ComputeBudget,
    solana_program_runtime::{
        invoke_context::BuiltinFunctionWithContext,
        loaded_programs::{LoadProgramMetrics, ProgramCacheEntry, ProgramCacheForTxBatch},
    },
    solana_sdk::{
        account::{Account, AccountSharedData},
        bpf_loader,
        bpf_loader_upgradeable::{self, UpgradeableLoaderState},
        feature_set::FeatureSet,
        native_loader,
        pubkey::Pubkey,
        rent::Rent,
    },
    std::sync::{Arc, RwLock},
};

pub struct ProgramCache {
    cache: RwLock<ProgramCacheForTxBatch>,
}

impl Default for ProgramCache {
    fn default() -> Self {
        let mut cache = ProgramCacheForTxBatch::default();
        BUILTINS.iter().for_each(|builtin| {
            let program_id = builtin.program_id;
            let entry = builtin.program_cache_entry();
            cache.replenish(program_id, entry);
        });
        Self {
            cache: RwLock::new(cache),
        }
    }
}

impl ProgramCache {
    pub(crate) fn cache(&self) -> &RwLock<ProgramCacheForTxBatch> {
        &self.cache
    }

    /// Add a program to the cache.
    pub fn add_program(
        &mut self,
        program_id: &Pubkey,
        loader_key: &Pubkey,
        elf: &[u8],
        compute_budget: &ComputeBudget,
        feature_set: &FeatureSet,
    ) {
        let environment = Arc::new(
            create_program_runtime_environment_v1(feature_set, compute_budget, false, false)
                .unwrap(),
        );
        self.cache.write().unwrap().replenish(
            *program_id,
            Arc::new(
                ProgramCacheEntry::new(
                    loader_key,
                    environment,
                    0,
                    0,
                    elf,
                    elf.len(),
                    &mut LoadProgramMetrics::default(),
                )
                .unwrap(),
            ),
        );
    }

    /// Add a builtin program to the cache.
    pub fn add_builtin(&mut self, builtin: Builtin) {
        let program_id = builtin.program_id;
        let entry = builtin.program_cache_entry();
        self.cache.write().unwrap().replenish(program_id, entry);
    }
}

pub struct Builtin {
    program_id: Pubkey,
    name: &'static str,
    entrypoint: BuiltinFunctionWithContext,
}

impl Builtin {
    fn program_cache_entry(&self) -> Arc<ProgramCacheEntry> {
        Arc::new(ProgramCacheEntry::new_builtin(
            0,
            self.name.len(),
            self.entrypoint,
        ))
    }
}

static BUILTINS: &[Builtin] = &[
    Builtin {
        program_id: solana_system_program::id(),
        name: "system_program",
        entrypoint: solana_system_program::system_processor::Entrypoint::vm,
    },
    Builtin {
        program_id: bpf_loader::id(),
        name: "solana_bpf_loader_program",
        entrypoint: solana_bpf_loader_program::Entrypoint::vm,
    },
    Builtin {
        program_id: bpf_loader_upgradeable::id(),
        name: "solana_bpf_loader_upgradeable_program",
        entrypoint: solana_bpf_loader_program::Entrypoint::vm,
    },
    /* ... */
];

fn builtin_program_account(program_id: &Pubkey, name: &str) -> (Pubkey, AccountSharedData) {
    let data = name.as_bytes().to_vec();
    let lamports = Rent::default().minimum_balance(data.len());
    let account = AccountSharedData::from(Account {
        lamports,
        data,
        owner: native_loader::id(),
        executable: true,
        rent_epoch: 0,
    });
    (*program_id, account)
}

/// Get the key and account for the system program.
pub fn system_program() -> (Pubkey, AccountSharedData) {
    builtin_program_account(&BUILTINS[0].program_id, BUILTINS[0].name)
}

/// Get the key and account for the BPF Loader Upgradeable program.
pub fn bpf_loader_upgradeable_program() -> (Pubkey, AccountSharedData) {
    builtin_program_account(&BUILTINS[1].program_id, BUILTINS[1].name)
}

/* ... */

/// Create a BPF Loader 2 program account.
pub fn program_account_loader_2(elf: &[u8]) -> AccountSharedData {
    let lamports = Rent::default().minimum_balance(elf.len());
    AccountSharedData::from(Account {
        lamports,
        data: elf.to_vec(),
        owner: bpf_loader::id(),
        executable: true,
        rent_epoch: 0,
    })
}

/// Create a BPF Loader Upgradeable program account.
pub fn program_account(program_id: &Pubkey) -> AccountSharedData {
    let programdata_address =
        Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::id()).0;
    let data = bincode::serialize(&UpgradeableLoaderState::Program {
        programdata_address,
    })
    .unwrap();
    let lamports = Rent::default().minimum_balance(data.len());
    AccountSharedData::from(Account {
        lamports,
        data,
        owner: bpf_loader_upgradeable::id(),
        executable: true,
        rent_epoch: 0,
    })
}

/// Create a BPF Loader Upgradeable program data account.
pub fn program_data_account(elf: &[u8]) -> AccountSharedData {
    let data = {
        let elf_offset = UpgradeableLoaderState::size_of_programdata_metadata();
        let data_len = elf_offset + elf.len();
        let mut data = vec![0; data_len];
        bincode::serialize_into(
            &mut data[0..elf_offset],
            &UpgradeableLoaderState::ProgramData {
                slot: 0,
                upgrade_authority_address: None,
            },
        )
        .unwrap();
        data[elf_offset..].copy_from_slice(elf);
        data
    };
    let lamports = Rent::default().minimum_balance(data.len());
    AccountSharedData::from(Account {
        lamports,
        data,
        owner: bpf_loader_upgradeable::id(),
        executable: false,
        rent_epoch: 0,
    })
}

/// Create a BPF Loader Upgradeable program and program data account.
///
/// Returns a tuple, where the first element is the program account and the
/// second element is the program data account.
pub fn program_accounts(program_id: &Pubkey, elf: &[u8]) -> (AccountSharedData, AccountSharedData) {
    (program_account(program_id), program_data_account(elf))
}
