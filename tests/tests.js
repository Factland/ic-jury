import fetch from 'node-fetch';
import fs from 'fs';
import crypto from 'crypto';
import sha256 from "sha256";
import { lebDecode, PipeArrayBuffer } from "@dfinity/candid";
import { Principal } from '@dfinity/principal';
import { Secp256k1PublicKey, Secp256k1KeyIdentity } from '@dfinity/identity';
import { Actor, Cbor, Certificate, HttpAgent, lookup_path, reconstruct, hashTreeToString } from '@dfinity/agent';
import { idlFactory } from '../src/declarations/jury/jury.did.js';
import exec from 'await-exec';
import assert from 'assert';

function toHex(buffer) { // buffer is an ArrayBuffer
	return [...new Uint8Array(buffer)]
		.map(x => x.toString(16).padStart(2, '0'))
		.join('');
}

function fromHex(hex) {
  const hexRe = new RegExp(/^([0-9A-F]{2})*$/i);
  if (!hexRe.test(hex)) {
    throw new Error("Invalid hexadecimal string.");
  }
  const buffer = [...hex]
    .reduce((acc, curr, i) => {
      acc[(i / 2) | 0] = (acc[(i / 2) | 0] || "") + curr;
      return acc;
    }, [])
    .map((x) => Number.parseInt(x, 16));

  return new Uint8Array(buffer).buffer;
}

function isBufferEqual(a, b) {
  if (a.byteLength !== b.byteLength) {
    return false;
  }
  const a8 = new Uint8Array(a);
  const b8 = new Uint8Array(b);
  for (let i = 0; i < a8.length; i++) {
    if (a8[i] !== b8[i]) {
      return false;
    }
  }
  return true;
}

function uint8ArrayToString(a) {
  return new TextDecoder().decode(a);
}

function dataToHex(data) {
  let d = { ...data };
  d.jurors = data.jurors.map(uint8ArrayToString);
  console.log(d);
  d
}

function blockToHex(block) {
  return {
    certificate: toHex(block.certificate),
    tree: toHex(block.tree),
    data: dataToHex(block.data),
    previous_hash: toHex(block.previous_hash)
  };
}

// Install the global brower compatible fetch.
global.fetch = fetch;

// Obtain controller identity.
const privateKeyFile = fs.readFileSync('./identity.pem')
const privateKeyObject = crypto.createPrivateKey({
    key: privateKeyFile,
    format: 'pem'
})
const privateKeyDER = privateKeyObject.export({
    format: 'der',
    type: 'sec1',
});
const PEM_DER_PREFIX = new Uint8Array([0x30, 0x74, 0x02, 0x01, 0x01, 0x04, 0x20]);
assert(isBufferEqual(PEM_DER_PREFIX, privateKeyDER.slice(0, 7)));
let secret_key = new Uint8Array(privateKeyDER.slice(7, 7+32));
const identity = Secp256k1KeyIdentity.fromSecretKey(secret_key);
const principal = identity.getPrincipal().toText();

// Authorize this identity.
console.log('authorizing principal', principal);
let authorize_cmd = 'dfx canister call  jury authorize \'(principal "' + principal + '")\'';
console.log('exec:', authorize_cmd, await exec(authorize_cmd));

// Get canister id.
let localCanisters;
try {
  localCanisters = JSON.parse(fs.readFileSync('../.dfx/local/canister_ids.json'));
} catch (error) {
  console.log('No local canister_ids.json found. Continuing production', error);
}
 
const canisterId = localCanisters['jury']['local'];
const url = 'http://' + canisterId + '.localhost:8080';

export const createActor = (idlFactory, canisterId, options) => {
  let agentOptions = options ? {...options.agentOptions} : {};
  const agent = new HttpAgent(agentOptions);
	agent.fetchRootKey().catch(err => {
		console.warn('Unable to fetch root key. Check to ensure that your local replica is running');
		console.error(err);
	});
  return Actor.createActor(idlFactory, {
    agent,
    canisterId,
    ...(options ? options.actorOptions : {}),
  });
};

// Now for the actual test
let actor = createActor(idlFactory, canisterId, { agentOptions: { host: url, identity }});

console.log('blockchain length', await actor.length());

var encoder = new TextEncoder();
let juror1 = encoder.encode("juror 1");
let juror2 = encoder.encode("juror 2");
let juror3 = encoder.encode("juror 3");
let add_jurors = [juror1, juror2, juror3];

let index = await actor.add(add_jurors);
console.log('add block number', index);

console.log('blockchain length', await actor.length());
console.log('get pending', await actor.get_pending());
let block = await actor.get_block(index);
console.log('get block from index', index, blockToHex(block));

index = await actor.remove([juror2])
console.log('remove block number', index);
console.log('blockchain length', await actor.length());
console.log('get pending', await actor.get_pending());
block = await actor.get_block(index);
console.log('get block from index', index, blockToHex(block));

let certificate = await actor.get_certificate();
console.log('certificate', toHex(certificate[0]));
let result = await actor.commit(certificate[0]);
console.log('commit result', result);
index = result[0];

console.log('blockchain length', await actor.length());
let size = await actor.get_size(index - 1)
console.log("jurors size from index", index, size);

let pool_size = await actor.get_pool_size(index - 1)
console.log("juror pool size from index", index, pool_size);

block = await actor.get_block(index - 1);
console.log('get block from index', index, blockToHex(block));

console.log('dfx ping');
let ping_output = await exec('dfx ping');
let root_key_pos = ping_output.stdout.search('"root_key"');
let root_key = JSON.parse('{ ' + ping_output.stdout.substring(root_key_pos));
root_key = new Uint8Array(root_key.root_key);
console.log('root_key', toHex(root_key));
let block_certificate = Cbor.decode(block.certificate);

console.log('deauthorizing', identity.getPrincipal().toText());
await actor.deauthorize(identity.getPrincipal());
