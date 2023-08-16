# An Unpredictable Random Jury

This canister enables selecting an unpredictable random jury from a juror pool.  It holds a chain of blocks, each of which is certified by the IC Root Key and contain either changes to the jury pool (adding or removing members), selecting a jury pool, or extending a jury pool.  Selection is based on a random seed which is derived from the IC threshold signature of the next block and is therefor unpredictable without compromise of the IC.

The result is a irrefutable record of random jury selection independent of the controllers.  The certified blockchain is public and available for remote backup.  The canister can also owned by a detached canister e.g. https://github.com/ninegua/ic-blackhole or a DAO to ensure availability.

## Blockchain

The blockchain is a sequence of blocks of the format:

```
type Kind = variant { Add; Remove; Select; Expand; };
type Data = record {
  kind: Kind;
  jurors: vec blob;
  rand: opt blob; // Only present for Select/Expand.
  jurors_index: nat32; // Only used for Select/Expand.
  memo: blob;
};
type Block = record {
  // Certificate is signed by the NNS root key and contains the root of tree.
  certificate: blob;
  // Under b"jury_block" is sha256 of a map from block number to sha245(data).
  tree: blob;
  data: Data;
  // Previous commit hash (not previous block hash).
  previous_hash: blob;
};
```

The canister smart contract provides an API to add and remove jury pool members, select and extend juries and commit and retrieve blocks:

```
service jury: (opt text) -> {
  //
  // Juror pool and jury operations
  //
  // Stage an Add Block and return the future log index.
  add: (jurors: vec blob, memo: blob) -> (nat32);
  // Stage a Remove and return the future log index.
  remove: (jurors: vec blob, memo: blob) -> (nat32);
  // Stage a Jury Block and return the future log index.
  select: (index: nat32, count: nat32, memo: blob) -> (nat32);
  // Stage an Expand Block and return the future log index.
  // The selected jury uses the same random number as the given 'index'.
  expand: (index: nat32, more: nat32, memo: blob) -> (nat32);

  //
  // Certification and operation log commit
  //
  // Get certificate for the certified data for the staged Block(s).
  // Returns None if nothing is staged.
  get_certificate: () -> (opt blob) query;
  // Commit the staged Block returning None if nothing is staged or length().
  commit: (certificate: blob) -> (opt nat32);
                              
  //
  // State accessors
  //
  // Return length of the log index including any pending Block(s).
  length: () -> (nat32) query;
  // Return the size of the pool of jurors at a log index.
  get_pool_size: (index: nat32) -> (nat32) query;
  // Return the size of the set of jurors (i.e. added, removed, select or extend) at a log index.
  get_size: (index: nat32) -> (nat32) query;
  // Return the number of pending Block(s).
  get_pending: () -> (nat32) query;
  // Get a Block.
  get_block: (index: nat32) -> (Block) query;
  // Return set of jurors (i.e. added, removed, select or extend) at a log index.
  get_jurors: (index: nat32) -> (vec blob) query;
  // Find the indexes of the jurors in the jury pool at the given log index.
  find: (index: nat32, jurors: vec blob) -> (vec opt nat32) query;
  // Returns jurors from the pool at the given log index.
  get_pool: (index: nat32, start: nat32, length: nat32) -> (vec blob) query;
  // Returns the ranges where the juror was eligible [start, end)* where the last range may not end.
  get_history: (juror: blob) -> (vec nat32) query;

  //
  // Manage the set of Principals allowed to stage and commit Blocks.
  //
  authorize: (principal) -> ();
  deauthorize: (principal) -> ();
  get_authorized: () -> (vec principal) query;
}
```

## Certification

The certificate contains an NNS signed delegation for the canister to the subnet which certifies the canister root hash along with the date.  The canister root hash is the root of the Merkle tree containing the hashes of all the block entries.  This enables each entry to be independently certified by extracting the corresponding path from the tree.

## Storing Blocks

First blocks are staged by calling `add()` `remove()`, `select()` or `extend()` which returns block index (for reference).  Then the certificate is retrieved via `get_certificate()` and then the blocks are appended by calling `commit()` with the certificate.

## Blockchain Persistence

The canister smart contract stores all persistent data in stable memory.  There is no provision for deleting or rewriting blocks short of reinstalling or deleting the canister.  However, because the blocks are certified, they can be backed up remotely and validated offline.  The blocks can even be transfered to a different canister smart contract by re-storing the blocks and substituting the original certificate during the `commit()` phase.

## Selection Cost

The algorithm used for jury selection (sample with rejection) has poor perfomance for pool size P and sample size S, when S is large and S nearly P (e.g. more than P/2).  This situation should be avoided.  The expectation is that P will generally be more than twice as large as S at which point the cost is essentially linear in S.

## Privacy
  
The jury pool identities are blinded.
In order to protect the identity of jurors, a blinded per jury identity is used <COMPLETE HERE>.
The controlling entity can provide a signed testimonial that the individual has the blinded jury id without disclosing their user id or blinded jury pool id.

## Viewing and Verifying 

Scripts are provided for these.
Verify your vote by comparing your per-jury id to the per-jury id provided by the controlling entity.

## Usage

### Backup and Remove Old Blocks

In some use cases it may be desirable to backup and remove old blocks from the canister smart contract.  A controller principal with `Admin` authoriation should remove all user permissions to prevent updates to the blockchain, `get_block` all the blocks and back them up, then deploy with `mode=reinstall` to wipe stable memory and (optionally) pass in the final block's hash (the result of `last_hash()`) as a 64-character hex value: `dfx deploy --argument '(opt "AABB...")'`.  Finally, User permissions can be restored.  Users should periodically retry if they get permission denied.

## Development

### Depenedencies

* node, npm
* rustup, cargo, rustc with wasm
* hash\_tree.rs is copied from github.com/dfinity/agent-rssrc/hash\_tree/mod.rs

### Setup

* (cd tests; npm i)

### Build

* make

### Test

* dfx start --background
* dfx deploy
* make test
