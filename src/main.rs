use candid::{CandidType, Decode, Deserialize, Encode, Principal};
use hash_tree::{HashTree, LookupResult};
use ic_cdk::export::candid::candid_method;
use ic_certified_map::{AsHashTree, Hash, RbTree};
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{
    cell::Cell as StableCell, log::Log, DefaultMemoryImpl, StableBTreeMap, Storable,
};
use rand::distributions::Distribution;
use rand::distributions::Uniform;
use rand::Rng;
use rand_core::SeedableRng;
use serde::Serialize;
use sha2::Digest;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::{borrow::Cow, cell::RefCell};
#[macro_use]
extern crate num_derive;

mod hash_tree;

type Memory = VirtualMemory<DefaultMemoryImpl>;
type Blob = Vec<u8>;
type History = Vec<u32>;
type PoolTree = RbTree<Blob, Blob>;
type BlockTree = RbTree<Blob, Hash>;

const MAX_KEY_SIZE: u32 = 32;
const MAX_HISTORY: usize = 8;

#[derive(Clone, Debug, Default, CandidType, Deserialize, FromPrimitive)]
enum Kind {
    #[default]
    Add,
    Remove,
    Select,
    Expand,
}

#[derive(Clone, Debug, Default, CandidType, Deserialize)]
struct Data {
    kind: Kind,
    jurors: Vec<Blob>,
    rand: Option<Blob>,
    jurors_index: u32,
    memo: Blob,
}

#[derive(Clone, Debug, Default, CandidType, Deserialize)]
struct Block {
    certificate: Blob,
    tree: Blob,
    data: Data,
    previous_hash: Hash,
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

#[derive(Clone, Debug, Default, CandidType, Deserialize)]
struct StoreHash(Hash);

#[derive(Clone, Debug, Default, CandidType, Deserialize)]
struct StoreData(Vec<Data>);

#[allow(dead_code)]
#[derive(Deserialize)]
struct ReplicaCertificate {
    tree: HashTree<'static>,
    signature: serde_bytes::ByteBuf,
}

impl Storable for StoreHash {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        Decode!(&bytes, Self).unwrap()
    }
}

impl Storable for StoreData {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        Decode!(&bytes, Self).unwrap()
    }
}

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
    static PENDING_DATA: RefCell<StableCell<StoreData, Memory>> = RefCell::new(StableCell::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(4))),
            StoreData::default()).unwrap());
    static PREVIOUS_HASH: RefCell<StableCell<StoreHash, Memory>> = RefCell::new(StableCell::init(
          MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(5))),
          <StoreHash>::default()).unwrap());
    // Map from juror to history: add index, (delete index, (add index ...))
    static TREE: RefCell<PoolTree> = RefCell::new(RbTree::new());
}

fn set_certificate() -> Blob {
    let data = PENDING_DATA.with(|d| d.borrow().get().0.clone());
    let previous_hash = get_previous_hash();
    let hash = build_tree(&data, &previous_hash).root_hash();
    let certified_data = &ic_certified_map::labeled_hash(b"jury_blocks", &hash);
    ic_cdk::api::set_certified_data(certified_data);
    certified_data.to_vec()
}

fn from_history(history: &History) -> Vec<u8> {
    history
        .iter()
        .map(|h| h.to_le_bytes().to_vec())
        .flatten()
        .collect()
}

fn to_history(b: &Blob) -> History {
    b.chunks_exact(4)
        .map(|h| u32::from_le_bytes(h.try_into().unwrap()))
        .collect()
}

fn push_pending(data: &Data) {
    PENDING_DATA.with(|d| {
        let mut pending = d.borrow().get().0.clone();
        pending.push(data.clone());
        d.borrow_mut().set(StoreData(pending)).unwrap();
    });
}

#[ic_cdk_macros::update(guard = "is_authorized")]
#[candid_method]
fn add(new_jurors: Vec<Blob>, memo: Blob) -> u32 {
    let mut new_data = Data::default();
    new_data.kind = Kind::Add;
    new_data.jurors = new_jurors.clone();
    new_data.memo = memo;
    push_pending(&new_data);
    let index = length() - 1;
    TREE.with(|t| {
        let mut t = t.borrow_mut();
        for j in new_jurors {
            if let Some(history) = t.get(&j) {
                let mut history = to_history(history);
                if history.len() % 2 == 0 {
                    // Currently deleted, reinsert.
                    if history.len() >= MAX_HISTORY {
                        ic_cdk::trap(&format!(
                            "exceeded MAX_HISTORY({}) changes for juror: {:?}",
                            MAX_HISTORY, j
                        ));
                    }
                    history.push(index);
                    t.insert(j, from_history(&history));
                } else {
                    // Already inserted, do nothing.
                }
            } else {
                t.insert(j, from_history(&vec![index]));
            }
        }
    });
    set_certificate();
    index
}

