use candid::{CandidType, Deserialize, Encode, Principal};
use hash_tree::{HashTree, LookupResult};
use ic_cdk::export::candid::candid_method;
use ic_certified_map::{AsHashTree, Hash, RbTree};
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{
    cell::Cell as StableCell, log::Log, DefaultMemoryImpl, StableBTreeMap, Storable,
};
use num::FromPrimitive;
use rand_core::{RngCore, SeedableRng};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
#[macro_use]
extern crate num_derive;

mod hash_tree;

type Memory = VirtualMemory<DefaultMemoryImpl>;
type Blob = Vec<u8>;

const MAX_KEY_SIZE: u32 = 32;

#[derive(Clone, Debug, Default, CandidType, Deserialize, FromPrimitive)]
enum Kind {
    #[default]
    Add,
    Remove,
    Jury,
    Expand,
}

#[derive(Clone, Debug, Default, CandidType, Deserialize)]
struct Data {
    kind: Kind,
    jurors: Vec<Blob>,
    rand: Blob,
    jurors_index: u32,
}

#[derive(Clone, Debug, Default, CandidType, Deserialize)]
struct Block {
    certificate: Blob,
    tree: Blob,
    data: Data,
    previous_hash: Blob,
}

#[derive(Clone, Debug, CandidType, Deserialize, FromPrimitive)]
enum Auth {
    Admin,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
struct Authorization {
    id: Principal,
    auth: Auth,
}

type PendingData = Vec<Data>;

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));
    static LOG: RefCell<Log<Memory, Memory>> = RefCell::new(
        Log::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))),
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1))),
            ).unwrap()
        );
    static AUTH: RefCell<StableBTreeMap<Memory, Blob, u32>> = RefCell::new(
        StableBTreeMap::init_with_sizes(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(3))),
            MAX_KEY_SIZE,
            4
            )
        );
    static PENDING_DATA: RefCell<PendingData> = RefCell::new(PendingData::default());
    static PREVIOUS_HASH: RefCell<[u8; 32]> = RefCell::new(<[u8; 32]>::default());
}

fn set_certificate(blocks: &Vec<Data>) -> Blob {
    let hash: [u8; 32] = sha2::Sha256::digest(Encode!(blocks).unwrap()).into();
    let certified_data: &[u8; 32] = &ic_certified_map::labeled_hash(b"jury_block", &hash);
    ic_cdk::api::set_certified_data(certified_data);
    certified_data.to_vec()
}

#[ic_cdk_macros::query]
#[candid_method]
fn get_certificate() -> Option<Blob> {
    if PENDING_DATA.with(|d| d.borrow().len()) == 0 {
        None
    } else {
        ic_cdk::api::data_certificate()
    }
}

#[ic_cdk_macros::query]
#[candid_method]
fn get_block(index: u32) -> Block {
    LOG.with(|m| candid::decode_one(&m.borrow().get(index as usize).unwrap()).unwrap())
}

#[ic_cdk_macros::query]
#[candid_method]
fn get_authorized() -> Vec<Principal> {
    let mut authorized = Vec::new();
    AUTH.with(|a| {
        for (k, _v) in a.borrow().iter() {
            authorized.push(Principal::from_slice(&k));
        }
    });
    authorized
}

#[ic_cdk_macros::update(guard = "is_authorized")]
#[candid_method]
fn authorize(principal: Principal) {
    let value = Auth::Admin;
    AUTH.with(|a| {
        a.borrow_mut()
            .insert(principal.as_slice().to_vec(), value as u32)
            .unwrap();
    });
}

#[ic_cdk_macros::update(guard = "is_authorized")]
#[candid_method]
fn deauthorize(principal: Principal) {
    AUTH.with(|a| {
        a.borrow_mut()
            .remove(&principal.as_slice().to_vec())
            .unwrap();
    });
}

fn authorize_principal(principal: &Principal) {
    AUTH.with(|a| {
        a.borrow_mut()
            .insert(principal.as_slice().to_vec(), Auth::Admin as u32)
            .unwrap();
    });
}

fn is_authorized() -> Result<(), String> {
    AUTH.with(|a| {
        if a.borrow()
            .contains_key(&ic_cdk::caller().as_slice().to_vec())
        {
            Ok(())
        } else {
            Err("You are not authorized".to_string())
        }
    })
}

async fn make_rng() -> rand_chacha::ChaCha20Rng {
    let raw_rand: Vec<u8> =
        match ic_cdk::call(Principal::management_canister(), "raw_rand", ()).await {
            Ok((res,)) => res,
            Err((_, err)) => ic_cdk::trap(&format!("failed to get seed: {}", err)),
        };
    let seed: [u8; 32] = raw_rand[..].try_into().unwrap_or_else(|_| {
        ic_cdk::trap(&format!(
                "when creating seed from raw_rand output, expected raw randomness to be of length 32, got {}",
                raw_rand.len()
                ));
    });
    rand_chacha::ChaCha20Rng::from_seed(seed)
}

#[ic_cdk_macros::init]
fn canister_init(previous_hash: Option<String>) {
    authorize_principal(&ic_cdk::caller());
}

#[ic_cdk_macros::post_upgrade]
fn post_upgrade() {
    // Reload state.
}

ic_cdk::export::candid::export_service!();

#[ic_cdk_macros::query(name = "__get_candid_interface_tmp_hack")]
fn export_candid() -> String {
    __export_service()
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    println!("{}", export_candid());
}

#[cfg(target_arch = "wasm32")]
fn main() {}