#[ic_cdk_macros::update(guard = "is_authorized")]
#[candid_method]
fn remove(remove_jurors: Vec<Blob>, memo: Blob) -> u32 {
    let mut new_data = Data::default();
    new_data.kind = Kind::Remove;
    new_data.jurors = remove_jurors.clone();
    new_data.memo = memo;
    push_pending(&new_data);
    let index = length() - 1;
    TREE.with(|t| {
        let mut t = t.borrow_mut();
        for j in remove_jurors {
            if let Some(history) = t.get(&j) {
                let mut history = to_history(history);
                if history.len() % 2 == 1 {
                    history.push(index);
                    t.insert(j, from_history(&history));
                } else {
                    // Already deleted, do nothing.
                }
            }
        }
    });
    set_certificate();
    index
}

fn collect_pool(index: u32) -> Vec<Blob> {
    TREE.with(|t| {
        let mut pool = Vec::new();
        t.borrow().for_each(|k, v| {
            // check that the juror is active at index
            let history = to_history(v);
            let mut found = false;
            if history.len() % 2 == 1 {
                // simple case: juror is currently active
                if history[history.len() - 1] <= index {
                    pool.push(k.to_vec());
                    found = true;
                }
            }
            if !found {
                let spans = history.as_slice().chunks_exact(2);
                for span in spans.clone() {
                    // check a span of history where the juror was active
                    if span[0] <= index && span[1] > index {
                        pool.push(k.to_vec());
                        break;
                    }
                }
                // no need to check the remainder as this was covered above
            }
        });
        pool
    })
}

fn make_jury(index: u32, count: u32, seed: Hash) -> Vec<Blob> {
    let mut rng = make_rng(seed);
    let pool = collect_pool(index);
    let mut result = Vec::new();
    let sample: Vec<usize> = sample(&mut rng, count as usize, pool.len());
    for s in sample {
        let juror: &Blob = pool.get(s).unwrap();
        result.push(juror.clone());
    }
    result
}

#[ic_cdk_macros::update(guard = "is_authorized")]
#[candid_method]
async fn select(index: u32, count: u32, memo: Blob) -> u32 {
    let mut new_data = Data::default();
    new_data.kind = Kind::Select;
    let seed = get_rng_seed().await;
    new_data.rand = Some(seed.to_vec());
    new_data.jurors = make_jury(index, count, seed);
    new_data.memo = memo;
    push_pending(&new_data);
    set_certificate();
    length() - 1
}

#[ic_cdk_macros::update(guard = "is_authorized")]
#[candid_method]
fn expand(index: u32, count: u32, memo: Blob) -> u32 {
    let mut new_data = Data::default();
    new_data.kind = Kind::Expand;
    let old = get_block(index);
    new_data.rand = old.data.rand.clone();
    let seed: Hash = old.data.rand.unwrap().try_into().unwrap();
    let old_count = old.data.jurors.len() as u32;
    new_data.jurors = make_jury(index, old_count + count, seed)[old_count as usize..].to_vec();
    new_data.memo = memo;
    push_pending(&new_data);
    set_certificate();
    length() - 1
}

#[ic_cdk_macros::query]
#[candid_method]
fn get_pool(index: u32, start: u32, length: u32) -> Vec<Blob> {
    let pool = collect_pool(index);
    return pool[(start as usize)..((start + length) as usize)].to_vec();
}

#[ic_cdk_macros::query]
#[candid_method]
fn get_certificate() -> Option<Blob> {
    if PENDING_DATA.with(|d| d.borrow().get().0.len()) == 0 {
        None
    } else {
        ic_cdk::api::data_certificate()
    }
}

fn sample<R, IndexVec>(rng: &mut R, amount: usize, length: usize) -> IndexVec
where
    R: Rng + ?Sized,
    IndexVec: From<Vec<usize>>,
{
    assert!(amount <= length);
    let mut cache = HashSet::with_capacity(amount as usize);
    let distr = Uniform::new(0, length);
    let mut indices = Vec::with_capacity(amount as usize);
    for _ in 0..amount {
        let mut pos = distr.sample(rng);
        while !cache.insert(pos) {
            pos = distr.sample(rng);
        }
        indices.push(pos);
    }
    IndexVec::from(indices)
}

fn get_previous_hash() -> Hash {
    let mut previous_hash = PREVIOUS_HASH.with(|h| h.borrow().get().0.clone());
    LOG.with(|l| {
        let l = l.borrow();
        if l.len() > 0 {
            previous_hash =
                sha2::Sha256::digest(Encode!(&l.get(l.len() - 1).unwrap()).unwrap()).into();
        }
    });
    previous_hash
}

fn build_tree(data: &Vec<Data>, previous_hash: &Hash) -> BlockTree {
    let mut tree = BlockTree::default();
    let offset = length() as u32;
    for (i, d) in data.iter().enumerate() {
        let hash: [u8; 32] = sha2::Sha256::digest(Encode!(d).unwrap()).into();
        let i = (i as u32) + offset;
        tree.insert(i.to_be_bytes().to_vec(), hash); // For lexigraphic order.
    }
    tree.insert("previous_hash".as_bytes().to_vec(), *previous_hash); // For lexigraphic order.
    tree
}

#[ic_cdk_macros::update(guard = "is_authorized")]
#[candid_method]
fn commit(certificate: Blob) -> Option<u32> {
    let data = PENDING_DATA.with(|d| d.borrow().get().0.clone());
    if data.len() == 0 {
        return None;
    }
    let previous_hash = get_previous_hash();
    // Check that the certificate corresponds to our tree.  Note: we are
    // not fully verifying the certificate, just checking for races.
    let tree = build_tree(&data, &previous_hash);
    let certified_data = &ic_certified_map::labeled_hash(b"jury_blocks", &tree.root_hash());
    let cert: ReplicaCertificate = serde_cbor::from_slice(&certificate[..]).unwrap();
    let canister_id = ic_cdk::api::id();
    let canister_id = canister_id.as_slice();
    if let LookupResult::Found(certified_data_bytes) = cert.tree.lookup_path(&[
        "canister".into(),
        canister_id.into(),
        "certified_data".into(),
    ]) {
        assert!(certified_data == certified_data_bytes);
    } else {
        ic_cdk::trap("certificate mismatch");
    }
    LOG.with(|l| {
        let l = l.borrow_mut();
        let hash_tree = ic_certified_map::labeled(b"jury_blocks", tree.as_hash_tree());
        let mut serializer = serde_cbor::ser::Serializer::new(vec![]);
        serializer.self_describe().unwrap();
        hash_tree.serialize(&mut serializer).unwrap();
        let tree: Blob = serializer.into_inner();
        for d in data {
            let block = Block {
                certificate: certificate.clone(),
                tree: tree.clone(),
                data: d,
                previous_hash,
            };
            let encoded_block = Encode!(&block).unwrap();
            l.append(&encoded_block).unwrap();
        }
    });
    PENDING_DATA.with(|d| d.borrow_mut().set(StoreData::default()).unwrap());
    Some(length())
}

#[ic_cdk_macros::query]
#[candid_method]
fn get_size(index: u32) -> u32 {
    get_block(index).data.jurors.len() as u32
}

#[ic_cdk_macros::query]
#[candid_method]
fn get_pool_size(index: u32) -> u32 {
    collect_pool(index).len() as u32
}

#[ic_cdk_macros::query]
#[candid_method]
fn length() -> u32 {
    (LOG.with(|l| l.borrow().len()) + PENDING_DATA.with(|d| d.borrow().get().0.len())) as u32
}

#[ic_cdk_macros::query]
#[candid_method]
fn get_pending() -> u32 {
    PENDING_DATA.with(|d| d.borrow().get().0.len()) as u32
}

#[ic_cdk_macros::query]
#[candid_method]
fn get_block(index: u32) -> Block {
    LOG.with(|l| {
        let committed = l.borrow().len() as u32;
        if index < committed {
            return candid::decode_one(&l.borrow().get(index as usize).unwrap()).unwrap();
        }
        let mut b = Block::default();
        b.data = PENDING_DATA.with(|d| {
            d.borrow()
                .get()
                .0
                .get((index - committed) as usize)
                .unwrap()
                .clone()
        });
        b
    })
}

#[ic_cdk_macros::query]
#[candid_method]
fn get_jurors(index: u32) -> Vec<Blob> {
    get_block(index).data.jurors
}

#[ic_cdk_macros::query]
#[candid_method]
fn find(index: u32, jurors: Vec<Blob>) -> Vec<Option<u32>> {
    let pool = collect_pool(index);
    let mut m: HashMap<Blob, u32> = HashMap::new();
    for (i, j) in jurors.iter().enumerate() {
        m.insert(j.clone(), i as u32);
    }
    let mut result: Vec<Option<u32>> = Vec::new();
    result.resize(jurors.len(), None);
    for (i, j) in pool.iter().enumerate() {
        if let Some(k) = m.get(j) {
            result[*k as usize] = Some(i as u32);
        }
    }
    result
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

async fn get_rng_seed() -> Hash {
    let raw_rand: Vec<u8> =
        match ic_cdk::call(Principal::management_canister(), "raw_rand", ()).await {
            Ok((res,)) => res,
            Err((_, err)) => ic_cdk::trap(&format!("failed to get seed: {}", err)),
        };
    let seed: Hash = raw_rand[..].try_into().unwrap_or_else(|_| {
        ic_cdk::trap(&format!(
                "when creating seed from raw_rand output, expected raw randomness to be of length 32, got {}",
                raw_rand.len()
                ));
    });
    seed
}

fn make_rng(seed: Hash) -> rand_chacha::ChaCha20Rng {
    rand_chacha::ChaCha20Rng::from_seed(seed)
}

#[ic_cdk_macros::init]
fn canister_init(previous_hash: Option<String>) {
    authorize_principal(&ic_cdk::caller());
    if let Some(previous_hash) = previous_hash {
        if let Ok(previous_hash) = hex::decode(&previous_hash) {
            if previous_hash.len() == 32 {
                PREVIOUS_HASH.with(|h| {
                    let hash: Hash = previous_hash.try_into().unwrap();
                    h.borrow_mut().set(StoreHash(hash)).unwrap();
                });
                return;
            }
        }
    }
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
